//! The scheduler of the merge queue: it takes the leases of the queues that have
//! work, and reconciles each one in one transaction under its lock. A
//! reconciliation writes the facts of `queue_inbox` to the log, then those of
//! the deliveries of the queue, runs `plan`, writes its decisions, and requests
//! the jobs that make the SCM converge. A crash at any point leaves nothing: the
//! transaction rolls back and the next reconciliation does the same work again.

use std::collections::BTreeMap;
use std::error::Error;
use std::sync::Arc;

use cihub_config::QueueSettings;
use cihub_domain::{
    Actor, ConnectionId, Event, EventKind, OrgId, QueueId, RepoId, RunId, RunStatus, SubmittedVia,
    run_of_test_branch,
};
use cihub_mq_core::plan;
use cihub_platform::{
    Delivery, Job, QueueWakeup, TICK, acquire_leases, begin_reconciliation, delete_delivery,
    enqueue_job, mark_dirty, notify_queue_changed, pending_deliveries, release_leases,
    schedule_tick, until_next_tick,
};
use cihub_scm::{ScmDelivery, ScmEvent, ScmUser};
use cihub_store::{PgPool, begin_system, delete_facts, pending_facts};
use serde_json::{Value, json};
use sqlx::{PgConnection, Postgres, Transaction};
use time::{Duration, OffsetDateTime};
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, info, info_span, warn};

use crate::assembly::{assemble, cleanup};
use crate::command::{ItemCommand, Merge, PR_COMMAND, PrCommand, PrCommandRequest, parse_command};
use crate::conditions_status::{observe_jobs, observed_prs};
use crate::config::{load_config, request_config_checks};
use crate::jobs::{
    SubmitPr, connection_job, is_own_context, new_event_id, refresh_after, refresh_at, subject,
    submit, user_of,
};
use crate::log::COVER_AFTER;
use crate::merge::merge_job;
use crate::projection::record_late_check;
use crate::repair::{merge_in_flight, refresh_checks, refresh_target, refresh_target_after};
use crate::surfaces::{WELCOME_PR, WelcomePr, converge_surfaces};
use crate::target::QueueTarget;
use crate::{MergeQueueError, QueueLog, parse_id};

/// The translation of a delivery of the SCM by its adapter.
pub type Translate =
    Arc<dyn Fn(&str, &Value) -> Result<ScmDelivery, Box<dyn Error + Send + Sync>> + Send + Sync>;

/// A pass takes at most this many leases.
const LEASES_PER_PASS: i64 = 50;

/// A reconciliation reads at most this many deliveries and facts; the rest
/// waits for the next one, which the queue, still dirty, asks for at once.
const INBOX_PER_RECONCILIATION: i64 = 200;

/// A pass over the deliveries of the connections reads at most this many.
const CONNECTION_DELIVERIES_PER_PASS: i64 = 100;

/// What a reconciliation needs besides the queue.
#[derive(Clone)]
pub struct Reconciler {
    pub translate: Translate,
    /// The address of the dashboard (`http.public_url`), for the links of the
    /// statuses and the comments of the PRs; `None` without it.
    pub dashboard_url: Option<String>,
}

impl Reconciler {
    /// Reconciles `queue` at the time of `clock`, in its own transaction. Does
    /// nothing when another transaction reconciles it; an organization without
    /// the module only loses the work of the queue.
    ///
    /// # Errors
    ///
    /// Postgres fails, or the log of the queue does not fit its decisions;
    /// nothing is written.
    pub async fn reconcile(
        &self,
        pool: &PgPool,
        queue: QueueId,
        clock: &(dyn Fn() -> OffsetDateTime + Sync),
    ) -> Result<(), MergeQueueError> {
        let Some(mut tx) = begin_reconciliation(pool, queue).await? else {
            return Ok(());
        };

        // Read once the queue is taken: a change that marked it dirty before
        // is seen at a time no earlier than the change.
        let now = clock();
        self.reconcile_in(&mut tx, queue, now).await?;

        Ok(tx.commit().await?)
    }

    /// The work of a reconciliation of `queue`, in the transaction of `conn`
    /// that holds its lock.
    ///
    /// # Errors
    ///
    /// Postgres fails, or the log of the queue does not fit its decisions.
    pub async fn reconcile_in(
        &self,
        conn: &mut PgConnection,
        queue: QueueId,
        now: OffsetDateTime,
    ) -> Result<(), MergeQueueError> {
        let target = QueueTarget::load(conn, queue).await?;
        tracing::Span::current().record("org_id", tracing::field::display(target.org_id));

        let settings = target.settings()?;
        let mut log = QueueLog::load(conn, queue).await?;
        // The facts that the jobs gave back come first: a check reported on
        // a test branch needs the assembly fact of its run.
        let given = pending_facts(conn, queue, INBOX_PER_RECONCILIATION).await?;
        let given_ids: Vec<_> = given.iter().map(|e| e.id).collect();
        let mut written = log.write(conn, given).await?.len();

        delete_facts(conn, &given_ids).await?;

        let mut facts = Vec::new();
        let mut jobs = Vec::new();
        let deliveries = pending_deliveries(conn, queue, INBOX_PER_RECONCILIATION).await?;
        let mut deferred = 0;

        for delivery in &deliveries {
            let processed = match (self.translate)(&delivery.event, &delivery.payload) {
                Ok(read) => {
                    let work = Work {
                        target: &target,
                        settings: &settings,
                        log: &log,
                        delivery,
                    };
                    work.read(conn, read.event, &mut facts, &mut jobs).await?
                }
                Err(err) => {
                    warn!(
                        delivery_id = delivery.delivery_id.as_str(),
                        event = delivery.event.as_str(),
                        error = &*err as &dyn Error,
                        "delivery not translated"
                    );
                    true
                }
            };
            if !processed {
                deferred += 1;
                continue;
            }

            delete_delivery(conn, &delivery.delivery_id).await?;
        }

        let requested_mode = log.snapshot.requested_mode;
        if settings.mode.value != requested_mode {
            facts.push(Event {
                id: new_event_id(),
                org_id: target.org_id,
                queue_id: queue,
                actor: Actor::System,
                occurred_at: now,
                cause_id: None,
                kind: EventKind::QueueSettingsChanged {
                    before: requested_mode,
                    after: settings.mode.value,
                },
            });
        }

        written += log.write(conn, facts).await?.len();

        let plan = plan(&log.snapshot, &settings, now)?;
        let ended = ended_runs(&plan.decisions);
        let decisions = log.write(conn, plan.decisions).await?.len();
        let converge = Converge {
            target: &target,
            settings: &settings,
            log: &log,
            desired: &plan.desired,
            ended: &ended,
        };
        let requested = self.request_jobs(conn, &converge, jobs, now).await?;

        log.cover(conn, now).await?;
        schedule_tick(conn, queue, plan.wake_at).await?;

        // The pages read the queue again once this transaction commits.
        if written + decisions > 0 {
            notify_queue_changed(conn, target.org_id, queue, Some(log.seq)).await?;
        }

        if written + decisions + requested + deferred > 0 {
            info!(
                deliveries = deliveries.len(),
                deferred,
                facts = written,
                decisions,
                jobs = requested,
                "queue reconciled"
            );
        }

        Ok(())
    }

    /// Requests `jobs`, then the jobs that make the SCM converge to the desired
    /// state: the read of the configuration file on a new head of the target,
    /// the reads that are due (PRs, checks, target head), the assembly of each
    /// run that waits for its test branch, the merge of each proved item, and
    /// the cleanup of each run that ended, after the retention. Returns the
    /// number of new jobs.
    async fn request_jobs(
        &self,
        conn: &mut PgConnection,
        converge: &Converge<'_>,
        mut jobs: Vec<Job>,
        now: OffsetDateTime,
    ) -> Result<usize, MergeQueueError> {
        let target = converge.target;
        if let Some(head) = &converge.log.snapshot.target.head
            && target.config_sha.as_deref() != Some(head.value.as_str())
        {
            jobs.push(load_config(target, &head.value));
        }

        for (point, prs) in reads_by_point(&converge.desired.observe) {
            jobs.extend(refresh_at(target, point, &prs));
        }

        for read in &converge.desired.check_reads {
            jobs.push(refresh_checks(target, read));
        }

        if let Some(point) = converge.desired.target_read {
            jobs.push(refresh_target(target, point));
        }

        for merge in &converge.desired.merges {
            jobs.push(merge_job(target, merge));
        }

        jobs.extend(
            converge_surfaces(
                conn,
                target,
                converge.settings,
                &converge.desired.surfaces,
                self.dashboard_url.as_deref(),
            )
            .await?,
        );

        for branch in &converge.desired.test_branches {
            let assembling = converge
                .log
                .snapshot
                .runs
                .get(&branch.run_id)
                .is_some_and(|run| run.status == RunStatus::Assembling);
            if assembling {
                jobs.push(assemble(target, branch));
            }
        }

        let keep = Duration::minutes(i64::from(
            converge.settings.keep_test_branches_minutes.value,
        ));
        let cleanups = converge
            .ended
            .iter()
            .map(|run_id| (cleanup(target, *run_id), now + keep));
        let mut requested = 0;

        for (job, run_after) in jobs.iter().map(|job| (job.clone(), now)).chain(cleanups) {
            if enqueue_job(conn, &job, run_after, now).await? {
                requested += 1;
            }
        }

        Ok(requested)
    }
}

/// What a reconciliation converges to, once its decisions are written.
struct Converge<'a> {
    target: &'a QueueTarget,
    settings: &'a QueueSettings,
    log: &'a QueueLog,
    desired: &'a cihub_mq_core::Desired,
    /// The runs that the decisions ended.
    ended: &'a [RunId],
}

/// The runs that `decisions` end.
fn ended_runs(decisions: &[Event]) -> Vec<RunId> {
    decisions
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::RunTransitioned {
                run_id,
                from: Some(_),
                to,
                ..
            } if to.is_terminal() => Some(*run_id),
            _ => None,
        })
        .collect()
}

fn reads_by_point(reads: &[cihub_mq_core::Read]) -> BTreeMap<OffsetDateTime, Vec<u64>> {
    let mut by_point: BTreeMap<_, Vec<u64>> = BTreeMap::new();
    for read in reads {
        by_point.entry(read.point).or_default().push(read.pr_number);
    }
    by_point
}

/// One delivery of a queue, in its reconciliation.
struct Work<'a> {
    target: &'a QueueTarget,
    settings: &'a QueueSettings,
    log: &'a QueueLog,
    delivery: &'a Delivery,
}

impl Work<'_> {
    /// What the delivery asks: a fact that it holds, or a job that reads
    /// what it does not hold. The other events wait for their lots. Returns
    /// `false` for a delivery that waits for the next reconciliation: a check
    /// reported on a test branch whose assembly fact has not come yet, or a
    /// push on the target while a merge of the queue is in flight.
    async fn read(
        &self,
        conn: &mut PgConnection,
        event: ScmEvent,
        facts: &mut Vec<Event>,
        jobs: &mut Vec<Job>,
    ) -> Result<bool, MergeQueueError> {
        let delivery_id = self.delivery.delivery_id.as_str();
        let snapshot = &self.log.snapshot;

        match event {
            ScmEvent::PrChanged {
                number,
                label_added,
                opened,
                base_branch,
                ..
            } => {
                if opened
                    && self.settings.comments_enabled.value
                    && self.settings.commands_enabled.value
                {
                    jobs.push(welcome_job(
                        self.target.org_id,
                        self.target.repo.connection.id,
                        delivery_id,
                        self.target.repo_id,
                        number,
                        base_branch,
                    ));
                }
                // The draft PR of a run is the queue's own: it shows no status.
                let is_run_draft = snapshot
                    .runs
                    .values()
                    .any(|run| run.draft_pr_number == Some(number));
                if !is_run_draft {
                    let subject = subject(delivery_id);
                    jobs.extend(observe_jobs(conn, self.target, &subject, &[number]).await?);
                }
                let tracked = snapshot.current_item(number).is_some()
                    || snapshot.requests.values().any(|r| {
                        matches!(r, cihub_mq_core::Request::Submit { pr_number, .. } if *pr_number == number)
                    });
                if tracked {
                    jobs.push(refresh_after(self.target, delivery_id, vec![number]));
                } else if let Some(label) = label_added
                    && self.settings.label_commands_enabled.value
                    && label == self.settings.enqueue_label.value
                {
                    let request = SubmitPr::by_label(self.target.repo_id, number, label);
                    jobs.push(submit(
                        self.target.org_id,
                        self.target.repo.connection.id,
                        delivery_id,
                        &request,
                    ));
                }
            }
            ScmEvent::Pushed {
                branch,
                before,
                after: Some(after),
                sender,
                ..
            } if branch == self.target.target_branch => {
                // The push of a merge of the queue may come before the fact
                // of its merge: it waits for the merge job to end.
                let own = snapshot.target.own_shas.contains(&after);
                if !own && merge_in_flight(conn, self.target.queue_id).await? {
                    return Ok(false);
                }
                let sender_id = user_of(conn, &sender).await?;
                // The push can make a PR behind or in conflict.
                let observed = observed_prs(conn, self.target).await?;
                let subject = subject(delivery_id);
                jobs.extend(observe_jobs(conn, self.target, &subject, &observed).await?);
                facts.push(self.fact(
                    Actor::User { user_id: sender_id },
                    EventKind::TargetPushed {
                        before,
                        after,
                        sender_id: Some(sender_id),
                    },
                ));
            }
            ScmEvent::CheckChanged {
                sha,
                branches,
                check,
                ..
            } => return self.check_changed(conn, sha, &branches, check, facts).await,
            // A comment names no base branch: it comes through the pass of
            // the connection.
            ScmEvent::Pushed { .. }
            | ScmEvent::Commented { .. }
            | ScmEvent::SubmitBoxChecked { .. }
            | ScmEvent::RulesChanged { .. }
            | ScmEvent::RepoChanged { .. }
            | ScmEvent::ConnectionChanged
            | ScmEvent::ReposChanged { .. }
            | ScmEvent::Ignored => {}
        }

        Ok(true)
    }

    /// A report on the test SHA of a live run is a fact; one that comes
    /// before the assembly fact of its run waits for it (`false`); one that
    /// comes after the end of the run is stored without a fact.
    async fn check_changed(
        &self,
        conn: &mut PgConnection,
        sha: String,
        branches: &[String],
        check: cihub_domain::CheckReport,
        facts: &mut Vec<Event>,
    ) -> Result<bool, MergeQueueError> {
        let snapshot = &self.log.snapshot;
        let live = snapshot
            .runs
            .values()
            .any(|run| !run.status.is_terminal() && run.test_sha.as_deref() == Some(&sha));

        if live {
            facts.push(self.fact(
                Actor::System,
                EventKind::CheckObserved {
                    test_sha: sha,
                    check,
                },
            ));
            return Ok(true);
        }

        let awaited = branches
            .iter()
            .filter_map(|branch| run_of_test_branch(branch))
            .any(|run_id| {
                snapshot.runs.get(&run_id).is_some_and(|run| {
                    run.status == RunStatus::Assembling && run.test_sha.is_none()
                })
            });

        if awaited {
            return Ok(false);
        }

        record_late_check(
            conn,
            self.target.org_id,
            self.target.queue_id,
            &sha,
            &check,
            self.delivery.received_at,
        )
        .await?;

        Ok(true)
    }

    /// A fact of the delivery, at the time CIHub received it.
    fn fact(&self, actor: Actor, kind: EventKind) -> Event {
        Event {
            id: new_event_id(),
            org_id: self.target.org_id,
            queue_id: self.target.queue_id,
            actor,
            occurred_at: self.delivery.received_at,
            cause_id: None,
            kind,
        }
    }
}

/// The welcome of the PR `pr_number` of `repo_id`, opened on `base_branch`,
/// after the delivery `delivery_id`.
fn welcome_job(
    org_id: OrgId,
    connection: ConnectionId,
    delivery_id: &str,
    repo_id: RepoId,
    pr_number: u64,
    base_branch: String,
) -> Job {
    let request = WelcomePr {
        repo_id,
        pr_number,
        base_branch,
    };
    connection_job(org_id, connection, delivery_id, WELCOME_PR, json!(request))
}

/// A delivery of a connection without a queue.
struct ConnectionDelivery {
    delivery_id: String,
    org_id: OrgId,
    connection: ConnectionId,
}

/// Reads the deliveries of a connection without a queue, and returns their number. A
/// `merge` command requests its submission, whose job finds the queue by the base
/// branch of the PR; a concluded check on the head of a `not_ready` item asks its queue
/// to read the PR. A delivery read is deleted in the same transaction, except one that
/// waits for the next pass.
///
/// # Errors
///
/// Postgres fails; nothing is written.
pub async fn pass_connection_deliveries(
    pool: &PgPool,
    reconciler: &Reconciler,
    now: OffsetDateTime,
) -> Result<usize, MergeQueueError> {
    type Row = (String, String, String, String, Value);

    let mut tx = begin_system(pool).await?;
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT delivery_id, org_id, connection_id, event, payload
         FROM webhook_inbox
         WHERE queue_id IS NULL AND connection_id IS NOT NULL
         ORDER BY received_at, delivery_id
         LIMIT $1
         FOR UPDATE SKIP LOCKED",
    )
    .bind(CONNECTION_DELIVERIES_PER_PASS)
    .fetch_all(&mut *tx)
    .await?;
    let count = rows.len();

    for (delivery_id, org_id, connection, event, payload) in rows {
        let delivery = ConnectionDelivery {
            org_id: parse_id(&org_id, &delivery_id)?,
            connection: parse_id(&connection, &delivery_id)?,
            delivery_id: delivery_id.clone(),
        };
        let processed = match (reconciler.translate)(&event, &payload) {
            Ok(read) => pass_delivery(&mut tx, &delivery, read.event, now).await?,
            Err(err) => {
                warn!(
                    delivery_id,
                    event,
                    error = &*err as &dyn Error,
                    "delivery not translated"
                );
                true
            }
        };
        if !processed {
            continue;
        }

        delete_delivery(&mut tx, &delivery_id).await?;
    }

    tx.commit().await?;

    Ok(count)
}

/// What a delivery of a connection without a queue asks. Returns
/// `false` for a delivery that waits for the next pass.
async fn pass_delivery(
    conn: &mut PgConnection,
    delivery: &ConnectionDelivery,
    event: ScmEvent,
    now: OffsetDateTime,
) -> Result<bool, MergeQueueError> {
    match event {
        ScmEvent::PrChanged {
            repo_external_id,
            number,
            base_branch,
            label_added,
            opened,
        } => {
            let change = PrChange {
                repo_external_id: &repo_external_id,
                number,
                base_branch: &base_branch,
                label_added,
                opened,
            };
            route_pr_change(conn, delivery, change, now).await?;
        }
        ScmEvent::SubmitBoxChecked {
            repo_external_id,
            number,
            comment_external_id,
            sender,
        } => {
            // Only the box of the comment of CIHub submits: anyone can write a box in a comment of their own.
            let ours = match repo_of(conn, delivery, &repo_external_id).await? {
                Some(repo) => is_our_comment(conn, repo, number, &comment_external_id).await?,
                None => false,
            };
            if ours {
                let command = Command {
                    repo_external_id: &repo_external_id,
                    number,
                    comment_external_id: None,
                    author: &sender,
                    merge: Merge::default(),
                };
                request_submission(conn, delivery, &command, now).await?;
            }
        }
        ScmEvent::Pushed {
            repo_external_id,
            branch,
            ..
        } => {
            return read_heads(conn, delivery, &repo_external_id, &branch, now).await;
        }
        ScmEvent::Commented {
            repo_external_id,
            number,
            comment_external_id,
            author,
            body,
        } => {
            let comment = Comment {
                repo_external_id: &repo_external_id,
                number,
                external_id: &comment_external_id,
                author: &author,
            };
            pass_comment(conn, delivery, &comment, &body, now).await?;
        }
        // A check that has not concluded changes no condition.
        ScmEvent::CheckChanged {
            repo_external_id,
            sha,
            check,
            ..
        // A status of CIHub changes no condition: reading the PR again after
        // each of its publications would only spend points.
        } if check.conclusion.is_some() && !is_own_context(&check.name) => {
            read_waiting_prs(conn, delivery, &repo_external_id, &sha, now).await?;
            observe_checked_prs(conn, delivery, &repo_external_id, &sha, now).await?;
        }
        _ => {}
    }
    Ok(true)
}

/// A comment on the PR `number`, which can hold a command.
struct Comment<'a> {
    repo_external_id: &'a str,
    number: u64,
    /// The identifier of the comment on the SCM.
    external_id: &'a str,
    author: &'a ScmUser,
}

/// Requests the job of the command that `body` holds, if it holds one.
async fn pass_comment(
    conn: &mut PgConnection,
    delivery: &ConnectionDelivery,
    comment: &Comment<'_>,
    body: &str,
    now: OffsetDateTime,
) -> Result<(), MergeQueueError> {
    // The jobs check `commands_enabled` on the queue that they find.
    let item_command = match parse_command(body) {
        Some(PrCommand::Merge(merge)) => {
            let command = Command {
                repo_external_id: comment.repo_external_id,
                number: comment.number,
                comment_external_id: Some(comment.external_id),
                author: comment.author,
                merge,
            };
            request_submission(conn, delivery, &command, now).await?;
            None
        }
        Some(PrCommand::Cancel) => Some(ItemCommand::Cancel),
        Some(PrCommand::Restart) => Some(ItemCommand::Restart),
        None => None,
    };
    if let Some(command) = item_command
        && let Some(repo_id) = repo_of_command(conn, delivery, comment.repo_external_id).await?
    {
        let request = PrCommandRequest {
            repo_id,
            pr_number: comment.number,
            requested_by: comment.author.into(),
            command,
            comment_id: comment.external_id.to_owned(),
        };
        let job = connection_job(
            delivery.org_id,
            delivery.connection,
            &delivery.delivery_id,
            PR_COMMAND,
            json!(request),
        );
        enqueue_job(conn, &job, now, now).await?;
    }
    Ok(())
}

/// A change of a PR whose base branch has several queues, so that the
/// routing gave it no queue.
struct PrChange<'a> {
    repo_external_id: &'a str,
    number: u64,
    base_branch: &'a str,
    label_added: Option<String>,
    opened: bool,
}

/// Whether `comment` is the comment of CIHub on the PR `number` of `repo`.
async fn is_our_comment(
    conn: &mut PgConnection,
    repo: RepoId,
    number: u64,
    comment: &str,
) -> Result<bool, MergeQueueError> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pr_comments
                        WHERE repo_id = $1 AND pr_number = $2 AND comment_external_id = $3)",
    )
    .bind(repo.to_string())
    .bind(i64::try_from(number).unwrap_or(i64::MAX))
    .bind(comment)
    .fetch_one(conn)
    .await
    .map_err(MergeQueueError::Query)
}

/// The repository of the connection of `delivery` whose identifier on the
/// SCM is `external_id`, when CIHub knows it.
async fn repo_of(
    conn: &mut PgConnection,
    delivery: &ConnectionDelivery,
    external_id: &str,
) -> Result<Option<RepoId>, MergeQueueError> {
    let repo: Option<String> =
        sqlx::query_scalar("SELECT id FROM repos WHERE connection_id = $1 AND external_id = $2")
            .bind(delivery.connection.to_string())
            .bind(external_id)
            .fetch_optional(&mut *conn)
            .await?;
    repo.map(|id| parse_id(&id, &id)).transpose()
}

/// Like [`repo_of`], for a command: a command on a repository that the
/// connection does not list is lost, so it is logged.
async fn repo_of_command(
    conn: &mut PgConnection,
    delivery: &ConnectionDelivery,
    external_id: &str,
) -> Result<Option<RepoId>, MergeQueueError> {
    let repo = repo_of(conn, delivery, external_id).await?;

    if repo.is_none() {
        warn!(
            delivery_id = delivery.delivery_id,
            org_id = %delivery.org_id,
            repo_external_id = external_id,
            "command on an unknown repository ignored"
        );
    }

    Ok(repo)
}

/// Gives `change` to the queue of the live item of its PR, which reads the
/// PR again; without a live item, the label of a queue of the branch
/// requests a submission, which chooses the queue.
async fn route_pr_change(
    conn: &mut PgConnection,
    delivery: &ConnectionDelivery,
    change: PrChange<'_>,
    now: OffsetDateTime,
) -> Result<(), MergeQueueError> {
    let Some(repo) = repo_of(conn, delivery, change.repo_external_id).await? else {
        return Ok(());
    };

    let queues = QueueTarget::queues_of_branch(conn, repo, change.base_branch).await?;
    let number = i64::try_from(change.number).unwrap_or(i64::MAX);
    // Each queue reads the PR: its routing says which one shows the status,
    // and the read skips the draft PR of a run.
    for queue in &queues {
        let subject = subject(&delivery.delivery_id);
        for job in observe_jobs(conn, queue, &subject, &[change.number]).await? {
            enqueue_job(conn, &job, now, now).await?;
        }
    }

    // The welcome job checks the settings of the first active queue of the
    // branch.
    if change.opened && !queues.is_empty() {
        let job = welcome_job(
            delivery.org_id,
            delivery.connection,
            &delivery.delivery_id,
            repo,
            change.number,
            change.base_branch.to_owned(),
        );
        enqueue_job(conn, &job, now, now).await?;
    }

    let live: Option<String> = sqlx::query_scalar(
        "SELECT i.queue_id FROM queue_items i JOIN queues q ON q.id = i.queue_id
         WHERE q.repo_id = $1 AND i.pr_number = $2 AND i.is_current
           AND i.state NOT IN ('merged', 'failed', 'cancelled')",
    )
    .bind(repo.to_string())
    .bind(number)
    .fetch_optional(&mut *conn)
    .await?;

    let job = if let Some(queue) = live {
        let target = QueueTarget::load(conn, parse_id(&queue, &queue)?).await?;
        refresh_after(&target, &delivery.delivery_id, vec![change.number])
    } else {
        let Some(label) = change.label_added else {
            return Ok(());
        };
        let wanted = queues.iter().any(|queue| {
            queue.settings().is_ok_and(|settings| {
                settings.label_commands_enabled.value && settings.enqueue_label.value == label
            })
        });
        if !wanted {
            return Ok(());
        }
        let request = SubmitPr::by_label(repo, change.number, label);
        submit(
            delivery.org_id,
            delivery.connection,
            &delivery.delivery_id,
            &request,
        )
    };

    enqueue_job(conn, &job, now, now).await?;

    Ok(())
}

/// Asks each queue of a `not_ready` item whose PR head is `sha` to read the
/// PR again, after a check concluded on that head. The read points of the
/// plan stay the fallback when the delivery is lost.
async fn read_waiting_prs(
    conn: &mut PgConnection,
    delivery: &ConnectionDelivery,
    repo_external_id: &str,
    sha: &str,
    now: OffsetDateTime,
) -> Result<(), MergeQueueError> {
    let Some(repo) = repo_of(conn, delivery, repo_external_id).await? else {
        return Ok(());
    };

    let items: Vec<(String, i64)> = sqlx::query_as(
        "SELECT i.queue_id, i.pr_number FROM queue_items i JOIN queues q ON q.id = i.queue_id
         WHERE q.repo_id = $1 AND i.pr_head_sha = $2
           AND i.is_current AND i.state = 'not_ready'
         ORDER BY i.queue_id, i.pr_number",
    )
    .bind(repo.to_string())
    .bind(sha)
    .fetch_all(&mut *conn)
    .await?;
    let mut prs_by_queue: BTreeMap<String, Vec<u64>> = BTreeMap::new();

    for (queue, number) in items {
        prs_by_queue
            .entry(queue)
            .or_default()
            .push(u64::try_from(number).unwrap_or_default());
    }

    for (queue, prs) in prs_by_queue {
        let target = QueueTarget::load(conn, parse_id(&queue, &queue)?).await?;
        let job = refresh_after(&target, &delivery.delivery_id, prs);
        enqueue_job(conn, &job, now, now).await?;
    }

    Ok(())
}

/// Asks each queue that shows a status on a PR whose head is `sha` to read
/// the PR again: a concluded check can change its conditions.
async fn observe_checked_prs(
    conn: &mut PgConnection,
    delivery: &ConnectionDelivery,
    repo_external_id: &str,
    sha: &str,
    now: OffsetDateTime,
) -> Result<(), MergeQueueError> {
    let Some(repo) = repo_of(conn, delivery, repo_external_id).await? else {
        return Ok(());
    };

    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT c.queue_id, c.pr_number FROM pr_conditions c JOIN queues q ON q.id = c.queue_id
         WHERE c.org_id = $1 AND c.head_sha = $2 AND q.repo_id = $3
         ORDER BY c.queue_id, c.pr_number",
    )
    .bind(delivery.org_id.to_string())
    .bind(sha)
    .bind(repo.to_string())
    .fetch_all(&mut *conn)
    .await?;
    let mut prs_by_queue: BTreeMap<String, Vec<u64>> = BTreeMap::new();

    for (queue, number) in rows {
        prs_by_queue
            .entry(queue)
            .or_default()
            .push(u64::try_from(number).unwrap_or_default());
    }

    for (queue, prs) in prs_by_queue {
        let target = QueueTarget::load(conn, parse_id(&queue, &queue)?).await?;
        let subject = subject(&delivery.delivery_id);
        for job in observe_jobs(conn, &target, &subject, &prs).await? {
            enqueue_job(conn, &job, now, now).await?;
        }
    }

    Ok(())
}

/// Asks each queue of `branch` to read its head again, after a push that
/// the routing gave to no queue because the branch has several. Returns
/// `false` while a merge of one of them is in flight: the push of the merge
/// may come before its fact, so the push waits for the next pass.
async fn read_heads(
    conn: &mut PgConnection,
    delivery: &ConnectionDelivery,
    repo_external_id: &str,
    branch: &str,
    now: OffsetDateTime,
) -> Result<bool, MergeQueueError> {
    let Some(repo) = repo_of(conn, delivery, repo_external_id).await? else {
        return Ok(true);
    };

    let queues = QueueTarget::queues_of_branch(conn, repo, branch).await?;
    for queue in &queues {
        if merge_in_flight(conn, queue.queue_id).await? {
            return Ok(false);
        }
    }

    for queue in &queues {
        let job = refresh_target_after(queue, &delivery.delivery_id);
        enqueue_job(conn, &job, now, now).await?;
        let observed = observed_prs(conn, queue).await?;
        let subject = subject(&delivery.delivery_id);
        for job in observe_jobs(conn, queue, &subject, &observed).await? {
            enqueue_job(conn, &job, now, now).await?;
        }
    }

    Ok(true)
}

/// A `merge` command in a comment on the PR `number`.
struct Command<'a> {
    repo_external_id: &'a str,
    number: u64,
    /// `None` for the submission box, which is not a command to acknowledge.
    comment_external_id: Option<&'a str>,
    author: &'a ScmUser,
    merge: Merge,
}

/// Requests the job that submits the PR of `command`, when CIHub knows its
/// repository.
async fn request_submission(
    conn: &mut PgConnection,
    delivery: &ConnectionDelivery,
    command: &Command<'_>,
    now: OffsetDateTime,
) -> Result<(), MergeQueueError> {
    let Some(repo) = repo_of_command(conn, delivery, command.repo_external_id).await? else {
        return Ok(());
    };

    let request = SubmitPr {
        repo_id: repo,
        pr_number: command.number,
        via: SubmittedVia::Comment,
        requested_by: Some(command.author.into()),
        priority: command.merge.priority,
        no_batch: command.merge.no_batch,
        direct: command.merge.direct,
        queue: command.merge.queue.clone(),
        label: None,
        comment_id: command.comment_external_id.map(str::to_owned),
    };
    let job = submit(
        delivery.org_id,
        delivery.connection,
        &delivery.delivery_id,
        &request,
    );

    enqueue_job(conn, &job, now, now).await?;

    Ok(())
}

/// The scheduler looks for the queues to cover this often.
const COVER_EVERY: time::Duration = time::Duration::hours(1);

/// How often the scheduler looks for the queues whose configuration check
/// is due; often enough that a check comes soon after a new queue.
const CONFIG_CHECKS_SWEEP_EVERY: time::Duration = time::Duration::minutes(5);

/// Marks dirty the queues whose oldest event after their last snapshot is
/// older than [`COVER_AFTER`]: their reconciliation keeps a snapshot, so that
/// the old partitions of `events` can go. Returns their number.
///
/// # Errors
///
/// Postgres fails.
pub async fn wake_uncovered(pool: &PgPool, now: OffsetDateTime) -> Result<usize, MergeQueueError> {
    let mut tx = begin_system(pool).await?;
    // By queue, the events after its snapshot through the index of its log,
    // rather than a scan of the whole log; a queue without a snapshot has
    // its own branch, as `coalesce` would keep the index out.
    let queues: Vec<String> = sqlx::query_scalar(
        "SELECT q.id FROM queues q
         WHERE EXISTS (SELECT 1 FROM events e
                       WHERE e.queue_id = q.id AND e.seq > q.snapshot_seq AND e.occurred_at <= $1)
            OR (q.snapshot_seq IS NULL
                AND EXISTS (SELECT 1 FROM events e
                            WHERE e.queue_id = q.id AND e.occurred_at <= $1))",
    )
    .bind(now - COVER_AFTER)
    .fetch_all(&mut *tx)
    .await?;
    mark_queues_dirty(tx, &queues).await
}

/// Marks dirty each of `queues` in `tx`, then commits it; returns their
/// number.
async fn mark_queues_dirty(
    mut tx: Transaction<'static, Postgres>,
    queues: &[String],
) -> Result<usize, MergeQueueError> {
    for queue in queues {
        mark_dirty(&mut tx, parse_id(queue, queue)?).await?;
    }
    tx.commit().await?;
    Ok(queues.len())
}

/// Marks dirty the queues that have a live item: a plan of an older version
/// may have asked for no wake-up. Returns their number.
///
/// # Errors
///
/// Postgres fails.
pub async fn wake_live_queues(pool: &PgPool) -> Result<usize, MergeQueueError> {
    let mut tx = begin_system(pool).await?;
    let queues: Vec<String> = sqlx::query_scalar(
        "SELECT q.id FROM queues q
         WHERE EXISTS (SELECT 1 FROM queue_items i
                       WHERE i.queue_id = q.id AND i.is_current
                         AND i.state NOT IN ('merged', 'failed', 'cancelled'))
         ORDER BY q.id",
    )
    .fetch_all(&mut *tx)
    .await?;
    mark_queues_dirty(tx, &queues).await
}

/// Wakes the live queues once, then takes the leases of the queues that have
/// work as `owner`, reconciles each one, then waits for a change or for
/// [`TICK`], until `cancel`; then gives its leases back. An error is logged,
/// and the work waits for the next pass.
///
/// # Errors
///
/// Postgres cannot be reached at the start, to listen to the queues.
pub(crate) async fn run_scheduler(
    pool: PgPool,
    owner: String,
    reconciler: Reconciler,
    clock: impl Fn() -> OffsetDateTime + Send + Sync,
    cancel: CancellationToken,
) -> Result<(), MergeQueueError> {
    let mut wakeup = QueueWakeup::listen(&pool).await?;
    match wake_live_queues(&pool).await {
        Ok(queues) => info!(queues, "live queues woken at start"),
        Err(err) => warn!(error = &err as &dyn Error, "live queues not woken at start"),
    }

    let mut next_cover = clock();
    // The checks of a start would compete with the work that waited for it.
    let mut next_config_checks = clock() + CONFIG_CHECKS_SWEEP_EVERY;
    while !cancel.is_cancelled() {
        let now = clock();
        cover_if_due(&pool, now, &mut next_cover).await;
        check_configs_if_due(&pool, now, &mut next_config_checks).await;

        if let Err(err) = pass_connection_deliveries(&pool, &reconciler, now).await {
            warn!(
                error = &err as &dyn Error,
                "deliveries of the connections not given"
            );
        }

        let leased_queues = reconcile_leased_queues(&pool, &owner, &reconciler, now, &clock).await;
        if i64::try_from(leased_queues).unwrap_or(i64::MAX) >= LEASES_PER_PASS {
            continue;
        }

        let wait = next_wait(&pool, clock()).await.unwrap_or_else(|err| {
            warn!(error = &err as &dyn Error, "next tick not read");
            TICK
        });
        wakeup
            .wait_or_pause(wait, &cancel, "queue notifications lost")
            .await;
    }

    let mut tx = begin_system(&pool).await?;
    release_leases(&mut tx, &owner).await?;

    Ok(tx.commit().await?)
}

async fn cover_if_due(pool: &PgPool, now: OffsetDateTime, next_cover: &mut OffsetDateTime) {
    if now < *next_cover {
        return;
    }

    *next_cover = now + COVER_EVERY;
    match wake_uncovered(pool, now).await {
        Ok(0) => {}
        Ok(queues) => info!(queues, "queues woken for their snapshot"),
        Err(err) => warn!(error = &err as &dyn Error, "queues to cover not read"),
    }
}

async fn check_configs_if_due(
    pool: &PgPool,
    now: OffsetDateTime,
    next_checks: &mut OffsetDateTime,
) {
    if now < *next_checks {
        return;
    }

    *next_checks = now + CONFIG_CHECKS_SWEEP_EVERY;
    match request_config_checks(pool, now).await {
        Ok(0) => {}
        Ok(queues) => info!(queues, "checks of the configuration requested"),
        Err(err) => warn!(
            error = &err as &dyn Error,
            "checks of the configuration not requested"
        ),
    }
}

async fn reconcile_leased_queues(
    pool: &PgPool,
    owner: &str,
    reconciler: &Reconciler,
    now: OffsetDateTime,
    clock: &(impl Fn() -> OffsetDateTime + Send + Sync),
) -> usize {
    let queues = match leases(pool, owner, now).await {
        Ok(queues) => queues,
        Err(err) => {
            warn!(error = &err as &dyn Error, "leases not taken");
            Vec::new()
        }
    };

    for queue in &queues {
        let span = info_span!("reconcile", org_id = tracing::field::Empty, queue_id = %queue);
        let done = reconciler
            .reconcile(pool, *queue, clock)
            .instrument(span.clone());
        if let Err(err) = done.await {
            span.in_scope(|| warn!(error = &err as &dyn Error, "queue not reconciled"));
        }
    }

    queues.len()
}

async fn next_wait(
    pool: &PgPool,
    now: OffsetDateTime,
) -> Result<std::time::Duration, MergeQueueError> {
    let mut tx = begin_system(pool).await?;
    let wait = until_next_tick(&mut tx, now).await?;

    tx.commit().await?;

    Ok(wait)
}

async fn leases(
    pool: &PgPool,
    owner: &str,
    now: OffsetDateTime,
) -> Result<Vec<QueueId>, MergeQueueError> {
    let mut tx = begin_system(pool).await?;
    let queues = acquire_leases(&mut tx, owner, now, LEASES_PER_PASS).await?;

    tx.commit().await?;

    Ok(queues)
}

//! The scheduler of the merge queue (`AGENTS.md` D64): it takes the leases of
//! the queues that have work, and reconciles each one in one transaction
//! under its lock. A reconciliation reads the deliveries and the facts of the
//! queue, writes the facts to the log, runs `plan`, writes its decisions, and
//! requests the jobs that make the SCM converge. A crash at any point leaves
//! nothing: the transaction rolls back and the next reconciliation does the
//! same work again.

use std::collections::BTreeMap;
use std::error::Error;
use std::sync::Arc;

use cihub_config::QueueSettings;
use cihub_domain::{
    Actor, ConnectionId, Event, EventKind, OrgId, QueueId, ScmInstanceId, SubmittedVia,
};
use cihub_mq_core::plan;
use cihub_platform::{
    Delivery, Job, QueueWakeup, TICK, Wake, acquire_leases, begin_reconciliation, enqueue_job,
    mark_dirty, mark_processed, pending_deliveries, release_leases, schedule_tick,
};
use cihub_scm::{MAX_PRS_PER_READ, ScmDelivery, ScmEvent, ScmUser};
use cihub_store::{PgPool, begin_system, deposit_fact, mark_facts_processed, pending_facts};
use serde_json::Value;
use sqlx::PgConnection;
use time::OffsetDateTime;
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, info, info_span, warn};

use crate::command::{Merge, parse_merge};
use crate::jobs::{SubmitPr, new_event_id, refresh_after, refresh_at, submit, user_of};
use crate::target::QueueTarget;
use crate::{COVER_AFTER, MergeQueueError, QueueLog};

/// The translation of a delivery of the SCM by its adapter (`AGENTS.md` D84).
pub type Translate =
    Arc<dyn Fn(&str, &Value) -> Result<ScmDelivery, Box<dyn Error + Send + Sync>> + Send + Sync>;

/// A pass takes at most this many leases.
pub const LEASES_PER_PASS: i64 = 50;

/// A reconciliation reads at most this many deliveries and facts; the rest
/// waits for the next one, which the queue, still dirty, asks for at once.
pub const INBOX_PER_RECONCILIATION: i64 = 200;

/// A pass over the deliveries of the connections reads at most this many.
pub const CONNECTION_DELIVERIES_PER_PASS: i64 = 100;

/// What a reconciliation needs besides the queue.
#[derive(Clone)]
pub struct Reconciler {
    pub settings: Arc<QueueSettings>,
    pub translate: Translate,
}

impl Reconciler {
    /// Reconciles `queue` at `now`, in its own transaction. Does nothing when
    /// another transaction reconciles it; an organization without the module
    /// only loses the work of the queue.
    ///
    /// # Errors
    ///
    /// Postgres fails, or the log of the queue does not fit its decisions;
    /// nothing is written.
    pub async fn reconcile(
        &self,
        pool: &PgPool,
        queue: QueueId,
        now: OffsetDateTime,
    ) -> Result<(), MergeQueueError> {
        let Some(mut tx) = begin_reconciliation(pool, queue)
            .await
            .map_err(MergeQueueError::Platform)?
        else {
            return Ok(());
        };
        self.reconcile_in(&mut tx, queue, now).await?;
        tx.commit().await.map_err(MergeQueueError::Query)
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
        if !target.enabled {
            return schedule_tick(conn, queue, None)
                .await
                .map_err(MergeQueueError::Platform);
        }
        let mut log = QueueLog::load(conn, queue).await?;
        let mut facts = Vec::new();
        let mut jobs = Vec::new();
        let deliveries = pending_deliveries(conn, queue, INBOX_PER_RECONCILIATION)
            .await
            .map_err(MergeQueueError::Platform)?;
        for delivery in &deliveries {
            let error = match (self.translate)(&delivery.event, &delivery.payload) {
                Ok(read) => {
                    let work = Work {
                        target: &target,
                        settings: &self.settings,
                        log: &log,
                        delivery,
                    };
                    work.read(conn, read.event, &mut facts, &mut jobs).await?;
                    None
                }
                Err(err) => Some(err.to_string()),
            };
            mark_processed(
                conn,
                delivery.scm_instance_id,
                &delivery.delivery_id,
                now,
                error.as_deref(),
            )
            .await
            .map_err(MergeQueueError::Platform)?;
        }
        let given = pending_facts(conn, queue, INBOX_PER_RECONCILIATION)
            .await
            .map_err(MergeQueueError::Store)?;
        let given_ids: Vec<_> = given.iter().map(|e| e.id).collect();
        facts.extend(given);
        let written = log.write(conn, facts, now).await?.len();
        mark_facts_processed(conn, &given_ids, now)
            .await
            .map_err(MergeQueueError::Store)?;
        let plan = plan(&log.snapshot, &self.settings, now).map_err(MergeQueueError::Plan)?;
        let decisions = log.write(conn, plan.decisions, now).await?.len();
        for (point, prs) in reads_by_point(&plan.desired.observe) {
            for (chunk, prs) in prs.chunks(MAX_PRS_PER_READ).enumerate() {
                jobs.push(refresh_at(&target, point, chunk, prs.to_vec()));
            }
        }
        let mut requested = 0;
        for job in &jobs {
            if enqueue_job(conn, job, now, now)
                .await
                .map_err(MergeQueueError::Platform)?
            {
                requested += 1;
            }
        }
        log.cover(conn, now).await?;
        schedule_tick(conn, queue, plan.wake_at)
            .await
            .map_err(MergeQueueError::Platform)?;
        if written + decisions + requested > 0 {
            info!(
                deliveries = deliveries.len(),
                facts = written,
                decisions,
                jobs = requested,
                "queue reconciled"
            );
        }
        Ok(())
    }
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
    /// what it does not hold. The other events wait for their lots.
    async fn read(
        &self,
        conn: &mut PgConnection,
        event: ScmEvent,
        facts: &mut Vec<Event>,
        jobs: &mut Vec<Job>,
    ) -> Result<(), MergeQueueError> {
        let delivery_id = self.delivery.delivery_id.as_str();
        let snapshot = &self.log.snapshot;
        match event {
            ScmEvent::PrChanged {
                number,
                label_added,
                ..
            } => {
                let tracked = snapshot.current_item(number).is_some()
                    || snapshot.requests.values().any(|r| {
                        matches!(r, cihub_mq_core::Request::Submit { pr_number, .. } if *pr_number == number)
                    });
                if tracked {
                    jobs.push(refresh_after(self.target, delivery_id, vec![number]));
                } else if self.settings.label_commands_enabled.value
                    && label_added.as_deref() == Some(self.settings.enqueue_label.value.as_str())
                {
                    let request = SubmitPr {
                        repo_id: self.target.repo_id,
                        pr_number: number,
                        via: SubmittedVia::Label,
                        requested_by: None,
                        priority: cihub_domain::Priority::MEDIUM,
                        no_batch: false,
                        direct: false,
                    };
                    let connection = (self.target.scm_instance_id, self.target.repo.connection.id);
                    jobs.push(submit(
                        self.target.org_id,
                        connection,
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
                let sender_id = user_of(conn, self.target.scm_instance_id, &sender).await?;
                facts.push(self.fact(
                    Actor::User { user_id: sender_id },
                    EventKind::TargetPushed {
                        before,
                        after,
                        sender_id,
                    },
                ));
            }
            ScmEvent::AccessLost => facts.push(self.fact(Actor::System, EventKind::AccessLost)),
            ScmEvent::AccessRestored => {
                facts.push(self.fact(Actor::System, EventKind::AccessRestored));
            }
            // A comment names no base branch: it comes through the pass of
            // the connection.
            ScmEvent::Pushed { .. }
            | ScmEvent::Commented { .. }
            | ScmEvent::CheckChanged { .. }
            | ScmEvent::RulesChanged { .. }
            | ScmEvent::RepoChanged { .. }
            | ScmEvent::ConnectionCreated { .. }
            | ScmEvent::ReposChanged { .. }
            | ScmEvent::Ignored => {}
        }
        Ok(())
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

/// A delivery of a connection without a queue.
struct ConnectionDelivery {
    instance: ScmInstanceId,
    delivery_id: String,
    org_id: OrgId,
    connection: ConnectionId,
    received_at: OffsetDateTime,
}

/// Reads the deliveries of a connection without a queue (`AGENTS.md` D103),
/// and returns their number. A lost or restored access goes to each queue of
/// the connection; a `merge` command requests its submission, whose job finds
/// the queue by the base branch of the PR. A delivery that no queue uses is
/// marked processed.
///
/// # Errors
///
/// Postgres fails; nothing is written.
pub async fn pass_connection_deliveries(
    pool: &PgPool,
    reconciler: &Reconciler,
    now: OffsetDateTime,
) -> Result<usize, MergeQueueError> {
    type Row = (
        String,
        String,
        String,
        String,
        String,
        Value,
        OffsetDateTime,
    );
    let mut tx = begin_system(pool).await.map_err(MergeQueueError::Store)?;
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT scm_instance_id, delivery_id, org_id, connection_id, event, payload, received_at
         FROM webhook_inbox
         WHERE routed_at IS NOT NULL AND processed_at IS NULL AND queue_id IS NULL
           AND connection_id IS NOT NULL
         ORDER BY received_at, delivery_id
         LIMIT $1
         FOR UPDATE SKIP LOCKED",
    )
    .bind(CONNECTION_DELIVERIES_PER_PASS)
    .fetch_all(&mut *tx)
    .await
    .map_err(MergeQueueError::Query)?;
    let count = rows.len();
    for (instance, delivery_id, org_id, connection, event, payload, received_at) in rows {
        let read = |source| MergeQueueError::ReadQueue {
            id: delivery_id.clone(),
            source,
        };
        let delivery = ConnectionDelivery {
            instance: instance.parse().map_err(read)?,
            org_id: org_id.parse().map_err(read)?,
            connection: connection.parse().map_err(read)?,
            delivery_id: delivery_id.clone(),
            received_at,
        };
        let error = match (reconciler.translate)(&event, &payload) {
            Ok(read) => {
                match read.event {
                    ScmEvent::AccessLost => {
                        give_to_queues(&mut tx, &delivery, EventKind::AccessLost, now).await?;
                    }
                    ScmEvent::AccessRestored => {
                        give_to_queues(&mut tx, &delivery, EventKind::AccessRestored, now).await?;
                    }
                    ScmEvent::Commented {
                        repo_external_id,
                        number,
                        author,
                        body,
                    } => {
                        let merge = parse_merge(&body)
                            .filter(|_| reconciler.settings.commands_enabled.value);
                        if let Some(merge) = merge {
                            let command = Command {
                                repo_external_id: &repo_external_id,
                                number,
                                author: &author,
                                merge,
                            };
                            request_submission(&mut tx, &delivery, &command, now).await?;
                        }
                    }
                    _ => {}
                }
                None
            }
            Err(err) => Some(err.to_string()),
        };
        mark_processed(
            &mut tx,
            delivery.instance,
            &delivery_id,
            now,
            error.as_deref(),
        )
        .await
        .map_err(MergeQueueError::Platform)?;
    }
    tx.commit().await.map_err(MergeQueueError::Query)?;
    Ok(count)
}

/// Deposits the fact `kind` of `delivery` in the inbox of each queue of its
/// connection, and marks them dirty.
async fn give_to_queues(
    conn: &mut PgConnection,
    delivery: &ConnectionDelivery,
    kind: EventKind,
    now: OffsetDateTime,
) -> Result<(), MergeQueueError> {
    let queues: Vec<String> = sqlx::query_scalar(
        "SELECT q.id FROM queues q JOIN repos r ON r.id = q.repo_id
         WHERE r.connection_id = $1 ORDER BY q.id",
    )
    .bind(delivery.connection.to_string())
    .fetch_all(&mut *conn)
    .await
    .map_err(MergeQueueError::Query)?;
    for queue in queues {
        let queue_id: QueueId = queue
            .parse()
            .map_err(|source| MergeQueueError::ReadQueue { id: queue, source })?;
        let fact = Event {
            id: new_event_id(),
            org_id: delivery.org_id,
            queue_id,
            actor: Actor::System,
            occurred_at: delivery.received_at,
            cause_id: None,
            kind: kind.clone(),
        };
        deposit_fact(conn, &fact, now)
            .await
            .map_err(MergeQueueError::Store)?;
        mark_dirty(conn, queue_id)
            .await
            .map_err(MergeQueueError::Platform)?;
    }
    Ok(())
}

/// A `merge` command in a comment on the PR `number`.
struct Command<'a> {
    repo_external_id: &'a str,
    number: u64,
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
    let repo: Option<String> =
        sqlx::query_scalar("SELECT id FROM repos WHERE connection_id = $1 AND external_id = $2")
            .bind(delivery.connection.to_string())
            .bind(command.repo_external_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(MergeQueueError::Query)?;
    let Some(repo) = repo else {
        return Ok(());
    };
    let request = SubmitPr {
        repo_id: repo
            .parse()
            .map_err(|source| MergeQueueError::ReadQueue { id: repo, source })?,
        pr_number: command.number,
        via: SubmittedVia::Comment,
        requested_by: Some(command.author.into()),
        priority: command.merge.priority,
        no_batch: command.merge.no_batch,
        direct: command.merge.direct,
    };
    let connection = (delivery.instance, delivery.connection);
    let job = submit(delivery.org_id, connection, &delivery.delivery_id, &request);
    enqueue_job(conn, &job, now, now)
        .await
        .map_err(MergeQueueError::Platform)?;
    Ok(())
}

/// The scheduler looks for the queues to cover this often (`AGENTS.md` D106).
pub const COVER_EVERY: time::Duration = time::Duration::hours(1);

/// Marks dirty the queues of the organizations with the module whose oldest
/// event after their last snapshot is older than [`COVER_AFTER`]: their
/// reconciliation keeps a snapshot, so that the old partitions of `events`
/// can go. Returns their number.
///
/// # Errors
///
/// Postgres fails.
pub async fn wake_uncovered(pool: &PgPool, now: OffsetDateTime) -> Result<usize, MergeQueueError> {
    let mut tx = begin_system(pool).await.map_err(MergeQueueError::Store)?;
    let queues: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT e.queue_id FROM events e
         JOIN org_modules m ON m.org_id = e.org_id AND m.module = $2::module_name
         WHERE e.occurred_at <= $1
           AND e.seq > coalesce(
               (SELECT max(s.seq) FROM queue_snapshots s WHERE s.queue_id = e.queue_id), 0)",
    )
    .bind(now - COVER_AFTER)
    .bind(cihub_domain::ModuleName::Mergequeue.as_str())
    .fetch_all(&mut *tx)
    .await
    .map_err(MergeQueueError::Query)?;
    for queue in &queues {
        let queue_id: QueueId = queue.parse().map_err(|source| MergeQueueError::ReadQueue {
            id: queue.clone(),
            source,
        })?;
        mark_dirty(&mut tx, queue_id)
            .await
            .map_err(MergeQueueError::Platform)?;
    }
    tx.commit().await.map_err(MergeQueueError::Query)?;
    Ok(queues.len())
}

/// Takes the leases of the queues that have work as `owner`, reconciles each
/// one, then waits for a change or for [`TICK`], until `cancel`; then gives
/// its leases back. An error is logged, and the work waits for the next pass.
///
/// # Errors
///
/// Postgres cannot be reached at the start, to listen to the queues.
pub async fn run_scheduler(
    pool: PgPool,
    owner: String,
    reconciler: Reconciler,
    clock: impl Fn() -> OffsetDateTime + Send + Sync,
    cancel: CancellationToken,
) -> Result<(), MergeQueueError> {
    let mut wakeup = QueueWakeup::listen(&pool)
        .await
        .map_err(MergeQueueError::Platform)?;
    let mut next_cover = clock();
    while !cancel.is_cancelled() {
        let now = clock();
        if now >= next_cover {
            next_cover = now + COVER_EVERY;
            match wake_uncovered(&pool, now).await {
                Ok(0) => {}
                Ok(queues) => info!(queues, "queues woken for their snapshot"),
                Err(err) => warn!(error = &err as &dyn Error, "queues to cover not read"),
            }
        }
        if let Err(err) = pass_connection_deliveries(&pool, &reconciler, now).await {
            warn!(
                error = &err as &dyn Error,
                "deliveries of the connections not given"
            );
        }
        let queues = match leases(&pool, &owner, now).await {
            Ok(queues) => queues,
            Err(err) => {
                warn!(error = &err as &dyn Error, "leases not taken");
                Vec::new()
            }
        };
        for queue in &queues {
            let span = info_span!("reconcile", org_id = tracing::field::Empty, queue_id = %queue);
            let done = reconciler
                .reconcile(&pool, *queue, now)
                .instrument(span.clone());
            if let Err(err) = done.await {
                span.in_scope(|| warn!(error = &err as &dyn Error, "queue not reconciled"));
            }
        }
        if i64::try_from(queues.len()).unwrap_or(i64::MAX) >= LEASES_PER_PASS {
            continue;
        }
        match wakeup.wait(TICK, &cancel).await {
            Ok(Wake::Notified | Wake::Tick | Wake::Cancelled) => {}
            Err(err) => {
                warn!(error = &err as &dyn Error, "queue notifications lost");
                tokio::select! {
                    () = cancel.cancelled() => {}
                    () = tokio::time::sleep(TICK) => {}
                }
            }
        }
    }
    let mut tx = begin_system(&pool).await.map_err(MergeQueueError::Store)?;
    release_leases(&mut tx, &owner)
        .await
        .map_err(MergeQueueError::Platform)?;
    tx.commit().await.map_err(MergeQueueError::Query)
}

async fn leases(
    pool: &PgPool,
    owner: &str,
    now: OffsetDateTime,
) -> Result<Vec<QueueId>, MergeQueueError> {
    let mut tx = begin_system(pool).await.map_err(MergeQueueError::Store)?;
    let queues = acquire_leases(&mut tx, owner, now, LEASES_PER_PASS)
        .await
        .map_err(MergeQueueError::Platform)?;
    tx.commit().await.map_err(MergeQueueError::Query)?;
    Ok(queues)
}

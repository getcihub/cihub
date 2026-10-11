//! The jobs of the merge queue that read the PRs: `refresh_prs` reads PRs
//! again, `submit_pr` submits a PR after a command or a label; the jobs of
//! the test cycle are in `assembly`. Each one gives its facts back to the
//! queue through `queue_inbox` and marks the queue dirty; it never writes the
//! log.

use std::collections::HashMap;
use std::error::Error;

use cihub_config::{QueueSettings, RepoConfig};
use cihub_domain::{
    Actor, ConnectionId, Event, EventId, EventKind, ItemState, JobClass, Mergeability, OrgId,
    Permission, PrState, Priority, QueueId, RepoId, ReviewDecision, SubmittedVia, UserId,
};
use cihub_platform::{Job, Outcome, PlatformError, error_chain, mark_dirty, notify_queue_changed};
use cihub_scm::{
    MAX_PRS_PER_READ, PrSnapshot, RepoRef, RequiredChecks, Scm, ScmError, ScmUser, check_passed,
};
use cihub_store::{PgPool, StoreError, begin_system, deposit_fact, ensure_user};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgConnection;
use time::OffsetDateTime;
use tracing::{info, warn};

use crate::MergeQueueError;
use crate::admission::{Reading, Sender, routes_to, rule_priority, unmet_conditions};
use crate::config::read_config_file;
use crate::target::{QueueTarget, RepoTarget};

pub const REFRESH_PRS: &str = "refresh_prs";
pub(crate) const SUBMIT_PR: &str = "submit_pr";

/// The PRs that `refresh_prs` reads, at most
/// [`MAX_PRS_PER_READ`](cihub_scm::MAX_PRS_PER_READ).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct RefreshPrs {
    pub prs: Vec<u64>,
}

/// An account of the SCM, as a job payload keeps it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Account {
    pub external_id: String,
    pub login: String,
}

impl From<&ScmUser> for Account {
    fn from(user: &ScmUser) -> Self {
        Self {
            external_id: user.external_id.clone(),
            login: user.login.clone(),
        }
    }
}

impl From<&Account> for ScmUser {
    fn from(account: &Account) -> Self {
        Self {
            external_id: account.external_id.clone(),
            login: account.login.clone(),
        }
    }
}

/// A submission that waits for its checks: the permission of the author of
/// a command, the label of a label submission.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SubmitPr {
    /// The repository of the PR: the job finds the queue by the base branch
    /// that it reads, since a comment does not give it.
    pub repo_id: RepoId,
    pub pr_number: u64,
    pub via: SubmittedVia,
    /// The author of the command; `None` for a label, whose sender the
    /// delivery does not give.
    pub requested_by: Option<Account>,
    /// `None` leaves the priority to the `priority_rules`.
    pub priority: Option<Priority>,
    pub no_batch: bool,
    pub direct: bool,
    /// The queue that the command names.
    pub queue: Option<String>,
    /// The label that the action put, for a submission by label.
    pub label: Option<String>,
    /// The comment of the command, which the job acknowledges once the
    /// queue has the submission.
    pub comment_id: Option<String>,
}

impl SubmitPr {
    /// The submission of the PR `pr_number` of `repo_id` by the label
    /// `label`: no author, and the priority of the rules.
    pub(crate) fn by_label(repo_id: RepoId, pr_number: u64, label: String) -> Self {
        Self {
            repo_id,
            pr_number,
            via: SubmittedVia::Label,
            requested_by: None,
            priority: None,
            no_batch: false,
            direct: false,
            queue: None,
            label: Some(label),
            comment_id: None,
        }
    }
}

/// A subject of a key has no `:`, which separates the parts of the key.
pub(crate) fn subject(text: &str) -> String {
    text.replace(':', "-")
}

pub(crate) fn job(
    target: &QueueTarget,
    key: String,
    job_type: &str,
    class: JobClass,
    payload: serde_json::Value,
) -> Job {
    Job {
        idempotency_key: key,
        job_type: job_type.to_owned(),
        class,
        org_id: Some(target.org_id),
        connection: Some(target.repo.connection.id),
        queue_id: Some(target.queue_id),
        payload,
        attempts: 0,
    }
}

/// A job of the connection `connection` after the delivery `delivery_id`,
/// without a queue yet: the job finds it.
pub(crate) fn connection_job(
    org_id: OrgId,
    connection: ConnectionId,
    delivery_id: &str,
    job_type: &str,
    payload: serde_json::Value,
) -> Job {
    Job {
        idempotency_key: format!("{job_type}:{}:1", subject(delivery_id)),
        job_type: job_type.to_owned(),
        class: JobClass::Interactive,
        org_id: Some(org_id),
        connection: Some(connection),
        queue_id: None,
        payload,
        attempts: 0,
    }
}

/// A read of `prs` after the delivery `delivery_id`.
pub(crate) fn refresh_after(target: &QueueTarget, delivery_id: &str, prs: Vec<u64>) -> Job {
    let key = format!(
        "{REFRESH_PRS}:{}-{}:1",
        target.queue_id,
        subject(delivery_id)
    );
    let payload = json!(RefreshPrs { prs });
    job(target, key, REFRESH_PRS, JobClass::Interactive, payload)
}

/// The reads of `prs`, due at the read point `point`.
pub(crate) fn refresh_at(target: &QueueTarget, point: OffsetDateTime, prs: &[u64]) -> Vec<Job> {
    refresh_reads(target, &window(point), JobClass::Repair, prs)
}

/// The reads of `prs` under the subject `subject`, by chunks of
/// [`MAX_PRS_PER_READ`]; the chunks after the first one add their rank to
/// the key.
pub(crate) fn refresh_reads(
    target: &QueueTarget,
    subject: &str,
    class: JobClass,
    prs: &[u64],
) -> Vec<Job> {
    prs.chunks(MAX_PRS_PER_READ)
        .enumerate()
        .map(|(chunk, prs)| {
            let rank = if chunk == 0 {
                String::new()
            } else {
                format!("-{}", chunk + 1)
            };
            let key = format!("{REFRESH_PRS}:{}-{subject}{rank}:1", target.queue_id);
            let payload = json!(RefreshPrs { prs: prs.to_vec() });
            job(target, key, REFRESH_PRS, class, payload)
        })
        .collect()
}

/// The window of a key of a read: the minute of its read point,
/// `AAAAMMJJTHHMM`.
pub(crate) fn window(point: OffsetDateTime) -> String {
    format!(
        "{:04}{:02}{:02}T{:02}{:02}",
        point.year(),
        u8::from(point.month()),
        point.day(),
        point.hour(),
        point.minute()
    )
}

/// The submission of `request` after the delivery `delivery_id`, on the
/// connection `connection`. It has no queue yet.
pub(crate) fn submit(
    org_id: OrgId,
    connection: ConnectionId,
    delivery_id: &str,
    request: &SubmitPr,
) -> Job {
    connection_job(org_id, connection, delivery_id, SUBMIT_PR, json!(request))
}

/// Answers `command` on its PR, to tell its author why the queue refused it.
pub(crate) async fn reply(
    scm: &dyn Scm,
    repo: &RepoRef,
    number: u64,
    author: &Account,
    command: &str,
    text: String,
    class: JobClass,
) -> Result<(), Failure> {
    let body = format!(
        "> {command}\n\n@{}, the queue refused this command: {text}\n",
        author.login
    );
    scm.create_comment(repo, number, &body, class).await?;
    Ok(())
}

/// Acknowledges the comment of a command that the queue accepted. A failure
/// is only logged: a retry of the job would give its facts to the queue a
/// second time.
pub(crate) async fn acknowledge(
    scm: &dyn Scm,
    repo: &RepoRef,
    comment_id: Option<&str>,
    class: JobClass,
) {
    let Some(comment_id) = comment_id else {
        return;
    };
    if let Err(err) = scm.acknowledge_comment(repo, comment_id, class).await {
        warn!(
            error = &err as &dyn Error,
            comment_id, "command not acknowledged"
        );
    }
}

/// The commit status of the queue on the head of a PR (`specs/GITHUB.md`).
pub fn queue_status_context(target_branch: &str) -> String {
    format!("CIHub Merge Queue ({target_branch})")
}

/// The mergeability that the queue sees: a PR blocked only by the status of the
/// queue itself is mergeable, otherwise a repository that requires this status
/// could never enter the queue.
pub(crate) fn mergeability(pr: &PrSnapshot, required: &RequiredChecks, own: &str) -> Mergeability {
    if pr.mergeability != Mergeability::Blocked || !required.names.contains(own) {
        return pr.mergeability;
    }

    if matches!(
        pr.review_decision,
        ReviewDecision::ChangesRequested | ReviewDecision::ReviewRequired
    ) {
        return Mergeability::Blocked;
    }

    let others_pass = required
        .names
        .iter()
        .filter(|name| *name != own)
        .all(|name| check_passed(pr, name));

    if others_pass && !check_passed(pr, own) {
        Mergeability::Mergeable
    } else {
        Mergeability::Blocked
    }
}

fn observed(
    pr: &PrSnapshot,
    required: &RequiredChecks,
    own: &str,
    unmet: Vec<String>,
) -> EventKind {
    EventKind::PrObserved {
        pr_number: pr.number,
        state: pr.state,
        head_sha: pr.head_sha.clone(),
        base_branch: pr.base_branch.clone(),
        draft: pr.draft,
        labels: pr.labels.clone(),
        mergeability: mergeability(pr, required, own),
        review_decision: pr.review_decision,
        checks: pr.checks.clone(),
        unmet_conditions: unmet,
        required_checks: required
            .names
            .iter()
            .filter(|name| *name != own)
            .cloned()
            .collect(),
    }
}

/// A fact comes from outside the log: its identifier is new.
pub(crate) fn new_event_id() -> EventId {
    #[expect(
        clippy::disallowed_methods,
        reason = "a fact that a job or a delivery gives has a new identifier"
    )]
    EventId::new()
}

pub(crate) async fn user_of(
    conn: &mut PgConnection,
    user: &ScmUser,
) -> Result<UserId, MergeQueueError> {
    #[expect(
        clippy::disallowed_methods,
        reason = "the identifier of an account of the SCM, used only when it is new"
    )]
    let candidate = UserId::new();
    ensure_user(conn, candidate, &user.external_id, &user.login)
        .await
        .map_err(MergeQueueError::Store)
}

/// Deposits `kinds` in the inbox of the queue of `target`, then marks the
/// queue dirty.
pub(crate) async fn give_back(
    conn: &mut PgConnection,
    org_id: OrgId,
    queue_id: QueueId,
    facts: Vec<(Actor, EventKind)>,
    now: OffsetDateTime,
) -> Result<(), MergeQueueError> {
    for (actor, kind) in facts {
        let event = Event {
            id: new_event_id(),
            org_id,
            queue_id,
            actor,
            occurred_at: now,
            cause_id: None,
            kind,
        };
        deposit_fact(conn, &event, now).await?;
    }
    mark_dirty(conn, queue_id)
        .await
        .map_err(MergeQueueError::Platform)
}

/// Why a job failed, and whether a retry can fix it.
pub(crate) enum Failure {
    Retry(String),
    Never(String),
}

impl From<MergeQueueError> for Failure {
    fn from(err: MergeQueueError) -> Self {
        Self::Retry(error_chain(&err))
    }
}

impl From<StoreError> for Failure {
    fn from(err: StoreError) -> Self {
        MergeQueueError::Store(err).into()
    }
}

impl From<sqlx::Error> for Failure {
    fn from(err: sqlx::Error) -> Self {
        MergeQueueError::Query(err).into()
    }
}

impl From<PlatformError> for Failure {
    fn from(err: PlatformError) -> Self {
        MergeQueueError::Platform(err).into()
    }
}

impl From<ScmError> for Failure {
    fn from(err: ScmError) -> Self {
        let text = error_chain(&err);
        match err {
            ScmError::Forbidden(_) | ScmError::Validation(_) => Self::Never(text),
            _ => Self::Retry(text),
        }
    }
}

pub(crate) fn outcome(result: Result<(), Failure>) -> Outcome {
    match result {
        Ok(()) => Outcome::Done,
        Err(Failure::Retry(text)) => Outcome::Retry(text),
        Err(Failure::Never(text)) => Outcome::Failed(text),
    }
}

pub(crate) fn read_payload<T: for<'de> Deserialize<'de>>(job: &Job) -> Result<T, Failure> {
    serde_json::from_value(job.payload.clone()).map_err(|err| Failure::Never(error_chain(&err)))
}

pub(crate) async fn target_of(pool: &PgPool, job: &Job) -> Result<QueueTarget, Failure> {
    let queue = job
        .queue_id
        .ok_or_else(|| Failure::Never("the job has no queue".into()))?;
    load_target(pool, queue).await
}

/// The queue `queue`, read in its own transaction.
pub(crate) async fn load_target(pool: &PgPool, queue: QueueId) -> Result<QueueTarget, Failure> {
    let mut tx = begin_system(pool).await?;
    let target = QueueTarget::load(&mut tx, queue).await?;

    tx.commit().await?;

    Ok(target)
}

/// The repository `repo`, read in its own transaction.
pub(crate) async fn load_repo(pool: &PgPool, repo: RepoId) -> Result<RepoTarget, Failure> {
    let mut tx = begin_system(pool).await?;
    let target = RepoTarget::load(&mut tx, repo).await?;

    tx.commit().await?;

    Ok(target)
}

/// Gives `facts` back to the queue of `target`, in their own transaction.
pub(crate) async fn give_back_facts(
    pool: &PgPool,
    target: &QueueTarget,
    facts: Vec<(Actor, EventKind)>,
    now: OffsetDateTime,
) -> Result<(), Failure> {
    let mut tx = begin_system(pool).await?;
    give_back(&mut tx, target.org_id, target.queue_id, facts, now).await?;
    tx.commit().await?;

    Ok(())
}

/// Gives back the facts of the PRs read at `now`, with their titles, for the
/// pages of the dashboard: an older read does not replace a newer title, and
/// a title that did not change is not written again.
async fn give_back_with_titles(
    pool: &PgPool,
    target: &QueueTarget,
    prs: &[PrSnapshot],
    facts: Vec<(Actor, EventKind)>,
    now: OffsetDateTime,
) -> Result<(), Failure> {
    let mut tx = begin_system(pool).await?;

    for pr in prs {
        sqlx::query(
            "INSERT INTO pr_titles (org_id, repo_id, pr_number, title, read_at)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (repo_id, pr_number) DO UPDATE SET title = $4, read_at = $5
             WHERE pr_titles.read_at <= $5 AND pr_titles.title <> $4",
        )
        .bind(target.org_id.to_string())
        .bind(target.repo_id.to_string())
        .bind(i64::try_from(pr.number).unwrap_or(i64::MAX))
        .bind(&pr.title)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    }

    give_back(&mut tx, target.org_id, target.queue_id, facts, now).await?;
    tx.commit().await?;

    Ok(())
}

/// Reads the PRs of the job and gives back one `pr.observed` per PR. A PR
/// that no longer exists gives nothing.
pub(crate) async fn refresh(
    pool: &PgPool,
    scm: &dyn Scm,
    job: &Job,
    now: OffsetDateTime,
) -> Result<(), Failure> {
    let request: RefreshPrs = read_payload(job)?;
    let target = target_of(pool, job).await?;
    let required = scm
        .required_checks(&target.repo, &target.target_branch, job.class)
        .await?;
    let prs = scm
        .pull_requests(&target.repo, &request.prs, job.class)
        .await?;
    let own = queue_status_context(&target.target_branch);
    let mut tx = begin_system(pool).await?;
    let submissions = submissions(&mut tx, target.queue_id, &request.prs).await?;

    tx.commit().await?;

    let config = target.config()?;
    let settings = target.settings_in(config.as_ref())?;
    let mut facts = Vec::with_capacity(prs.len());

    for pr in &prs {
        // The new submission comes before the observation, which cancels the
        // old item: the queue keeps the observation for the new one.
        let resubmitted = submissions
            .get(&pr.number)
            .and_then(|submission| resubmission(&settings, &target, pr, submission));
        facts.extend(resubmitted);
        // An admitted item no longer depends on its conditions.
        let unmet = match submissions.get(&pr.number) {
            Some(Submission { admitted: true, .. }) => Vec::new(),
            submission => {
                let sender = submission.map(|s| s.sender.clone()).unwrap_or_default();
                let reading = Reading {
                    sender: &sender,
                    rules: &required,
                    own_status: &own,
                    class: job.class,
                    now,
                };
                unmet_conditions(scm, &target, config.as_ref(), pr, &reading).await?
            }
        };
        let kind = observed(pr, &required, &own, unmet);
        facts.push((Actor::System, kind));
    }

    give_back_with_titles(pool, &target, &prs, facts, now).await
}

/// The live item of a PR, as `refresh_prs` needs it.
struct Submission {
    /// The item left `not_ready`.
    admitted: bool,
    sender: Sender,
    /// What a new submission of the PR keeps.
    head_sha: String,
    priority: Priority,
    no_batch: bool,
    direct: bool,
    via: SubmittedVia,
    submitted_by: Option<UserId>,
    author_id: UserId,
}

#[derive(sqlx::FromRow)]
struct SubmissionRow {
    pr_number: i64,
    state: String,
    login: Option<String>,
    permission: Option<String>,
    head_sha: String,
    priority: i16,
    no_batch: bool,
    direct: bool,
    via: String,
    submitted_by: Option<String>,
    author_id: String,
}

impl SubmissionRow {
    fn decode(self) -> Option<(u64, Submission)> {
        let submission = Submission {
            admitted: self.state != ItemState::NotReady.as_str(),
            sender: Sender {
                login: self.login,
                permission: self.permission.and_then(|p| p.parse().ok()),
            },
            head_sha: self.head_sha,
            priority: Priority(u8::try_from(self.priority).ok()?),
            no_batch: self.no_batch,
            direct: self.direct,
            via: self.via.parse().ok()?,
            submitted_by: self.submitted_by.and_then(|id| id.parse().ok()),
            author_id: self.author_id.parse().ok()?,
        };
        Some((u64::try_from(self.pr_number).ok()?, submission))
    }
}

/// The live items of `prs` in `queue`, with who submitted them.
async fn submissions(
    conn: &mut PgConnection,
    queue: QueueId,
    prs: &[u64],
) -> Result<HashMap<u64, Submission>, MergeQueueError> {
    let numbers: Vec<i64> = prs.iter().filter_map(|n| i64::try_from(*n).ok()).collect();
    let rows: Vec<SubmissionRow> = sqlx::query_as(
        "SELECT i.pr_number, i.state::text AS state, u.login,
                i.sender_permission::text AS permission, i.pr_head_sha AS head_sha,
                i.priority_value AS priority, i.no_batch, i.force_enqueued AS direct,
                i.submitted_via::text AS via, i.submitted_by, i.pr_author_id AS author_id
         FROM queue_items i LEFT JOIN users u ON u.id = i.submitted_by
         WHERE i.queue_id = $1 AND i.is_current
           AND i.state NOT IN ('merged', 'failed', 'cancelled')
           AND i.pr_number = ANY($2)",
    )
    .bind(queue.to_string())
    .bind(numbers)
    .fetch_all(conn)
    .await?;
    Ok(rows.into_iter().filter_map(SubmissionRow::decode).collect())
}

/// The new submission of a PR in the queue that has a new head, when the
/// queue resubmits on a push: same priority, source and options. A PR that
/// is closed, a draft or on another base leaves the queue instead, and a
/// submission by label needs the label still.
fn resubmission(
    settings: &QueueSettings,
    target: &QueueTarget,
    pr: &PrSnapshot,
    submission: &Submission,
) -> Option<(Actor, EventKind)> {
    let pushed = pr.state == PrState::Open
        && !pr.draft
        && pr.base_branch == target.target_branch
        && pr.head_sha != submission.head_sha;
    let labelled =
        submission.via != SubmittedVia::Label || pr.labels.contains(&settings.enqueue_label.value);

    if !(settings.resubmit_on_push.value && pushed && labelled) {
        return None;
    }

    let actor = submission
        .submitted_by
        .map_or(Actor::System, |user_id| Actor::User { user_id });
    let kind = EventKind::ItemSubmitted {
        pr_number: pr.number,
        head_sha: pr.head_sha.clone(),
        priority: submission.priority,
        no_batch: submission.no_batch,
        direct: submission.direct,
        submitted_via: submission.via,
        author_id: submission.author_id,
        additions: pr.additions,
        deletions: pr.deletions,
        changed_files: pr.changed_files,
        sender_permission: submission.sender.permission,
    };

    Some((actor, kind))
}

/// The author of the command of `request` with their permission, or `None`
/// when the permission is too low (`write`, `admin` for `--direct`). A label
/// gives no sender.
async fn sender_of(
    scm: &dyn Scm,
    repo: &RepoTarget,
    request: &SubmitPr,
    class: JobClass,
) -> Result<Option<Sender>, Failure> {
    let Some(author) = &request.requested_by else {
        return Ok(Some(Sender::default()));
    };

    let need = if request.direct {
        Permission::Admin
    } else {
        Permission::Write
    };
    let has = scm
        .permission(&repo.repo, &ScmUser::from(author), class)
        .await?;

    Ok((has >= need).then(|| Sender {
        login: Some(author.login.clone()),
        permission: Some(has),
    }))
}

/// The default branch of `repo`.
async fn default_branch_of(pool: &PgPool, repo: RepoId) -> Result<String, Failure> {
    let mut tx = begin_system(pool).await?;
    let default_branch = sqlx::query_scalar("SELECT default_branch FROM repos WHERE id = $1")
        .bind(repo.to_string())
        .fetch_one(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(default_branch)
}

/// The queues of `branch` that take submissions, in the order of the
/// routing.
async fn open_queues(
    pool: &PgPool,
    repo: RepoId,
    branch: &str,
    default_branch: &str,
) -> Result<Vec<QueueTarget>, Failure> {
    let mut tx = begin_system(pool).await?;
    let queues = QueueTarget::queues_of_branch(&mut tx, repo, branch).await?;

    tx.commit().await?;

    let mut open = Vec::with_capacity(queues.len());
    for queue in queues {
        if queue.takes_submissions(default_branch)? {
            open.push(queue);
        }
    }

    Ok(open)
}

/// Creates the queues of `branch`: the ones that the configuration file
/// names on the head of the branch, or else, on the default branch, the
/// queue named after it. Each one keeps the file that it comes from, so that
/// the routing of the submission reads its conditions.
async fn create_queues(
    pool: &PgPool,
    scm: &dyn Scm,
    repo: &RepoTarget,
    branch: &str,
    default_branch: &str,
    class: JobClass,
) -> Result<(), Failure> {
    let Some(head) = scm.branch_head(&repo.repo, branch, class).await? else {
        return Ok(());
    };

    let read = read_config_file(scm, &repo.repo, &head, class).await?;
    let checked = read.and_then(|text| {
        text.map(|text| RepoConfig::parse(&text).map(|config| (text, config)))
            .transpose()
    });
    let mut names: Vec<String> = match &checked {
        Ok(Some((_, config))) => config
            .queues
            .iter()
            .filter(|queue| queue.branch == branch)
            .map(|queue| queue.name.clone())
            .collect(),
        _ => Vec::new(),
    };

    if names.is_empty() && branch == default_branch {
        names.push(branch.to_owned());
    }

    let (text, error) = match checked {
        Ok(found) => (found.map(|(text, _)| text), None),
        Err(err) => (None, Some(err.to_string())),
    };
    let mut tx = begin_system(pool).await?;

    for name in names {
        #[expect(
            clippy::disallowed_methods,
            reason = "a new queue gets a new identifier"
        )]
        let id = QueueId::new();
        let inserted = sqlx::query(
            "INSERT INTO queues (id, org_id, repo_id, target_branch, name, dirty,
                                 config_text, config_sha, config_error)
             VALUES ($1, $2, $3, $4, $5, true, $6, $7, $8)
             ON CONFLICT (repo_id, name) DO NOTHING",
        )
        .bind(id.to_string())
        .bind(repo.org_id.to_string())
        .bind(repo.repo_id.to_string())
        .bind(branch)
        .bind(&name)
        .bind(&text)
        .bind(&head)
        .bind(&error)
        .execute(&mut *tx)
        .await?;
        if inserted.rows_affected() == 1 {
            info!(queue_name = name, target_branch = branch, "queue created");
            mark_dirty(&mut tx, id).await?;
            notify_queue_changed(&mut tx, repo.org_id, id).await?;
        }
    }

    tx.commit().await?;

    Ok(())
}

/// The queue that `pr` enters: the queue that the command names, or else the
/// first queue of the branch whose `route_conditions` it meets, or else the
/// first one. The `queue_conditions` do not route: they only admit. `None`
/// when the command names a queue that the branch does not have.
async fn route<'a>(
    scm: &dyn Scm,
    queues: &'a [QueueTarget],
    pr: &PrSnapshot,
    request: &SubmitPr,
    reading: &Reading<'_>,
) -> Result<Option<&'a QueueTarget>, Failure> {
    if let Some(name) = &request.queue {
        return Ok(queues.iter().find(|queue| queue.name == *name));
    }
    for queue in queues {
        if routes_to(scm, queue, queue.config()?.as_ref(), pr, reading).await? {
            return Ok(Some(queue));
        }
    }
    Ok(queues.first())
}

/// Why the queue refuses the submission, if it does: a command while the
/// commands are off, a label that is not the label of the queue, or that the
/// PR no longer has.
fn refusal(settings: &QueueSettings, request: &SubmitPr, pr: &PrSnapshot) -> Option<&'static str> {
    let label = &settings.enqueue_label.value;
    match request.via {
        SubmittedVia::Comment if !settings.commands_enabled.value => Some("commands disabled"),
        SubmittedVia::Label
            if !settings.label_commands_enabled.value || request.label.as_ref() != Some(label) =>
        {
            Some("not the label of the queue")
        }
        SubmittedVia::Label if !pr.labels.contains(label) => Some("label removed"),
        _ => None,
    }
}

/// What the PR is, when it cannot enter a queue: only an open PR that is not a
/// draft can.
fn not_open(pr: &PrSnapshot) -> Option<&'static str> {
    match pr.state {
        PrState::Merged => Some("merged"),
        PrState::Closed => Some("closed"),
        PrState::Open if pr.draft => Some("a draft"),
        PrState::Open => None,
    }
}

async fn find_pull_request(
    scm: &dyn Scm,
    repo: &RepoTarget,
    pr_number: u64,
    class: JobClass,
) -> Result<Option<PrSnapshot>, Failure> {
    match scm.pull_request(&repo.repo, pr_number, class).await {
        Ok(pr) => Ok(Some(pr)),
        Err(ScmError::NotFound(_)) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

async fn queues_of_pr(
    pool: &PgPool,
    scm: &dyn Scm,
    repo: &RepoTarget,
    pr: &PrSnapshot,
    request: &SubmitPr,
    class: JobClass,
) -> Result<Vec<QueueTarget>, Failure> {
    let default_branch = default_branch_of(pool, repo.repo_id).await?;
    let queues = open_queues(pool, repo.repo_id, &pr.base_branch, &default_branch).await?;

    // A label is a setting of a queue: only a command creates one.
    if !queues.is_empty() || request.via != SubmittedVia::Comment {
        return Ok(queues);
    }

    create_queues(pool, scm, repo, &pr.base_branch, &default_branch, class).await?;

    open_queues(pool, repo.repo_id, &pr.base_branch, &default_branch).await
}

async fn submission_priority(
    scm: &dyn Scm,
    target: &QueueTarget,
    config: Option<&RepoConfig>,
    pr: &PrSnapshot,
    request: &SubmitPr,
    reading: &Reading<'_>,
) -> Result<Priority, Failure> {
    if let Some(priority) = request.priority {
        return Ok(priority);
    }

    let rule = rule_priority(scm, target, config, pr, reading).await?;
    Ok(rule.unwrap_or(Priority::MEDIUM))
}

async fn submission_accounts(
    pool: &PgPool,
    pr: &PrSnapshot,
    request: &SubmitPr,
) -> Result<(UserId, Actor), Failure> {
    // The accounts come in their own transaction: the one that gives the
    // facts back waits for the reconciliation of the queue, which may need
    // the same rows of `users`.
    let mut tx = begin_system(pool).await?;
    let author_id = user_of(&mut tx, &pr.author).await?;
    let actor = match &request.requested_by {
        Some(account) => Actor::User {
            user_id: user_of(&mut tx, &account.into()).await?,
        },
        None => Actor::System,
    };

    tx.commit().await?;

    Ok((author_id, actor))
}

/// Answers the refused submission `request`: a command or a checkbox gets the
/// cause, a label nothing, since its delivery does not give its author.
async fn refuse_submission(
    scm: &dyn Scm,
    repo: &RepoRef,
    request: &SubmitPr,
    text: String,
    class: JobClass,
) -> Result<(), Failure> {
    let Some(author) = &request.requested_by else {
        return Ok(());
    };

    reply(
        scm,
        repo,
        request.pr_number,
        author,
        "/cihub merge",
        text,
        class,
    )
    .await
}

/// Checks the submission of the job, reads the PR, and gives back its
/// `item.submitted` and its `pr.observed`. A refused command gets a comment
/// that says why; a refused label gets none.
pub(crate) async fn submit_pr(
    pool: &PgPool,
    scm: &dyn Scm,
    job: &Job,
    now: OffsetDateTime,
) -> Result<(), Failure> {
    let request: SubmitPr = read_payload(job)?;
    let repo = load_repo(pool, request.repo_id).await?;
    let pr_number = request.pr_number;
    let repo_ref = &repo.repo;
    let refuse = |text: String| refuse_submission(scm, repo_ref, &request, text, job.class);

    let Some(sender) = sender_of(scm, &repo, &request, job.class).await? else {
        info!(pr_number, "submission refused: permission");
        return refuse(
            "the command needs the write permission on the repository (admin with `--direct`)."
                .to_owned(),
        )
        .await;
    };

    let Some(pr) = find_pull_request(scm, &repo, pr_number, job.class).await? else {
        return Ok(());
    };

    if let Some(state) = not_open(&pr) {
        info!(pr_number, state, "submission refused: PR not open");
        return refuse(format!("this PR is {state}.")).await;
    }

    let queues = queues_of_pr(pool, scm, &repo, &pr, &request, job.class).await?;
    let Some(first) = queues.first() else {
        info!(
            pr_number,
            base_branch = pr.base_branch,
            "submission dropped: no queue"
        );
        return refuse(format!(
            "no queue serves the branch `{}`; a queue of `.cihub.yaml` on this branch would.",
            pr.base_branch
        ))
        .await;
    };

    let required = scm
        .required_checks(&first.repo, &first.target_branch, job.class)
        .await?;
    let own = queue_status_context(&first.target_branch);
    let reading = Reading {
        sender: &sender,
        rules: &required,
        own_status: &own,
        class: job.class,
        now,
    };

    let Some(target) = route(scm, &queues, &pr, &request, &reading).await? else {
        info!(pr_number, "submission dropped: no queue of that name");
        return refuse(format!(
            "the branch `{}` has no queue named `{}`.",
            pr.base_branch,
            request.queue.as_deref().unwrap_or_default()
        ))
        .await;
    };

    let config = target.config()?;
    if let Some(reason) = refusal(&target.settings_in(config.as_ref())?, &request, &pr) {
        info!(pr_number, reason, "submission dropped");
        return refuse(format!(
            "the commands are off for the queue `{}`.",
            target.name
        ))
        .await;
    }

    let unmet = unmet_conditions(scm, target, config.as_ref(), &pr, &reading).await?;
    let priority =
        submission_priority(scm, target, config.as_ref(), &pr, &request, &reading).await?;
    let (author_id, actor) = submission_accounts(pool, &pr, &request).await?;

    let submitted = EventKind::ItemSubmitted {
        pr_number,
        head_sha: pr.head_sha.clone(),
        priority,
        no_batch: request.no_batch,
        direct: request.direct,
        submitted_via: request.via,
        author_id,
        additions: pr.additions,
        deletions: pr.deletions,
        changed_files: pr.changed_files,
        sender_permission: sender.permission,
    };
    let facts = vec![
        (actor, submitted),
        (Actor::System, observed(&pr, &required, &own, unmet)),
    ];

    give_back_with_titles(pool, target, std::slice::from_ref(&pr), facts, now).await?;
    acknowledge(scm, repo_ref, request.comment_id.as_deref(), job.class).await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::target::{test_pr as pr, test_target};

    #[test]
    fn a_pr_blocked_only_by_the_queue_status_is_mergeable() {
        let own = queue_status_context("main");
        assert_eq!(own, "CIHub Merge Queue (main)");

        let required = RequiredChecks {
            names: BTreeSet::from(["ci".to_owned(), own.clone()]),
            up_to_date: false,
        };
        let not_required = RequiredChecks::default();
        let blocked = Mergeability::Blocked;
        let approved = ReviewDecision::Approved;
        let cases = [
            (
                pr(blocked, approved, &[("ci", true)]),
                &required,
                Mergeability::Mergeable,
            ),
            (
                pr(blocked, ReviewDecision::None, &[("ci", true)]),
                &required,
                Mergeability::Mergeable,
            ),
            (pr(blocked, approved, &[("ci", false)]), &required, blocked),
            (pr(blocked, approved, &[]), &required, blocked),
            (
                pr(blocked, ReviewDecision::ReviewRequired, &[("ci", true)]),
                &required,
                blocked,
            ),
            (
                pr(blocked, ReviewDecision::ChangesRequested, &[("ci", true)]),
                &required,
                blocked,
            ),
            // The queue status passes: something else blocks the PR.
            (
                pr(blocked, approved, &[("ci", true), (own.as_str(), true)]),
                &required,
                blocked,
            ),
            (
                pr(blocked, approved, &[("ci", true)]),
                &not_required,
                blocked,
            ),
            (
                pr(Mergeability::Behind, approved, &[]),
                &required,
                Mergeability::Behind,
            ),
        ];

        for (n, (pr, required, want)) in cases.into_iter().enumerate() {
            assert_eq!(mergeability(&pr, required, &own), want, "case {n}");
        }
    }

    #[test]
    fn keys_name_their_cause() {
        let target = test_target();
        let queue = target.queue_id;
        let after = refresh_after(&target, "a:b", vec![1]);

        assert_eq!(after.idempotency_key, format!("refresh_prs:{queue}-a-b:1"));
        assert_eq!(after.class, JobClass::Interactive);

        let point = OffsetDateTime::from_unix_timestamp(1_790_000_000).unwrap();
        let prs: Vec<u64> = (1..=u64::try_from(MAX_PRS_PER_READ + 1).unwrap()).collect();
        let reads = refresh_at(&target, point, &prs);

        assert_eq!(reads.len(), 2);
        assert_eq!(
            reads[0].idempotency_key,
            format!("refresh_prs:{queue}-20260921T1413:1")
        );
        assert_eq!(reads[0].class, JobClass::Repair);
        assert_eq!(
            reads[1].idempotency_key,
            format!("refresh_prs:{queue}-20260921T1413-2:1")
        );

        let second: RefreshPrs = serde_json::from_value(reads[1].payload.clone()).unwrap();
        assert_eq!(second.prs, vec![prs[MAX_PRS_PER_READ]]);

        let request = SubmitPr {
            repo_id: target.repo_id,
            pr_number: 1,
            via: SubmittedVia::Comment,
            requested_by: None,
            priority: None,
            no_batch: false,
            direct: false,
            queue: None,
            label: None,
            comment_id: None,
        };

        assert_eq!(
            submit(target.org_id, target.repo.connection.id, "d", &request).idempotency_key,
            "submit_pr:d:1"
        );
    }
}

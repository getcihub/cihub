//! The jobs of the merge queue that read the PRs: `refresh_prs` reads PRs
//! again, `submit_pr` submits a PR after a command or a label; the jobs of
//! the test cycle are in `assembly`. Each one gives its facts back to the
//! queue through `queue_inbox` and marks the queue dirty; it never writes the
//! log. This module keeps what the jobs share: keys, payload and observation
//! helpers, and `Failure`.

mod create_queue;
mod refresh;
mod submit_pr;

pub use refresh::REFRESH_PRS;
pub(crate) use refresh::{refresh, refresh_after, refresh_at, refresh_reads};
pub(crate) use submit_pr::{SUBMIT_PR, SubmitPr, submit, submit_pr};

use std::error::Error;

use cihub_domain::{
    Actor, ConnectionId, Event, EventId, EventKind, JobClass, Mergeability, OrgId, QueueId, RepoId,
    ReviewDecision, UserId,
};
use cihub_mq_core::PrObservation;
use cihub_platform::{Job, Outcome, PlatformError, ensure_user, error_chain, mark_dirty};
use cihub_scm::{PrSnapshot, RepoRef, RequiredChecks, Scm, ScmError, ScmUser};
use cihub_store::{PgPool, StoreError, begin_system, deposit_fact};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use time::OffsetDateTime;
use tracing::field::display;
use tracing::{Span, warn};

use crate::MergeQueueError;
use crate::target::{QueueTarget, RepoTarget};

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
            comment_id, "command.not_acknowledged"
        );
    }
}

/// The commit status of the queue on the head of a PR (`specs/GITHUB.md`).
pub fn queue_status_context(target_branch: &str) -> String {
    format!("CIHub Merge Queue ({target_branch})")
}

/// The commit status that says whether a PR would enter its queue now
/// (`specs/GITHUB.md`).
pub fn conditions_status_context(target_branch: &str) -> String {
    format!("CIHub Conditions ({target_branch})")
}

/// Whether `name` is a status that CIHub writes, on any branch.
pub(crate) fn is_own_context(name: &str) -> bool {
    ["CIHub Merge Queue (", "CIHub Conditions ("]
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

/// The mergeability that the queue sees: a PR blocked only by the statuses of
/// CIHub is mergeable, otherwise a repository that requires one of them
/// could never enter the queue.
pub(crate) fn mergeability(pr: &PrSnapshot, required: &RequiredChecks) -> Mergeability {
    let required_own: Vec<&String> = required
        .names
        .iter()
        .filter(|name| is_own_context(name))
        .collect();
    if pr.mergeability != Mergeability::Blocked || required_own.is_empty() {
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
        .filter(|name| !is_own_context(name))
        .all(|name| pr.has_passed(name));

    if others_pass && !required_own.iter().all(|name| pr.has_passed(name)) {
        Mergeability::Mergeable
    } else {
        Mergeability::Blocked
    }
}

/// What the queue sees of `pr`: the item and the status of the conditions
/// read the same mergeability and the same required checks.
pub(crate) fn observation(
    pr: &PrSnapshot,
    required: &RequiredChecks,
    unmet: Vec<String>,
) -> PrObservation {
    PrObservation {
        url: pr.url.clone(),
        state: pr.state,
        head_sha: pr.head_sha.clone(),
        base_branch: pr.base_branch.clone(),
        draft: pr.is_draft,
        labels: pr.labels.clone(),
        mergeability: mergeability(pr, required),
        review_decision: pr.review_decision,
        checks: pr.checks.clone(),
        unmet_conditions: unmet,
        required_checks: required
            .names
            .iter()
            .filter(|name| !is_own_context(name))
            .cloned()
            .collect(),
    }
}

fn observed(pr: &PrSnapshot, required: &RequiredChecks, unmet: Vec<String>) -> EventKind {
    let PrObservation {
        url,
        state,
        head_sha,
        base_branch,
        draft,
        labels,
        mergeability,
        review_decision,
        checks,
        unmet_conditions,
        required_checks,
    } = observation(pr, required, unmet);

    EventKind::PrObserved {
        pr_number: pr.number,
        url,
        state,
        head_sha,
        base_branch,
        draft,
        labels,
        mergeability,
        review_decision,
        checks,
        unmet_conditions,
        required_checks,
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
    Retry(MergeQueueError),
    Never(MergeQueueError),
}

impl Failure {
    pub(crate) fn retry(text: impl Into<String>) -> Self {
        Self::Retry(MergeQueueError::Job(text.into()))
    }

    pub(crate) fn never(text: impl Into<String>) -> Self {
        Self::Never(MergeQueueError::Job(text.into()))
    }
}

impl From<MergeQueueError> for Failure {
    fn from(err: MergeQueueError) -> Self {
        Self::Retry(err)
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
        match err {
            ScmError::Forbidden(_) | ScmError::Validation(_) => Self::Never(err.into()),
            _ => Self::Retry(err.into()),
        }
    }
}

pub(crate) fn outcome(result: Result<(), Failure>) -> Outcome {
    match result {
        Ok(()) => Outcome::Done,
        Err(Failure::Retry(err)) => Outcome::Retry(error_chain(&err)),
        Err(Failure::Never(err)) => Outcome::Failed(error_chain(&err)),
    }
}

/// The correlation keys that a payload can carry, under the same names as in
/// the span of the job (`specs/OBSERVABILITY.md`).
const PAYLOAD_KEYS: [&str; 4] = ["repo_id", "pr_number", "item_id", "run_id"];

/// Reads the payload of `job` and records its correlation keys in the span
/// of the job, so that every line of the job says where it happened.
pub(crate) fn read_payload<T: for<'de> Deserialize<'de>>(job: &Job) -> Result<T, Failure> {
    let request = serde_json::from_value(job.payload.clone())
        .map_err(|err| Failure::Never(MergeQueueError::Payload(err)))?;

    let span = Span::current();
    for key in PAYLOAD_KEYS {
        match job.payload.get(key) {
            Some(serde_json::Value::String(text)) => {
                span.record(key, text.as_str());
            }
            Some(serde_json::Value::Number(number)) => {
                span.record(key, number.as_u64());
            }
            _ => {}
        }
    }

    Ok(request)
}

pub(crate) async fn target_of(pool: &PgPool, job: &Job) -> Result<QueueTarget, Failure> {
    let queue = job
        .queue_id
        .ok_or_else(|| Failure::never("the job has no queue"))?;
    let target = load_target(pool, queue).await?;

    Span::current().record("repo_id", display(target.repo_id));
    Ok(target)
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
    give_back_with_titles(pool, target, &[], facts, now).await
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::target::test_pr as pr;

    #[test]
    fn a_pr_blocked_only_by_the_statuses_of_cihub_is_mergeable() {
        let (queue, conditions) = (
            &queue_status_context("main"),
            &conditions_status_context("main"),
        );
        let required = RequiredChecks {
            names: BTreeSet::from(["ci".to_owned(), queue.to_owned()]),
            requires_up_to_date: false,
        };
        let both_required = RequiredChecks {
            names: BTreeSet::from(["ci".to_owned(), queue.to_owned(), conditions.to_owned()]),
            requires_up_to_date: false,
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
                pr(blocked, approved, &[("ci", true), (queue, true)]),
                &required,
                blocked,
            ),
            (
                pr(blocked, approved, &[("ci", true), (queue, true)]),
                &both_required,
                Mergeability::Mergeable,
            ),
            (
                pr(
                    blocked,
                    approved,
                    &[("ci", true), (queue, true), (conditions, true)],
                ),
                &both_required,
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
            assert_eq!(mergeability(&pr, required), want, "case {n}");
        }
    }

    #[test]
    fn a_line_of_a_job_says_where_it_happened() {
        let logs = cihub_telemetry::CapturedLogs::default();
        let (org, queue, repo) = (
            OrgId::from_ulid(ulid::Ulid(1)),
            QueueId::from_ulid(ulid::Ulid(2)),
            RepoId::from_ulid(ulid::Ulid(3)),
        );
        let job = Job {
            idempotency_key: "submit_pr:delivery-1:1".to_owned(),
            job_type: SUBMIT_PR.to_owned(),
            class: JobClass::Interactive,
            org_id: Some(org),
            connection: None,
            queue_id: Some(queue),
            payload: serde_json::json!({ "repo_id": repo, "pr_number": 4821 }),
            attempts: 1,
        };

        tracing::subscriber::with_default(logs.subscriber(), || {
            let _job = cihub_platform::job_span(&job).entered();
            let read: Result<serde::de::IgnoredAny, _> = read_payload(&job);
            assert!(read.is_ok());
            tracing::info!(reason = "permission", "submission.refused");
        });

        let records = logs.records().unwrap();
        let [record] = records.as_slice() else {
            panic!("one record expected, got {records:?}");
        };
        assert_eq!(record["message"], "submission.refused");
        assert_eq!(record["reason"], "permission");
        assert_eq!(record["job_key"], "submit_pr:delivery-1:1");
        assert_eq!(record["job_type"], SUBMIT_PR);
        assert_eq!(record["org_id"], org.to_string());
        assert_eq!(record["queue_id"], queue.to_string());
        assert_eq!(record["repo_id"], repo.to_string());
        assert_eq!(record["pr_number"], 4821);
    }
}

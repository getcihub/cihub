//! The jobs of the merge queue that lot 5 runs: `refresh_prs` reads PRs
//! again (`AGENTS.md` D96, D101), `submit_pr` submits a PR after a command
//! or a label (D102). Each one gives its facts back to the queue through
//! `queue_inbox` (D100) and marks the queue dirty; it never writes the log.

use std::sync::Arc;

use cihub_config::QueueSettings;
use cihub_domain::{
    Actor, CheckConclusion, ConnectionId, Event, EventId, EventKind, JobClass, Mergeability, OrgId,
    Priority, QueueId, RepoId, ReviewDecision, ScmInstanceId, SubmittedVia, UserId,
};
use cihub_platform::{Job, Outcome, error_chain, mark_dirty};
use cihub_scm::{PrSnapshot, RepoPermission, RequiredChecks, Scm, ScmError, ScmUser};
use cihub_store::{PgPool, begin_system, deposit_fact, ensure_user};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgConnection;
use time::OffsetDateTime;
use tracing::info;

use crate::MergeQueueError;
use crate::target::{QueueTarget, RepoTarget};

pub const REFRESH_PRS: &str = "refresh_prs";
pub const SUBMIT_PR: &str = "submit_pr";

/// The PRs that `refresh_prs` reads, at most
/// [`MAX_PRS_PER_READ`](cihub_scm::MAX_PRS_PER_READ).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RefreshPrs {
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
    pub priority: Priority,
    pub no_batch: bool,
    pub direct: bool,
}

/// A subject of a key has no `:` (`AGENTS.md` D68).
fn subject(text: &str) -> String {
    text.replace(':', "-")
}

fn job(
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
        connection: Some((target.scm_instance_id, target.repo.connection.id)),
        queue_id: Some(target.queue_id),
        payload,
        attempts: 0,
    }
}

/// A read of `prs` after the delivery `delivery_id` (`AGENTS.md` D101).
pub(crate) fn refresh_after(target: &QueueTarget, delivery_id: &str, prs: Vec<u64>) -> Job {
    let key = format!(
        "{REFRESH_PRS}:{}-{}:1",
        target.queue_id,
        subject(delivery_id)
    );
    let payload = json!(RefreshPrs { prs });
    job(target, key, REFRESH_PRS, JobClass::Interactive, payload)
}

/// The reads of `prs`, due at the read point `point` (`AGENTS.md` D101); the
/// chunks after the first one of a point add their rank to the key.
pub(crate) fn refresh_at(
    target: &QueueTarget,
    point: OffsetDateTime,
    chunk: usize,
    prs: Vec<u64>,
) -> Job {
    let window = format!(
        "{:04}{:02}{:02}T{:02}{:02}",
        point.year(),
        u8::from(point.month()),
        point.day(),
        point.hour(),
        point.minute()
    );
    let rank = if chunk == 0 {
        String::new()
    } else {
        format!("-{}", chunk + 1)
    };
    let key = format!("{REFRESH_PRS}:{}-{window}{rank}:1", target.queue_id);
    let payload = json!(RefreshPrs { prs });
    job(target, key, REFRESH_PRS, JobClass::Repair, payload)
}

/// The submission of `request` after the delivery `delivery_id`, on the
/// connection `connection` (`AGENTS.md` D102). It has no queue yet.
pub(crate) fn submit(
    org_id: OrgId,
    connection: (ScmInstanceId, ConnectionId),
    delivery_id: &str,
    request: &SubmitPr,
) -> Job {
    Job {
        idempotency_key: format!("{SUBMIT_PR}:{}:1", subject(delivery_id)),
        job_type: SUBMIT_PR.to_owned(),
        class: JobClass::Interactive,
        org_id: Some(org_id),
        connection: Some(connection),
        queue_id: None,
        payload: json!(request),
        attempts: 0,
    }
}

/// The commit status of the queue on the head of a PR (`docs/GITHUB.md`).
pub fn queue_status_context(target_branch: &str) -> String {
    format!("CIHub Merge Queue ({target_branch})")
}

/// The mergeability that the queue sees (`AGENTS.md` D105): a PR blocked only
/// by the status of the queue itself is mergeable, otherwise a repository that
/// requires this status could never enter the queue.
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
    let passing = |name: &str| {
        pr.checks
            .iter()
            .any(|c| c.name == name && c.conclusion.is_some_and(CheckConclusion::is_passing))
    };
    let others_pass = required
        .names
        .iter()
        .filter(|name| *name != own)
        .all(|name| passing(name));
    if others_pass && !passing(own) {
        Mergeability::Mergeable
    } else {
        Mergeability::Blocked
    }
}

fn observed(pr: &PrSnapshot, mergeability: Mergeability) -> EventKind {
    EventKind::PrObserved {
        pr_number: pr.number,
        state: pr.state,
        head_sha: pr.head_sha.clone(),
        base_branch: pr.base_branch.clone(),
        draft: pr.draft,
        labels: pr.labels.clone(),
        mergeability,
        review_decision: pr.review_decision,
        checks: pr.checks.clone(),
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
    instance: ScmInstanceId,
    user: &ScmUser,
) -> Result<UserId, MergeQueueError> {
    #[expect(
        clippy::disallowed_methods,
        reason = "the identifier of an account of the SCM, used only when it is new"
    )]
    let candidate = UserId::new();
    ensure_user(conn, candidate, instance, &user.external_id, &user.login)
        .await
        .map_err(MergeQueueError::Store)
}

/// Deposits `kinds` in the inbox of the queue of `target`, then marks the
/// queue dirty.
async fn give_back(
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
        deposit_fact(conn, &event, now)
            .await
            .map_err(MergeQueueError::Store)?;
    }
    mark_dirty(conn, queue_id)
        .await
        .map_err(MergeQueueError::Platform)
}

/// Why a job failed, and whether a retry can fix it.
enum Failure {
    Retry(String),
    Never(String),
}

impl From<MergeQueueError> for Failure {
    fn from(err: MergeQueueError) -> Self {
        Self::Retry(error_chain(&err))
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

fn outcome(result: Result<(), Failure>) -> Outcome {
    match result {
        Ok(()) => Outcome::Done,
        Err(Failure::Retry(text)) => Outcome::Retry(text),
        Err(Failure::Never(text)) => Outcome::Failed(text),
    }
}

fn read_payload<T: for<'de> Deserialize<'de>>(job: &Job) -> Result<T, Failure> {
    serde_json::from_value(job.payload.clone()).map_err(|err| Failure::Never(error_chain(&err)))
}

async fn target_of(pool: &PgPool, job: &Job) -> Result<QueueTarget, Failure> {
    let queue = job
        .queue_id
        .ok_or_else(|| Failure::Never("the job has no queue".into()))?;
    let mut tx = begin_system(pool)
        .await
        .map_err(|err| Failure::from(MergeQueueError::Store(err)))?;
    let target = QueueTarget::load(&mut tx, queue).await?;
    tx.commit()
        .await
        .map_err(|err| Failure::from(MergeQueueError::Query(err)))?;
    Ok(target)
}

/// Reads the PRs of the job and gives back one `pr.observed` per PR. A PR
/// that no longer exists gives nothing.
pub(crate) async fn execute_refresh(
    pool: PgPool,
    scm: Arc<dyn Scm>,
    job: Job,
    now: OffsetDateTime,
) -> Outcome {
    outcome(refresh(&pool, scm.as_ref(), &job, now).await)
}

async fn refresh(
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
    let facts = prs
        .iter()
        .map(|pr| {
            (
                Actor::System,
                observed(pr, mergeability(pr, &required, &own)),
            )
        })
        .collect();
    let mut tx = begin_system(pool)
        .await
        .map_err(|err| Failure::from(MergeQueueError::Store(err)))?;
    give_back(&mut tx, target.org_id, target.queue_id, facts, now).await?;
    tx.commit()
        .await
        .map_err(|err| Failure::from(MergeQueueError::Query(err)))?;
    Ok(())
}

/// Checks the submission of the job, reads the PR, and gives back its
/// `item.submitted` and its `pr.observed`. A refused submission gives
/// nothing: the message on the PR comes with lot 9.
pub(crate) async fn execute_submit(
    pool: PgPool,
    scm: Arc<dyn Scm>,
    settings: Arc<QueueSettings>,
    job: Job,
    now: OffsetDateTime,
) -> Outcome {
    outcome(submit_pr(&pool, scm.as_ref(), &settings, &job, now).await)
}

async fn submit_pr(
    pool: &PgPool,
    scm: &dyn Scm,
    settings: &QueueSettings,
    job: &Job,
    now: OffsetDateTime,
) -> Result<(), Failure> {
    let request: SubmitPr = read_payload(job)?;
    let mut tx = begin_system(pool)
        .await
        .map_err(|err| Failure::from(MergeQueueError::Store(err)))?;
    let repo = RepoTarget::load(&mut tx, request.repo_id).await?;
    tx.commit()
        .await
        .map_err(|err| Failure::from(MergeQueueError::Query(err)))?;
    let pr_number = request.pr_number;
    if let Some(author) = &request.requested_by {
        let need = if request.direct {
            RepoPermission::Admin
        } else {
            RepoPermission::Write
        };
        let user = ScmUser::from(author);
        let has = scm.permission(&repo.repo, &user, job.class).await?;
        if has < need {
            info!(pr_number, "submission refused: permission");
            return Ok(());
        }
    }
    let pr = match scm.pull_request(&repo.repo, pr_number, job.class).await {
        Ok(pr) => pr,
        Err(ScmError::NotFound(_)) => return Ok(()),
        Err(err) => return Err(err.into()),
    };
    let mut tx = begin_system(pool)
        .await
        .map_err(|err| Failure::from(MergeQueueError::Store(err)))?;
    let target = QueueTarget::of_branch(&mut tx, repo.repo_id, &pr.base_branch).await?;
    tx.commit()
        .await
        .map_err(|err| Failure::from(MergeQueueError::Query(err)))?;
    let Some(target) = target.filter(|t| t.enabled) else {
        info!(
            pr_number,
            base_branch = pr.base_branch,
            "submission dropped: no queue"
        );
        return Ok(());
    };
    if request.via == SubmittedVia::Label && !pr.labels.contains(&settings.enqueue_label.value) {
        info!(pr_number, "submission dropped: label removed");
        return Ok(());
    }
    let required = scm
        .required_checks(&target.repo, &target.target_branch, job.class)
        .await?;
    let own = queue_status_context(&target.target_branch);
    let mut tx = begin_system(pool)
        .await
        .map_err(|err| Failure::from(MergeQueueError::Store(err)))?;
    let author_id = user_of(&mut tx, target.scm_instance_id, &pr.author).await?;
    let actor = match &request.requested_by {
        Some(account) => Actor::User {
            user_id: user_of(&mut tx, target.scm_instance_id, &account.into()).await?,
        },
        None => Actor::System,
    };
    let submitted = EventKind::ItemSubmitted {
        pr_number,
        head_sha: pr.head_sha.clone(),
        priority: request.priority,
        no_batch: request.no_batch,
        direct: request.direct,
        submitted_via: request.via,
        author_id,
        additions: pr.additions,
        deletions: pr.deletions,
        changed_files: pr.changed_files,
    };
    let facts = vec![
        (actor, submitted),
        (
            Actor::System,
            observed(&pr, mergeability(&pr, &required, &own)),
        ),
    ];
    give_back(&mut tx, target.org_id, target.queue_id, facts, now).await?;
    tx.commit()
        .await
        .map_err(|err| Failure::from(MergeQueueError::Query(err)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use cihub_domain::{CheckKind, CheckReport, PrState};

    use super::*;

    fn pr(
        mergeability: Mergeability,
        review: ReviewDecision,
        checks: &[(&str, bool)],
    ) -> PrSnapshot {
        PrSnapshot {
            number: 1,
            state: PrState::Open,
            head_sha: "a".into(),
            base_branch: "main".into(),
            draft: false,
            labels: BTreeSet::new(),
            mergeability,
            review_decision: review,
            checks: checks
                .iter()
                .map(|(name, ok)| CheckReport {
                    name: (*name).into(),
                    kind: CheckKind::Status,
                    conclusion: Some(if *ok {
                        CheckConclusion::Success
                    } else {
                        CheckConclusion::Failure
                    }),
                    details_url: None,
                })
                .collect(),
            author: ScmUser {
                external_id: "1".into(),
                login: "octocat".into(),
            },
            additions: 0,
            deletions: 0,
            changed_files: 0,
        }
    }

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
        let target = QueueTarget {
            queue_id: QueueId::from_ulid(ulid::Ulid(2)),
            org_id: OrgId::from_ulid(ulid::Ulid(1)),
            scm_instance_id: ScmInstanceId::from_ulid(ulid::Ulid(3)),
            repo: cihub_scm::RepoRef {
                connection: cihub_scm::ConnectionRef {
                    id: ConnectionId::from_ulid(ulid::Ulid(4)),
                    external_id: "4".into(),
                },
                external_id: "5".into(),
                namespace: "acme".into(),
                name: "app".into(),
            },
            target_branch: "main".into(),
            enabled: true,
            repo_id: RepoId::from_ulid(ulid::Ulid(6)),
        };
        let queue = target.queue_id;
        let after = refresh_after(&target, "a:b", vec![1]);
        assert_eq!(after.idempotency_key, format!("refresh_prs:{queue}-a-b:1"));
        assert_eq!(after.class, JobClass::Interactive);
        let point = OffsetDateTime::from_unix_timestamp(1_790_000_000).unwrap();
        let first = refresh_at(&target, point, 0, vec![1]);
        assert_eq!(
            first.idempotency_key,
            format!("refresh_prs:{queue}-20260921T1413:1")
        );
        assert_eq!(first.class, JobClass::Repair);
        let second = refresh_at(&target, point, 1, vec![2]);
        assert_eq!(
            second.idempotency_key,
            format!("refresh_prs:{queue}-20260921T1413-2:1")
        );
        let request = SubmitPr {
            repo_id: target.repo_id,
            pr_number: 1,
            via: SubmittedVia::Comment,
            requested_by: None,
            priority: Priority::MEDIUM,
            no_batch: false,
            direct: false,
        };
        let connection = (target.scm_instance_id, target.repo.connection.id);
        assert_eq!(
            submit(target.org_id, connection, "d", &request).idempotency_key,
            "submit_pr:d:1"
        );
    }
}

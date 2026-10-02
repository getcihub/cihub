//! The merge job (`AGENTS.md` D123): `merge_pr` merges a PR of the desired
//! state, guarded by its head, and gives back `merge.succeeded` or
//! `merge.failed`. It reads the projection first, so that a decision that
//! changed since the request merges nothing (principle III).

use std::sync::Arc;

use cihub_config::QueueSettings;
use cihub_domain::{
    Actor, EventKind, ItemId, ItemState, JobClass, JobError, MergeCommitTitle, MergeMethod,
    PrState, RunId,
};
use cihub_mq_core::Merge;
use cihub_platform::{Job, MAX_ATTEMPTS, Outcome, error_chain};
use cihub_scm::{PrMerge, Scm, ScmError};
use cihub_store::{PgPool, begin_system};
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::{Duration, OffsetDateTime};
use tracing::{info, warn};

use crate::MergeQueueError;
use crate::jobs::{Failure, give_back, job, outcome, read_payload, target_of};
use crate::target::QueueTarget;

pub const MERGE_PR: &str = "merge_pr";

/// After this many attempts, a merge that still fails on a passing error
/// gives up (`AGENTS.md` D123): with the backoff of the jobs, about a minute.
pub const MERGE_ATTEMPTS: i32 = 6;

// A retry must end in a fact before the job goes `dead`.
const _: () = assert!(MERGE_ATTEMPTS < MAX_ATTEMPTS);

/// A merge waits for the end of a rate limit at most this long.
pub const MERGE_RATE_LIMIT_WAIT: Duration = Duration::minutes(15);

/// The merge of one item, as the plan describes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct MergePr {
    pub item_id: ItemId,
    /// The run that proved the item: an item proved again by another run
    /// has another job.
    pub run_id: RunId,
    pub pr_number: u64,
    pub head_sha: String,
    pub tested_sha: String,
}

/// The merge of `merge`, one job per item and run that proved it.
pub(crate) fn merge_job(target: &QueueTarget, merge: &Merge) -> Job {
    let run_id = merge.run_id;
    let request = MergePr {
        item_id: merge.item_id,
        run_id,
        pr_number: merge.pr_number,
        head_sha: merge.head_sha.clone(),
        tested_sha: merge.tested_sha.clone(),
    };
    let key = format!("{MERGE_PR}:{}-{run_id}:1", merge.item_id);
    job(target, key, MERGE_PR, JobClass::Merge, json!(request))
}

/// The title of the merge commit: the `merge-commit-title:` line of the
/// description, or the title of the PR as `merge_commit_title` says.
pub fn merge_commit_title(
    title: &str,
    body: &str,
    number: u64,
    setting: MergeCommitTitle,
) -> String {
    let line = body.lines().find_map(|line| {
        line.trim()
            .strip_prefix("merge-commit-title:")
            .map(str::trim)
            .filter(|title| !title.is_empty())
    });
    match (line, setting) {
        (Some(line), _) => line.to_owned(),
        (None, MergeCommitTitle::PrTitle) => title.to_owned(),
        (None, MergeCommitTitle::PrTitleAndNumber) => format!("{title} (#{number})"),
    }
}

/// What the merge gives: a fact, or a new attempt later.
enum Merged {
    Fact(EventKind),
    Retry(String),
}

/// The fact or the new attempt that an error of the SCM gives, after
/// `attempts` attempts.
fn after_error(request: &MergePr, err: &ScmError, attempts: i32, now: OffsetDateTime) -> Merged {
    let failed = |error| {
        Merged::Fact(EventKind::MergeFailed {
            item_id: request.item_id,
            direct: false,
            error,
        })
    };
    let text = error_chain(err);
    match err {
        ScmError::HeadChanged => failed(JobError::HeadChanged),
        ScmError::Forbidden(_) => failed(JobError::BranchProtectionUnmet),
        ScmError::MergeRejected(_) | ScmError::Validation(_) => failed(JobError::MergeRejected),
        ScmError::RateLimited { until } | ScmError::Throttled { until }
            if *until - now <= MERGE_RATE_LIMIT_WAIT && attempts < MAX_ATTEMPTS =>
        {
            Merged::Retry(text)
        }
        ScmError::RateLimited { .. } | ScmError::Throttled { .. } => {
            failed(JobError::RateLimitExceeded)
        }
        _ if attempts < MERGE_ATTEMPTS => Merged::Retry(text),
        ScmError::Unavailable(_) => failed(JobError::RepeatedServerErrors),
        ScmError::MergeInProgress => failed(JobError::MergeAlreadyInProgress),
        _ => failed(JobError::MergeRejected),
    }
}

pub(crate) async fn execute_merge(
    pool: PgPool,
    scm: Arc<dyn Scm>,
    settings: Arc<QueueSettings>,
    job: Job,
    now: OffsetDateTime,
) -> Outcome {
    outcome(merge_pr(&pool, scm.as_ref(), &settings, &job, now).await)
}

async fn merge_pr(
    pool: &PgPool,
    scm: &dyn Scm,
    settings: &QueueSettings,
    job: &Job,
    now: OffsetDateTime,
) -> Result<(), Failure> {
    let request: MergePr = read_payload(job)?;
    let target = target_of(pool, job).await?;
    let item_id = request.item_id;
    if !still_to_merge(pool, &request).await? {
        info!(%item_id, "item no longer waits for its merge");
        return Ok(());
    }
    let merged = match land(scm, settings, &target, &request, job.class).await {
        Ok(Landed::Merged(merged_sha)) => {
            info!(%item_id, merged_sha, "PR merged");
            Merged::Fact(EventKind::MergeSucceeded {
                item_id,
                merged_sha,
            })
        }
        Ok(Landed::AlreadyMerged) => Merged::Fact(EventKind::MergeFailed {
            item_id,
            direct: false,
            error: JobError::PrAlreadyMerged,
        }),
        Err(err) => after_error(&request, &err, job.attempts, now),
    };
    let fact = match merged {
        Merged::Fact(fact) => fact,
        Merged::Retry(text) => return Err(Failure::Retry(text)),
    };
    if let EventKind::MergeFailed { error, .. } = &fact {
        warn!(%item_id, error = error.as_str(), "PR not merged");
    }
    let mut tx = begin_system(pool)
        .await
        .map_err(|err| Failure::from(MergeQueueError::Store(err)))?;
    give_back(
        &mut tx,
        target.org_id,
        target.queue_id,
        vec![(Actor::Bot, fact)],
        now,
    )
    .await?;
    tx.commit()
        .await
        .map_err(|err| Failure::from(MergeQueueError::Query(err)))?;
    Ok(())
}

/// What the merge of a PR did.
enum Landed {
    /// The commit that landed on the target.
    Merged(String),
    /// The PR was merged before: not by this job (`AGENTS.md` D132).
    AlreadyMerged,
}

/// Reads the PR, then merges it with the method of the settings. A PR
/// already merged is not merged again: someone else merged it, or this job
/// before it stopped, and the queue then tests what follows again.
async fn land(
    scm: &dyn Scm,
    settings: &QueueSettings,
    target: &QueueTarget,
    request: &MergePr,
    class: JobClass,
) -> Result<Landed, ScmError> {
    let pr = scm
        .pull_request(&target.repo, request.pr_number, class)
        .await?;
    if pr.state == PrState::Merged {
        return Ok(Landed::AlreadyMerged);
    }
    let method = settings.merge_method.value;
    let title = if method == MergeMethod::FastForward {
        String::new()
    } else {
        merge_commit_title(
            &pr.title,
            &pr.body,
            pr.number,
            settings.merge_commit_title.value,
        )
    };
    let merge = PrMerge {
        number: request.pr_number,
        head_sha: request.head_sha.clone(),
        tested_sha: request.tested_sha.clone(),
        method,
        title,
        message: String::new(),
    };
    match scm.merge_pr(&target.repo, &merge, class).await {
        // The target moved: the push restarts the group (`AGENTS.md` D124).
        Err(ScmError::NotFastForward) => Err(ScmError::Unavailable(
            "the target moved: not a fast forward".into(),
        )),
        other => other.map(Landed::Merged),
    }
}

/// The item is still `tests_passed`, proved by the run of the request.
async fn still_to_merge(pool: &PgPool, request: &MergePr) -> Result<bool, Failure> {
    let mut tx = begin_system(pool)
        .await
        .map_err(|err| Failure::from(MergeQueueError::Store(err)))?;
    let to_merge: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM queue_items
                        WHERE id = $1 AND state = $2::item_state AND verified_by_run_id = $3)",
    )
    .bind(request.item_id.to_string())
    .bind(ItemState::TestsPassed.as_str())
    .bind(request.run_id.to_string())
    .fetch_one(&mut *tx)
    .await
    .map_err(|err| Failure::from(MergeQueueError::Query(err)))?;
    tx.commit()
        .await
        .map_err(|err| Failure::from(MergeQueueError::Query(err)))?;
    Ok(to_merge)
}

#[cfg(test)]
mod tests {
    use ulid::Ulid;

    use super::*;

    #[test]
    fn the_title_comes_from_the_description_or_the_pr() {
        let cases = [
            ("Fix", "", MergeCommitTitle::PrTitle, "Fix"),
            ("Fix", "", MergeCommitTitle::PrTitleAndNumber, "Fix (#7)"),
            (
                "Fix",
                "Some text\n  merge-commit-title: Better title \nmore",
                MergeCommitTitle::PrTitleAndNumber,
                "Better title",
            ),
            (
                "Fix",
                "merge-commit-title:   ",
                MergeCommitTitle::PrTitle,
                "Fix",
            ),
        ];
        for (title, body, setting, want) in cases {
            assert_eq!(
                merge_commit_title(title, body, 7, setting),
                want,
                "{body:?}"
            );
        }
    }

    #[test]
    fn errors_give_their_fact_or_a_new_attempt() {
        let request = MergePr {
            item_id: ItemId::from_ulid(Ulid(1)),
            run_id: RunId::from_ulid(Ulid(2)),
            pr_number: 1,
            head_sha: "head".into(),
            tested_sha: "test".into(),
        };
        let now = OffsetDateTime::UNIX_EPOCH;
        let error = |err, attempts| match after_error(&request, &err, attempts, now) {
            Merged::Fact(EventKind::MergeFailed { error, .. }) => Some(error),
            Merged::Fact(_) => unreachable!("an error gives no success"),
            Merged::Retry(_) => None,
        };
        let unavailable = || ScmError::Unavailable("502".into());
        assert_eq!(error(ScmError::HeadChanged, 1), Some(JobError::HeadChanged));
        assert_eq!(
            error(ScmError::Forbidden("rules".into()), 1),
            Some(JobError::BranchProtectionUnmet)
        );
        assert_eq!(
            error(ScmError::MergeRejected("no".into()), 1),
            Some(JobError::MergeRejected)
        );
        assert_eq!(error(unavailable(), MERGE_ATTEMPTS - 1), None);
        assert_eq!(
            error(unavailable(), MERGE_ATTEMPTS),
            Some(JobError::RepeatedServerErrors)
        );
        assert_eq!(
            error(ScmError::MergeInProgress, MERGE_ATTEMPTS),
            Some(JobError::MergeAlreadyInProgress)
        );
        let soon = now + Duration::minutes(10);
        let late = now + Duration::minutes(20);
        assert_eq!(error(ScmError::RateLimited { until: soon }, 3), None);
        assert_eq!(
            error(ScmError::RateLimited { until: late }, 1),
            Some(JobError::RateLimitExceeded)
        );
        assert_eq!(
            error(ScmError::Throttled { until: soon }, MAX_ATTEMPTS),
            Some(JobError::RateLimitExceeded)
        );
    }

    #[test]
    fn the_key_names_the_item_and_its_run() {
        let target = crate::target::test_target();
        let run_id = RunId::from_ulid(Ulid(2));
        let merge = Merge {
            item_id: ItemId::from_ulid(Ulid(1)),
            pr_number: 3,
            head_sha: "head".into(),
            run_id,
            tested_sha: "test".into(),
        };
        let job = merge_job(&target, &merge);
        assert_eq!(
            job.idempotency_key,
            format!("merge_pr:{}-{run_id}:1", merge.item_id)
        );
        assert_eq!(job.class, JobClass::Merge);
    }
}

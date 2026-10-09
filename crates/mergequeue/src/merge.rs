//! The merge job (`DECISIONS.md` D123): `merge_pr` merges a PR of the desired
//! state, guarded by its head, and gives back `merge.succeeded` or
//! `merge.failed`. It reads the projection first, so that a decision that
//! changed since the request merges nothing (principle III).

use cihub_config::QueueSettings;
use cihub_domain::{
    Actor, EventKind, ItemId, ItemState, JobClass, JobError, MergeCommitTitle, MergeMethod,
    Mergeability, PrState, RunId,
};
use cihub_mq_core::Merge;
use cihub_platform::{Job, MAX_ATTEMPTS, error_chain};
use cihub_scm::{PrMerge, PrSnapshot, Scm, ScmError, check_passed};
use cihub_store::{PgPool, begin_system};
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::{Duration, OffsetDateTime};
use tracing::{info, warn};

use crate::assembly::{Step, required_statuses};
use crate::jobs::{
    Failure, give_back_facts, job, mergeability, queue_status_context, read_payload, target_of,
};
use crate::surfaces::merging_status;
use crate::target::QueueTarget;

pub(crate) const MERGE_PR: &str = "merge_pr";

/// After this many attempts, a merge that still fails on a passing error
/// gives up (`DECISIONS.md` D123): with the backoff of the jobs, about a minute.
pub(crate) const MERGE_ATTEMPTS: i32 = 6;

// A retry must end in a fact before the job goes `dead`.
const _: () = assert!(MERGE_ATTEMPTS < MAX_ATTEMPTS);

/// A merge waits for the end of a rate limit at most this long.
pub(crate) const MERGE_RATE_LIMIT_WAIT: Duration = Duration::minutes(15);

/// The merge of one item, as the plan describes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct MergePr {
    pub item_id: ItemId,
    /// The run that proved the item: an item proved again by another run
    /// has another job. `None` for a direct merge (`DECISIONS.md` D153).
    pub run_id: Option<RunId>,
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
    let subject = run_id.map_or_else(|| "direct".to_owned(), |run_id| run_id.to_string());
    let key = format!("{MERGE_PR}:{}-{subject}:1", merge.item_id);
    job(target, key, MERGE_PR, JobClass::Merge, json!(request))
}

/// The title of the merge commit: the `merge-commit-title:` line of the
/// description, or the title of the PR as `merge_commit_title` says.
pub(crate) fn merge_commit_title(
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

/// The fact or the new attempt that an error of the SCM gives, after
/// `attempts` attempts.
fn after_error(
    request: &MergePr,
    err: &ScmError,
    attempts: i32,
    now: OffsetDateTime,
) -> Result<EventKind, Failure> {
    let failed = |error| {
        Ok(EventKind::MergeFailed {
            item_id: request.item_id,
            direct: request.run_id.is_none(),
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
            Err(Failure::Retry(text))
        }
        ScmError::RateLimited { .. } | ScmError::Throttled { .. } => {
            failed(JobError::RateLimitExceeded)
        }
        _ if attempts < MERGE_ATTEMPTS => Err(Failure::Retry(text)),
        ScmError::Unavailable(_) => failed(JobError::RepeatedServerErrors),
        ScmError::MergeInProgress => failed(JobError::MergeAlreadyInProgress),
        _ => failed(JobError::MergeRejected),
    }
}

fn log_merge_failure(item_id: ItemId, fact: &EventKind, scm_error: Option<&str>) {
    if let EventKind::MergeFailed { error, .. } = fact
        && *error != JobError::DirectMergeUnavailable
    {
        warn!(
            %item_id,
            error = error.as_str(),
            scm_error,
            "PR not merged"
        );
    }
}

pub(crate) async fn merge_pr(
    pool: &PgPool,
    scm: &dyn Scm,
    job: &Job,
    now: OffsetDateTime,
) -> Result<(), Failure> {
    let request: MergePr = read_payload(job)?;
    let target = target_of(pool, job).await?;
    let settings = &target.settings()?;
    let item_id = request.item_id;
    if !still_to_merge(pool, &request).await? {
        info!(%item_id, "item no longer waits for its merge");
        return Ok(());
    }

    let landed = land(scm, settings, &target, &request, job.class).await;
    let scm_error = landed.as_ref().err().map(|err| error_chain(err));
    let failed = |error| EventKind::MergeFailed {
        item_id,
        direct: request.run_id.is_none(),
        error,
    };
    let fact = match landed {
        Ok(Landed::Merged(merged_sha)) => {
            info!(%item_id, merged_sha, "PR merged");

            EventKind::MergeSucceeded {
                item_id,
                merged_sha,
            }
        }
        Ok(Landed::AlreadyMerged) => failed(JobError::PrAlreadyMerged),
        Ok(Landed::Unavailable) => {
            info!(%item_id, "direct merge not possible: the item goes through the tests");

            EventKind::MergeFailed {
                item_id,
                direct: true,
                error: JobError::DirectMergeUnavailable,
            }
        }
        Err(err) => after_error(&request, &err, job.attempts, now)?,
    };

    log_merge_failure(item_id, &fact, scm_error.as_deref());

    give_back_facts(pool, &target, vec![(Actor::Bot, fact)], now).await
}

/// What the merge of a PR did.
enum Landed {
    /// The commit that landed on the target.
    Merged(String),
    /// The PR was merged before: not by this job (`DECISIONS.md` D132).
    AlreadyMerged,
    /// The direct merge cannot happen now (`DECISIONS.md` D153).
    Unavailable,
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
    if request.run_id.is_none() && !direct_merge_possible(scm, settings, target, &pr, class).await?
    {
        return Ok(Landed::Unavailable);
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
    if settings.status_check_enabled.value {
        let status = merging_status(&target.target_branch, &request.head_sha);
        scm.set_status(&target.repo, &status, class).await?;
    }
    match scm.merge_pr(&target.repo, &merge, class).await {
        // The target moved: the push restarts the group (`DECISIONS.md` D124).
        Err(ScmError::NotFastForward) => Err(ScmError::Unavailable(
            "the target moved: not a fast forward".into(),
        )),
        other => other.map(Landed::Merged),
    }
}

/// A direct merge lands exactly what the CI of the PR tested (`DECISIONS.md`
/// D153): the required statuses of a test SHA on the current head of the
/// target are green on the head of the PR, the PR contains that head, and
/// the PR is mergeable. An error of the configuration leaves the item to the
/// tests, whose assembly reports it.
async fn direct_merge_possible(
    scm: &dyn Scm,
    settings: &QueueSettings,
    target: &QueueTarget,
    pr: &PrSnapshot,
    class: JobClass,
) -> Result<bool, ScmError> {
    let repo = &target.repo;
    let Some(base) = scm.branch_head(repo, &target.target_branch, class).await? else {
        return Ok(false);
    };
    let names = match required_statuses(scm, target, settings, &base, class).await {
        Ok((names, _)) => names,
        Err(Step::Transient(text) | Step::Passing(text)) => {
            return Err(ScmError::Unavailable(text));
        }
        Err(Step::GiveUp(..) | Step::Wait(_)) => return Ok(false),
    };
    if !names.iter().all(|name| check_passed(pr, name)) {
        return Ok(false);
    }
    let rules = scm
        .required_checks(repo, &target.target_branch, class)
        .await?;
    let own = queue_status_context(&target.target_branch);
    if mergeability(pr, &rules, &own) != Mergeability::Mergeable {
        return Ok(false);
    }
    scm.is_ancestor(repo, &base, &pr.head_sha, class).await
}

/// The item is still `tests_passed`, proved by the run of the request, or
/// still `pending` for a direct merge.
async fn still_to_merge(pool: &PgPool, request: &MergePr) -> Result<bool, Failure> {
    let mut tx = begin_system(pool).await?;
    let state = if request.run_id.is_some() {
        ItemState::TestsPassed
    } else {
        ItemState::Pending
    };
    let to_merge: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM queue_items
                        WHERE id = $1 AND state = $2::item_state
                          AND verified_by_run_id IS NOT DISTINCT FROM $3)",
    )
    .bind(request.item_id.to_string())
    .bind(state.as_str())
    .bind(request.run_id.map(|run_id| run_id.to_string()))
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
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
            run_id: Some(RunId::from_ulid(Ulid(2))),
            pr_number: 1,
            head_sha: "head".into(),
            tested_sha: "test".into(),
        };
        let now = OffsetDateTime::UNIX_EPOCH;
        let error = |err, attempts| match after_error(&request, &err, attempts, now) {
            Ok(EventKind::MergeFailed { error, .. }) => Some(error),
            Ok(_) => unreachable!("an error gives no success"),
            Err(_) => None,
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
            run_id: Some(run_id),
            tested_sha: "test".into(),
        };
        let job = merge_job(&target, &merge);
        assert_eq!(
            job.idempotency_key,
            format!("merge_pr:{}-{run_id}:1", merge.item_id)
        );
        assert_eq!(job.class, JobClass::Merge);
        // A direct merge has its own key (`DECISIONS.md` D153).
        let direct = Merge {
            run_id: None,
            ..merge
        };
        assert_eq!(
            merge_job(&target, &direct).idempotency_key,
            format!("merge_pr:{}-direct:1", direct.item_id)
        );
    }
}

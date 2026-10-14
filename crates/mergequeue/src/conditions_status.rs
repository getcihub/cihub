//! The status that says whether a PR would enter its queue now, submitted or
//! not (`specs/GITHUB.md`): `observe_pr` reads open PRs and writes the status
//! they must show in `pr_conditions`, `publish_conditions` publishes it. The
//! log of the queue does not change: a PR not submitted is not in the queue.

use cihub_domain::{BlockReason, JobClass, PrState, run_of_test_branch};
use cihub_mq_core::{PrObservation, conditions_of};
use cihub_platform::{Job, enqueue_job};
use cihub_scm::{MAX_PRS_PER_READ, PrSnapshot, RequiredChecks, Scm, ScmError, StatusUpdate};
use cihub_store::{PgPool, begin_system};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgConnection;
use time::OffsetDateTime;
use tracing::info;

use crate::MergeQueueError;
use crate::admission::{Reading, Sender, routes_to, unmet_conditions};
use crate::jobs::{
    Failure, conditions_status_context, job, mergeability, own_contexts, read_payload, target_of,
};
use crate::surfaces::{StatusState, block_text, capitalized, hash, truncate};
use crate::target::QueueTarget;

pub(crate) const OBSERVE_PR: &str = "observe_pr";
pub(crate) const PUBLISH_CONDITIONS: &str = "publish_conditions";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ObservePr {
    pub prs: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct PublishConditions {
    pub pr_number: i64,
}

/// The status a PR must show, on its head.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ConditionsStatus {
    pub sha: String,
    pub state: StatusState,
    pub description: String,
}

/// The reads of `prs` after `subject`, a delivery or a moment, by chunks of
/// [`MAX_PRS_PER_READ`]. Class `repair`: the status never slows the queue.
pub(crate) fn observe_jobs(target: &QueueTarget, subject: &str, prs: &[u64]) -> Vec<Job> {
    prs.chunks(MAX_PRS_PER_READ)
        .enumerate()
        .map(|(chunk, prs)| {
            let key = format!("{OBSERVE_PR}:{}-{subject}-{chunk}:1", target.queue_id);
            let payload = json!(ObservePr { prs: prs.to_vec() });
            job(target, key, OBSERVE_PR, JobClass::Repair, payload)
        })
        .collect()
}

/// The open PRs of the queue that show a status.
///
/// # Errors
///
/// Postgres fails.
pub(crate) async fn observed_prs(
    conn: &mut PgConnection,
    target: &QueueTarget,
) -> Result<Vec<u64>, MergeQueueError> {
    let numbers: Vec<i64> = sqlx::query_scalar(
        "SELECT pr_number FROM pr_conditions WHERE queue_id = $1 ORDER BY pr_number",
    )
    .bind(target.queue_id.to_string())
    .fetch_all(conn)
    .await?;

    Ok(numbers
        .into_iter()
        .filter_map(|number| u64::try_from(number).ok())
        .collect())
}

/// The state and the description of the status for `blocks`, what keeps the
/// PR out of its queue: a required check that runs, or a SCM that does not
/// say yet, is `pending`; any other reason fails.
fn status_of(blocks: &[BlockReason]) -> (StatusState, String) {
    let waits = |reason: &&BlockReason| {
        matches!(
            reason,
            BlockReason::Undetermined | BlockReason::AwaitingRequiredCheck { .. }
        )
    };
    let shown = blocks
        .iter()
        .find(|reason| !waits(reason))
        .or(blocks.first());
    let Some(shown) = shown else {
        return (StatusState::Success, "Ready for the queue".to_owned());
    };

    let state = if blocks.iter().all(|reason| waits(&reason)) {
        StatusState::Pending
    } else {
        StatusState::Failure
    };
    let text = capitalized(&block_text(shown));
    let description = match blocks.len() - 1 {
        0 => text,
        others => format!("{text}, and {others} more"),
    };

    (state, truncate(&description))
}

/// What the queue sees of `pr`, as `refresh_prs` gives it to the core.
fn observation(
    pr: &PrSnapshot,
    required: &RequiredChecks,
    own: &[String],
    unmet: Vec<String>,
) -> PrObservation {
    PrObservation {
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
            .filter(|name| !own.contains(name))
            .cloned()
            .collect(),
    }
}

/// Reads the PRs of the job and writes the status that each one must show; a
/// PR that is closed, gone, a draft PR of a run, or that the routing sends to
/// another queue loses its row.
pub(crate) async fn observe(
    pool: &PgPool,
    scm: &dyn Scm,
    job: &Job,
    now: OffsetDateTime,
) -> Result<(), Failure> {
    let request: ObservePr = read_payload(job)?;
    let target = target_of(pool, job).await?;
    let config = target.config()?;
    if !target
        .settings_in(config.as_ref())?
        .status_check_enabled
        .value
    {
        return Ok(());
    }

    let required = match scm
        .required_checks(&target.repo, &target.target_branch, job.class)
        .await
    {
        Ok(required) => required,
        // The target is gone: `check_config` reports it, and no PR can enter.
        Err(ScmError::NotFound(_)) => return Ok(()),
        Err(err) => return Err(err.into()),
    };
    let prs = scm
        .pull_requests(&target.repo, &request.prs, job.class)
        .await?;
    let own = own_contexts(&target.target_branch);
    let sender = Sender::default();
    let reading = Reading {
        sender: &sender,
        rules: &required,
        own_statuses: &own,
        class: job.class,
        now,
    };
    let mut statuses = Vec::with_capacity(prs.len());

    for pr in &prs {
        // The draft PR of a run is the queue's own, not a PR to admit.
        let is_ours = pr.state == PrState::Open
            && pr.base_branch == target.target_branch
            && run_of_test_branch(&pr.head_branch).is_none()
            && routes_to(scm, &target, config.as_ref(), pr, &reading).await?;
        if !is_ours {
            continue;
        }

        let unmet = unmet_conditions(scm, &target, config.as_ref(), pr, &reading).await?;
        let blocks = conditions_of(&observation(pr, &required, &own, unmet));
        let (state, description) = status_of(&blocks);
        statuses.push((
            pr.number,
            ConditionsStatus {
                sha: pr.head_sha.clone(),
                state,
                description,
            },
        ));
    }

    let gone: Vec<i64> = request
        .prs
        .iter()
        .filter(|number| statuses.iter().all(|(shown, _)| shown != *number))
        .filter_map(|number| i64::try_from(*number).ok())
        .collect();
    let mut tx = begin_system(pool).await?;

    // A newer read of a reopened PR keeps its row.
    sqlx::query(
        "DELETE FROM pr_conditions
         WHERE queue_id = $1 AND pr_number = ANY($2) AND observed_at <= $3",
    )
    .bind(target.queue_id.to_string())
    .bind(&gone)
    .bind(now)
    .execute(&mut *tx)
    .await?;

    for (number, status) in &statuses {
        let number = i64::try_from(*number).unwrap_or(i64::MAX);
        if let Some(desired_hash) = keep_status(&mut tx, &target, number, status, now).await? {
            let publish = publish_job(&target, number, &desired_hash, &job.idempotency_key);
            enqueue_job(&mut tx, &publish, now, now).await?;
        }
    }

    tx.commit().await?;

    Ok(())
}

/// Keeps `status` as the one the PR `number` must show, unless a newer read
/// wrote its row. Returns its hash while the SCM does not show it: a status
/// that changed, or one whose publication has not ended yet.
async fn keep_status(
    conn: &mut PgConnection,
    target: &QueueTarget,
    number: i64,
    status: &ConditionsStatus,
    now: OffsetDateTime,
) -> Result<Option<String>, Failure> {
    let desired = serde_json::to_value(status).map_err(|err| Failure::Never(err.to_string()))?;
    let desired_hash = hash(status);
    let is_unpublished: Option<bool> = sqlx::query_scalar(
        "INSERT INTO pr_conditions (org_id, queue_id, pr_number, head_sha, desired, desired_hash,
                                    observed_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (queue_id, pr_number) DO UPDATE
         SET head_sha = EXCLUDED.head_sha, desired = EXCLUDED.desired,
             desired_hash = EXCLUDED.desired_hash, observed_at = EXCLUDED.observed_at
         WHERE pr_conditions.observed_at <= EXCLUDED.observed_at
         RETURNING published_hash IS DISTINCT FROM desired_hash",
    )
    .bind(target.org_id.to_string())
    .bind(target.queue_id.to_string())
    .bind(number)
    .bind(&status.sha)
    .bind(desired)
    .bind(&desired_hash)
    .bind(now)
    .fetch_optional(conn)
    .await?;

    Ok(is_unpublished.unwrap_or_default().then_some(desired_hash))
}

/// The publication of the status `status_hash` after the read `read`, the key of
/// its job: a status that comes back to an earlier value is published again.
fn publish_job(target: &QueueTarget, pr_number: i64, status_hash: &str, read: &str) -> Job {
    let key = format!(
        "{PUBLISH_CONDITIONS}:{}-{pr_number}-{}:1",
        target.queue_id,
        hash(&(status_hash, read))
    );
    let payload = json!(PublishConditions { pr_number });
    job(target, key, PUBLISH_CONDITIONS, JobClass::Repair, payload)
}

/// Makes the SCM show the last status of the PR. One job at a time publishes
/// for a PR, so that an older status never lands after a newer one: the job
/// holds the lock of the PR until it keeps what it published.
pub(crate) async fn publish(
    pool: &PgPool,
    scm: &dyn Scm,
    job: &Job,
    _now: OffsetDateTime,
) -> Result<(), Failure> {
    let request: PublishConditions = read_payload(job)?;
    let target = target_of(pool, job).await?;
    let mut lock = begin_system(pool).await?;
    let is_locked: bool =
        sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtext($1), $2::integer)")
            .bind(format!("pr_conditions:{}", target.queue_id))
            .bind(request.pr_number)
            .fetch_one(&mut *lock)
            .await?;
    if !is_locked {
        return Err(Failure::Retry(
            "another job publishes the status of this PR".into(),
        ));
    }

    let row: Option<(serde_json::Value, String, Option<String>)> = sqlx::query_as(
        "SELECT desired, desired_hash, published_hash FROM pr_conditions
         WHERE queue_id = $1 AND pr_number = $2",
    )
    .bind(target.queue_id.to_string())
    .bind(request.pr_number)
    .fetch_optional(&mut *lock)
    .await?;
    let Some((desired, desired_hash, published)) = row else {
        return Ok(());
    };

    if published.as_deref() == Some(desired_hash.as_str()) {
        return Ok(());
    }

    let status: ConditionsStatus =
        serde_json::from_value(desired).map_err(|err| Failure::Never(err.to_string()))?;
    let update = StatusUpdate {
        sha: status.sha.clone(),
        context: conditions_status_context(&target.target_branch),
        status: status.state.into(),
        description: status.description.clone(),
        details_url: None,
    };

    scm.set_status(&target.repo, &update, job.class).await?;

    sqlx::query(
        "UPDATE pr_conditions SET published_hash = $3 WHERE queue_id = $1 AND pr_number = $2",
    )
    .bind(target.queue_id.to_string())
    .bind(request.pr_number)
    .bind(&desired_hash)
    .execute(&mut *lock)
    .await?;

    lock.commit().await?;

    info!(
        pr_number = request.pr_number,
        state = ?status.state,
        "conditions status published"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn awaiting(check: &str) -> BlockReason {
        BlockReason::AwaitingRequiredCheck {
            check: check.into(),
            status: "pending".into(),
        }
    }

    #[test]
    fn the_status_says_what_keeps_the_pr_out_of_its_queue() {
        let failing = BlockReason::FailingRequiredCheck { check: "ci".into() };
        let cases = [
            (vec![], StatusState::Success, "Ready for the queue"),
            (
                vec![awaiting("ci")],
                StatusState::Pending,
                "The required check `ci` is pending",
            ),
            (
                vec![BlockReason::Undetermined, awaiting("lint")],
                StatusState::Pending,
                "The SCM does not say yet why the PR is blocked, and 1 more",
            ),
            // A reason that fails comes first, whatever its place.
            (
                vec![awaiting("lint"), failing, BlockReason::BehindBase],
                StatusState::Failure,
                "The required check `ci` failed, and 2 more",
            ),
        ];

        for (n, (blocks, state, description)) in cases.into_iter().enumerate() {
            assert_eq!(
                status_of(&blocks),
                (state, description.to_owned()),
                "case {n}"
            );
        }
    }
}

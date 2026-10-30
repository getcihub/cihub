//! The repair jobs: `refresh_checks` reads again the checks of a run in progress
//! that no report reached, and creates again its test branch when it is gone;
//! `refresh_target` reads again the head of the target branch. Each one gives its
//! facts back through `queue_inbox`, so that a lost webhook is only a delay
//! (principle V). `sync_queue` asks for every read at once.

use cihub_domain::{Actor, EventKind, EventType, JobClass, QueueId, RunId, RunStatus};
use cihub_mq_core::CheckRead;
use cihub_platform::{Job, enqueue_job};
use cihub_scm::{BranchMove, Scm};
use cihub_store::{PgPool, begin_system};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgConnection;
use time::OffsetDateTime;
use tracing::info;

use crate::MergeQueueError;
use crate::QueueLog;
use crate::conditions_status::observe_open_prs_job;
use crate::config::read_setup_job;
use crate::jobs::{
    Failure, give_back_facts, job, read_payload, refresh_reads, subject, target_of, window,
};
use crate::merge::MERGE_PR;
use crate::target::QueueTarget;

pub(crate) const REFRESH_CHECKS: &str = "refresh_checks";
pub(crate) const REFRESH_TARGET: &str = "refresh_target";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct RefreshChecks {
    pub run_id: RunId,
    pub test_sha: String,
    pub test_branch: String,
}

/// The read of the checks of `read`, at its read point.
pub(crate) fn refresh_checks(target: &QueueTarget, read: &CheckRead) -> Job {
    refresh_checks_job(target, read, &window(read.point), JobClass::Repair)
}

fn refresh_checks_job(
    target: &QueueTarget,
    read: &CheckRead,
    subject: &str,
    class: JobClass,
) -> Job {
    let request = RefreshChecks {
        run_id: read.run_id,
        test_sha: read.test_sha.clone(),
        test_branch: read.test_branch.clone(),
    };
    let key = format!("{REFRESH_CHECKS}:{}-{subject}:1", read.run_id);
    job(target, key, REFRESH_CHECKS, class, json!(request))
}

/// The read of the head of the target branch, at the read point `point`.
pub(crate) fn refresh_target(target: &QueueTarget, point: OffsetDateTime) -> Job {
    refresh_target_job(target, &window(point), JobClass::Repair)
}

/// The read of the head of the target branch after the delivery
/// `delivery_id` of a push, when the routing gave the push to no queue
/// because the branch had none yet.
pub(crate) fn refresh_target_after(target: &QueueTarget, delivery_id: &str) -> Job {
    refresh_target_job(target, &subject(delivery_id), JobClass::Interactive)
}

fn refresh_target_job(target: &QueueTarget, subject: &str, class: JobClass) -> Job {
    let key = format!("{REFRESH_TARGET}:{}-{subject}:1", target.queue_id);
    job(target, key, REFRESH_TARGET, class, json!({}))
}

/// A merge job of `queue` waits or runs, or its success waits in the inbox:
/// the push of its merge may come before its fact. The job gives back its
/// fact before it ends, so a reconciliation that read the inbox before that
/// fact sees it here.
pub(crate) async fn merge_in_flight(
    conn: &mut PgConnection,
    queue: QueueId,
) -> Result<bool, MergeQueueError> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM jobs
                        WHERE queue_id = $1 AND type = $2 AND state IN ('queued', 'running'))
             OR EXISTS (SELECT 1 FROM queue_inbox
                        WHERE queue_id = $1 AND type = $3::event_type)",
    )
    .bind(queue.to_string())
    .bind(MERGE_PR)
    .bind(EventType::MergeSucceeded.as_str())
    .fetch_one(&mut *conn)
    .await
    .map_err(MergeQueueError::Query)
}

/// Reads the checks of the test SHA of a run still in progress and gives
/// them back; creates again the test branch when it is gone.
pub(crate) async fn read_checks(
    pool: &PgPool,
    scm: &dyn Scm,
    job: &Job,
    now: OffsetDateTime,
) -> Result<(), Failure> {
    let request: RefreshChecks = read_payload(job)?;
    let target = target_of(pool, job).await?;
    let state = crate::assembly::run_state(pool, request.run_id).await?;

    if state.map(|(status, _)| status) != Some(RunStatus::InProgress) {
        return Ok(());
    }

    let repo = &target.repo;
    if scm
        .branch_head(repo, &request.test_branch, job.class)
        .await?
        .is_none()
    {
        info!(run_id = %request.run_id, "test branch gone: created again at its test SHA");
        scm.set_branch(
            repo,
            &request.test_branch,
            &request.test_sha,
            BranchMove::FastForwardOnly,
            job.class,
        )
        .await?;
    }

    let checks = scm.checks(repo, &request.test_sha, job.class).await?;
    let facts = checks
        .into_iter()
        .map(|check| {
            (
                Actor::System,
                EventKind::CheckObserved {
                    test_sha: request.test_sha.clone(),
                    check,
                },
            )
        })
        .collect();

    give_back_facts(pool, &target, facts, now).await
}

/// Reads the head of the target branch and gives it back, unless a merge
/// of the queue is in flight: its head would look like a push from outside
/// before the fact of the merge.
pub(crate) async fn read_target(
    pool: &PgPool,
    scm: &dyn Scm,
    job: &Job,
    now: OffsetDateTime,
) -> Result<(), Failure> {
    let target = target_of(pool, job).await?;
    let mut tx = begin_system(pool).await?;
    let merging = merge_in_flight(&mut tx, target.queue_id).await?;

    tx.commit().await?;

    if merging {
        return Ok(());
    }

    let Some(after) = scm
        .branch_head(&target.repo, &target.target_branch, job.class)
        .await?
    else {
        return Ok(());
    };

    let fact = EventKind::TargetPushed {
        before: None,
        after,
        sender_id: None,
    };

    give_back_facts(pool, &target, vec![(Actor::System, fact)], now).await
}

/// Asks, in the class `interactive`, for a read of each live PR of `queue`, of
/// the checks of each run in progress and of the head of its target, and, in
/// the class `repair`, for a read of each open PR of the target, for its
/// conditions status;
/// returns the number of new jobs. A second sync in the same minute asks for nothing.
///
/// # Errors
///
/// Postgres fails, or the queue does not exist; nothing is asked.
pub async fn sync_queue(
    pool: &PgPool,
    queue: QueueId,
    now: OffsetDateTime,
) -> Result<usize, MergeQueueError> {
    let mut tx = begin_system(pool).await?;
    let target = QueueTarget::load(&mut tx, queue).await?;
    let log = QueueLog::load(&mut tx, queue).await?;
    let subject = format!("sync-{}", window(now));
    let snapshot = &log.snapshot;
    let prs: Vec<u64> = snapshot.live_items().map(|i| i.pr_number).collect();
    let mut jobs = refresh_reads(&target, &subject, JobClass::Interactive, &prs);

    for run in snapshot.runs_in_progress() {
        let Some(test_sha) = run.test_sha.clone() else {
            continue;
        };
        let read = CheckRead {
            run_id: run.id,
            test_sha,
            test_branch: run.test_branch.clone(),
            point: now,
        };
        jobs.push(refresh_checks_job(
            &target,
            &read,
            &subject,
            JobClass::Interactive,
        ));
    }

    jobs.push(refresh_target_job(&target, &subject, JobClass::Interactive));
    jobs.push(read_setup_job(&target, &subject, JobClass::Interactive));
    jobs.push(observe_open_prs_job(&target, &subject));

    let mut requested = 0;
    for job in &jobs {
        if enqueue_job(&mut tx, job, now, now).await? {
            requested += 1;
        }
    }

    tx.commit().await?;
    info!(%queue, jobs = requested, "queue sync requested");

    Ok(requested)
}

#[cfg(test)]
mod tests {
    use ulid::Ulid;

    use super::*;

    #[test]
    fn keys_name_the_run_or_the_queue_and_the_point() {
        let target = crate::target::test_target();
        let point = OffsetDateTime::from_unix_timestamp(1_790_000_000).unwrap();
        let run_id = RunId::from_ulid(Ulid(7));
        let read = CheckRead {
            run_id,
            test_sha: "sha".into(),
            test_branch: "cihub-merge/pr-1/run".into(),
            point,
        };
        let checks = refresh_checks(&target, &read);

        assert_eq!(
            checks.idempotency_key,
            format!("refresh_checks:{run_id}-20260921T1413:1")
        );
        assert_eq!(checks.class, JobClass::Repair);

        let head = refresh_target(&target, point);
        assert_eq!(
            head.idempotency_key,
            format!("refresh_target:{}-20260921T1413:1", target.queue_id)
        );
        assert_eq!(head.class, JobClass::Repair);
    }
}

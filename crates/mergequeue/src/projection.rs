//! The projections of the log: each event that `apply` accepted writes the
//! same change in `queue_items`, `test_groups`, `test_runs` and `queues`, at
//! its writing and at a replay. A transition is guarded by the state before
//! (`UPDATE … WHERE state = $from`), and its row count is checked.

use cihub_domain::{
    CheckObservation, CheckReport, Event, EventKind, Item, OrgId, Priority, QueueId, RunStatus,
};
use cihub_mq_core::Snapshot;
use sqlx::PgConnection;
use sqlx::postgres::PgQueryResult;
use time::OffsetDateTime;

use crate::MergeQueueError;

/// Writes the change of `event`, the event `seq` of its log; `after` is the
/// snapshot once `event` is applied.
#[allow(
    clippy::too_many_lines,
    reason = "one arm per event, each short and independent"
)]
pub(crate) async fn project(
    conn: &mut PgConnection,
    seq: i64,
    event: &Event,
    after: &Snapshot,
) -> Result<(), MergeQueueError> {
    let at = event.occurred_at;
    let mismatch = || MergeQueueError::Projection(event.id);
    let number = |n: u64| i64::try_from(n).map_err(|_| mismatch());
    match &event.kind {
        EventKind::ItemTransitioned {
            item_id,
            from: None,
            ..
        } => {
            let item = after.items.get(item_id).ok_or_else(mismatch)?;
            create_item(conn, event, item, number(item.pr_number)?).await?;
        }
        EventKind::ItemTransitioned {
            item_id,
            from: Some(from),
            to,
            reason,
            position,
            verified_by_run_id,
            merged_sha,
        } => {
            let done = sqlx::query(
                "UPDATE queue_items SET
                     state = $3::item_state, state_reason = $4::reason, state_changed_at = $5,
                     position = coalesce($6, position),
                     verified_by_run_id = CASE WHEN $3 = 'pending' THEN NULL
                                               ELSE coalesce($7, verified_by_run_id) END,
                     group_id = CASE WHEN $3 = 'pending' THEN NULL ELSE group_id END,
                     merged_sha = coalesce($8, merged_sha),
                     seq = CASE WHEN $2 = 'not_ready' AND $3 = 'pending' THEN $9 ELSE seq END,
                     concluded_at = CASE WHEN $10 THEN $5 END
                 WHERE id = $1 AND state = $2::item_state",
            )
            .bind(item_id.to_string())
            .bind(from.as_str())
            .bind(to.as_str())
            .bind(reason.as_str())
            .bind(at)
            .bind(position)
            .bind(verified_by_run_id.map(|id| id.to_string()))
            .bind(merged_sha)
            .bind(seq)
            .bind(to.is_terminal())
            .execute(&mut *conn)
            .await;
            one_row(done, event)?;
        }
        EventKind::ItemReadinessChanged {
            item_id,
            mergeability,
            block_reasons,
        } => {
            let blocks = serde_json::to_value(block_reasons).map_err(MergeQueueError::Snapshot)?;
            let done = sqlx::query(
                "UPDATE queue_items SET mergeability = $2::mergeability, block_reasons = $3
                 WHERE id = $1",
            )
            .bind(item_id.to_string())
            .bind(mergeability.as_str())
            .bind(blocks)
            .execute(&mut *conn)
            .await;
            one_row(done, event)?;
        }
        EventKind::QueueRenumbered { positions } => {
            for (item_id, position) in positions {
                let done = sqlx::query("UPDATE queue_items SET position = $2 WHERE id = $1")
                    .bind(item_id.to_string())
                    .bind(position)
                    .execute(&mut *conn)
                    .await;
                one_row(done, event)?;
            }
        }
        EventKind::GroupFormed {
            group_id,
            kind,
            item_ids,
            predecessor_ids,
            base_sha,
            parent_group_id,
        } => {
            let ids = |ids: &[cihub_domain::ItemId]| -> Vec<String> {
                ids.iter().map(ToString::to_string).collect()
            };
            sqlx::query(
                "INSERT INTO test_groups (id, org_id, queue_id, kind, item_ids, parent_group_id,
                                          predecessor_ids, base_sha, seq, state, state_reason,
                                          created_at)
                 VALUES ($1, $2, $3, $4::group_kind, $5, $6, $7, $8, $9, 'testing',
                         'test_run_started', $10)",
            )
            .bind(group_id.to_string())
            .bind(event.org_id.to_string())
            .bind(event.queue_id.to_string())
            .bind(kind.as_str())
            .bind(ids(item_ids))
            .bind(parent_group_id.map(|id| id.to_string()))
            .bind(ids(predecessor_ids))
            .bind(base_sha)
            .bind(seq)
            .bind(at)
            .execute(&mut *conn)
            .await
            .map_err(MergeQueueError::Query)?;
            sqlx::query("UPDATE queue_items SET group_id = $1 WHERE id = ANY($2)")
                .bind(group_id.to_string())
                .bind(ids(item_ids))
                .execute(&mut *conn)
                .await
                .map_err(MergeQueueError::Query)?;
        }
        EventKind::GroupTransitioned {
            group_id,
            from,
            to,
            reason,
        } => {
            let done = sqlx::query(
                "UPDATE test_groups SET state = $3::group_state, state_reason = $4::reason,
                                        concluded_at = CASE WHEN $6 THEN $5 END
                 WHERE id = $1 AND state = $2::group_state",
            )
            .bind(group_id.to_string())
            .bind(from.as_str())
            .bind(to.as_str())
            .bind(reason.as_str())
            .bind(at)
            .bind(to.is_terminal())
            .execute(&mut *conn)
            .await;
            one_row(done, event)?;
        }
        EventKind::RunTransitioned {
            run_id,
            group_id,
            from: None,
            to,
            reason,
            attempt,
            test_branch,
            ..
        } => {
            let group = after.groups.get(group_id).ok_or_else(mismatch)?;
            let items = |ids: &[cihub_domain::ItemId]| {
                ids.iter()
                    .filter_map(|id| after.items.get(id))
                    .collect::<Vec<_>>()
            };
            let head_shas: Vec<String> = items(&group.predecessor_ids)
                .into_iter()
                .chain(items(&group.item_ids))
                .map(|item| item.head_sha.clone())
                .collect();
            let tested: Vec<i64> = items(&group.item_ids)
                .into_iter()
                .map(|item| number(item.pr_number))
                .collect::<Result<_, _>>()?;
            sqlx::query(
                "INSERT INTO test_runs (id, org_id, queue_id, group_id, attempt, test_branch,
                                        status, status_reason, required_statuses, started_at,
                                        failure_causing_statuses, base_sha, head_shas,
                                        tested_pr_numbers)
                 VALUES ($1, $2, $3, $4, $5, $6, $7::run_status, $8::reason, '{}', $9, '{}', $10,
                         $11, $12)",
            )
            .bind(run_id.to_string())
            .bind(event.org_id.to_string())
            .bind(event.queue_id.to_string())
            .bind(group_id.to_string())
            .bind(i32::try_from(*attempt).map_err(|_| mismatch())?)
            .bind(test_branch)
            .bind(to.as_str())
            .bind(reason.as_str())
            .bind(at)
            .bind(&group.base_sha)
            .bind(head_shas)
            .bind(tested)
            .execute(&mut *conn)
            .await
            .map_err(MergeQueueError::Query)?;
        }
        EventKind::RunTransitioned {
            run_id,
            from: Some(from),
            to,
            reason,
            failure_causing_statuses,
            ..
        } => {
            let failed = matches!(to, RunStatus::Failed | RunStatus::TimedOut);
            let done = sqlx::query(
                "UPDATE test_runs SET
                     status = $3::run_status, status_reason = $4::reason,
                     ended_at = CASE WHEN $6 THEN $5 ELSE ended_at END,
                     failure_reason = CASE WHEN $7 THEN $4::reason ELSE failure_reason END,
                     failure_causing_statuses = CASE WHEN $7 THEN $8
                                                     ELSE failure_causing_statuses END
                 WHERE id = $1 AND status = $2::run_status",
            )
            .bind(run_id.to_string())
            .bind(from.as_str())
            .bind(to.as_str())
            .bind(reason.as_str())
            .bind(at)
            .bind(to.is_terminal())
            .bind(failed)
            .bind(failure_causing_statuses)
            .execute(&mut *conn)
            .await;
            one_row(done, event)?;
        }
        EventKind::QueueTransitioned { .. } => {
            let done = sqlx::query(
                "UPDATE queues SET state = $2::queue_state, mode = $3::queue_mode WHERE id = $1",
            )
            .bind(event.queue_id.to_string())
            .bind(after.state.as_str())
            .bind(after.mode.as_str())
            .execute(&mut *conn)
            .await;
            one_row(done, event)?;
        }
        EventKind::TestBranchAssembled {
            run_id,
            test_sha,
            draft_pr_number,
            required_statuses,
            required_statuses_source,
        } => {
            let draft = draft_pr_number.map(number).transpose()?;
            let done = sqlx::query(
                "UPDATE test_runs SET test_sha = $2, draft_pr_number = $3, required_statuses = $4,
                                      required_statuses_source = $5::required_statuses_source
                 WHERE id = $1",
            )
            .bind(run_id.to_string())
            .bind(test_sha)
            .bind(draft)
            .bind(required_statuses)
            .bind(required_statuses_source.as_str())
            .execute(&mut *conn)
            .await;
            one_row(done, event)?;
        }
        EventKind::TestBranchAssemblyFailed {
            run_id,
            conflict_culprit,
            ..
        } => {
            let culprit = conflict_culprit
                .as_ref()
                .map(serde_json::to_value)
                .transpose()
                .map_err(MergeQueueError::Snapshot)?;
            let done = sqlx::query("UPDATE test_runs SET conflict_culprit = $2 WHERE id = $1")
                .bind(run_id.to_string())
                .bind(culprit)
                .execute(&mut *conn)
                .await;
            one_row(done, event)?;
        }
        EventKind::CheckObserved { test_sha, check } => {
            let run = after
                .runs
                .values()
                .find(|r| !r.status.is_terminal() && r.test_sha.as_deref() == Some(test_sha))
                .ok_or_else(mismatch)?;
            let observed = after
                .checks
                .get(&run.id)
                .and_then(|checks| checks.get(&(check.name.clone(), check.kind)))
                .ok_or_else(mismatch)?;
            upsert_check(conn, &observed.value).await?;
        }
        // The other facts change only the observed state, and the window has
        // no column before lot 8.
        EventKind::ItemSubmitted { .. }
        | EventKind::ItemCancelRequested { .. }
        | EventKind::ItemRestartRequested { .. }
        | EventKind::PrObserved { .. }
        | EventKind::TargetPushed { .. }
        | EventKind::TargetsReceived { .. }
        | EventKind::MergeSucceeded { .. }
        | EventKind::MergeFailed { .. }
        | EventKind::QueueControlRequested { .. }
        | EventKind::QueueSettingsChanged { .. }
        | EventKind::AccessLost
        | EventKind::AccessRestored
        | EventKind::QueueWindowChanged { .. } => {}
    }
    Ok(())
}

/// A new item, from the submission that caused it: the submission gives its
/// author and the account that submitted it.
async fn create_item(
    conn: &mut PgConnection,
    event: &Event,
    item: &Item,
    pr_number: i64,
) -> Result<(), MergeQueueError> {
    sqlx::query("UPDATE queue_items SET is_current = false WHERE queue_id = $1 AND pr_number = $2")
        .bind(event.queue_id.to_string())
        .bind(pr_number)
        .execute(&mut *conn)
        .await
        .map_err(MergeQueueError::Query)?;
    let blocks = serde_json::to_value(&item.block_reasons).map_err(MergeQueueError::Snapshot)?;
    let done = sqlx::query(
        "INSERT INTO queue_items (id, org_id, queue_id, pr_number, pr_head_sha, pr_author_id,
                                  state, state_reason, state_changed_at, priority_value,
                                  priority_name, no_batch, force_enqueued, skip_the_line,
                                  submitted_by, submitted_via, submitted_at, mergeability,
                                  block_reasons, is_current)
         SELECT $1, $2, $3, $4, $5, s.payload->>'author_id', $6::item_state, $7::reason, $8, $9,
                $10, $11, $12, $13, s.actor_user_id, $14::submitted_via, s.occurred_at,
                $15::mergeability, $16, true
         FROM events s
         WHERE s.queue_id = $3 AND s.id = $17 AND s.type = 'item.submitted'",
    )
    .bind(item.id.to_string())
    .bind(event.org_id.to_string())
    .bind(event.queue_id.to_string())
    .bind(pr_number)
    .bind(&item.head_sha)
    .bind(item.state.as_str())
    .bind(item.state_reason.as_str())
    .bind(event.occurred_at)
    .bind(i16::from(item.priority.0))
    .bind(item.priority.name())
    .bind(item.no_batch)
    .bind(item.force_enqueued)
    .bind(item.priority == Priority::URGENT)
    .bind(item.submitted_via.as_str())
    .bind(item.mergeability.as_str())
    .bind(blocks)
    .bind(event.cause_id.map(|id| id.to_string()))
    .execute(conn)
    .await;
    one_row(done, event)
}

/// Writes the last report of a check on the test SHA of its run, when it is
/// newer than the one stored, and the time of the last report of the run.
/// The same rows come from a fact of the log and from a late report (D118),
/// so a replay of the log keeps the late reports.
pub(crate) async fn upsert_check(
    conn: &mut PgConnection,
    check: &CheckObservation,
) -> Result<(), MergeQueueError> {
    sqlx::query(
        "INSERT INTO check_observations (org_id, run_id, name, kind, required, conclusion,
                                         details_url, observed_at)
         VALUES ($1, $2, $3, $4::check_kind, $5, $6::check_conclusion, $7, $8)
         ON CONFLICT (run_id, name, kind) DO UPDATE
             SET required = excluded.required, conclusion = excluded.conclusion,
                 details_url = excluded.details_url, observed_at = excluded.observed_at
             WHERE excluded.observed_at >= check_observations.observed_at",
    )
    .bind(check.org_id.to_string())
    .bind(check.run_id.to_string())
    .bind(&check.name)
    .bind(check.kind.as_str())
    .bind(check.required)
    .bind(check.conclusion.map(cihub_domain::CheckConclusion::as_str))
    .bind(&check.details_url)
    .bind(check.observed_at)
    .execute(&mut *conn)
    .await
    .map_err(MergeQueueError::Query)?;
    sqlx::query(
        "UPDATE test_runs
         SET last_report_at = (SELECT max(observed_at) FROM check_observations WHERE run_id = $1)
         WHERE id = $1",
    )
    .bind(check.run_id.to_string())
    .execute(conn)
    .await
    .map_err(MergeQueueError::Query)?;
    Ok(())
}

/// Stores a report on the test SHA of a run of `queue` that is no longer
/// live, without a fact in the log (`AGENTS.md` D118). Returns `false` when
/// no run of the queue has this test SHA.
pub(crate) async fn record_late_check(
    conn: &mut PgConnection,
    org_id: OrgId,
    queue: QueueId,
    test_sha: &str,
    check: &CheckReport,
    at: OffsetDateTime,
) -> Result<bool, MergeQueueError> {
    let run: Option<(String, Vec<String>)> = sqlx::query_as(
        "SELECT id, required_statuses FROM test_runs
         WHERE queue_id = $1 AND test_sha = $2
         ORDER BY started_at DESC LIMIT 1",
    )
    .bind(queue.to_string())
    .bind(test_sha)
    .fetch_optional(&mut *conn)
    .await
    .map_err(MergeQueueError::Query)?;
    let Some((run_id, required)) = run else {
        return Ok(false);
    };
    let observation = CheckObservation {
        org_id,
        run_id: run_id
            .parse()
            .map_err(|source| MergeQueueError::ReadQueue { id: run_id, source })?,
        name: check.name.clone(),
        kind: check.kind,
        required: required.contains(&check.name),
        conclusion: check.conclusion,
        details_url: check.details_url.clone(),
        observed_at: at,
    };
    upsert_check(conn, &observation).await?;
    Ok(true)
}

fn one_row(done: Result<PgQueryResult, sqlx::Error>, event: &Event) -> Result<(), MergeQueueError> {
    match done {
        Ok(done) if done.rows_affected() == 1 => Ok(()),
        Ok(_) => Err(MergeQueueError::Projection(event.id)),
        Err(err) => Err(MergeQueueError::Query(err)),
    }
}

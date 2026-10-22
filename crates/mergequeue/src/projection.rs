//! The projections of the log: each event that `apply` accepted writes the
//! same change in `queue_items`, `test_groups`, `test_runs` and `queues`, at
//! its writing and at a replay. A transition is guarded by the state before
//! (`UPDATE … WHERE state = $from`), and its row count is checked.

use cihub_domain::{
    CheckObservation, CheckReport, Event, EventKind, EventType, Item, ItemId, ItemState,
    Mergeability, OrgId, QueueId, Reason, RunStatus,
};
use cihub_mq_core::Snapshot;
use cihub_store::event_kind;
use sqlx::PgConnection;
use sqlx::postgres::PgQueryResult;
use time::OffsetDateTime;

use crate::{MergeQueueError, parse_id};

/// Writes the change of `event`; `after` is the snapshot once `event` is
/// applied.
#[allow(
    clippy::too_many_lines,
    reason = "one arm per event, each short and independent"
)]
pub(crate) async fn project(
    conn: &mut PgConnection,
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
            to,
            reason,
            ..
        } => {
            let item = match after.items.get(item_id) {
                Some(item) => item.clone(),
                // A refused submission is concluded at once, so `apply`
                // already evicted it.
                None => refused_item(conn, event, *item_id, *to, *reason)
                    .await?
                    .ok_or_else(mismatch)?,
            };
            let url = after
                .prs
                .get(&item.pr_number)
                .map(|pr| pr.value.url.as_str());
            create_item(conn, event, &item, number(item.pr_number)?, url).await?;
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
                     merged_sha = coalesce($8, merged_sha)
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
            .execute(&mut *conn)
            .await;
            one_row(done, event)?;
        }
        EventKind::ItemReadinessChanged {
            item_id,
            block_reasons,
            ..
        } => {
            let blocks = serde_json::to_value(block_reasons).map_err(MergeQueueError::Snapshot)?;
            let done = sqlx::query("UPDATE queue_items SET block_reasons = $2 WHERE id = $1")
                .bind(item_id.to_string())
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
            item_ids,
            predecessor_ids,
            ..
        } => {
            let ids = |ids: &[cihub_domain::ItemId]| -> Vec<String> {
                ids.iter().map(ToString::to_string).collect()
            };
            sqlx::query(
                "INSERT INTO test_groups (id, org_id, queue_id, item_ids, predecessor_ids, state)
                 VALUES ($1, $2, $3, $4, $5, 'testing')",
            )
            .bind(group_id.to_string())
            .bind(event.org_id.to_string())
            .bind(event.queue_id.to_string())
            .bind(ids(item_ids))
            .bind(ids(predecessor_ids))
            .execute(&mut *conn)
            .await?;
            sqlx::query("UPDATE queue_items SET group_id = $1 WHERE id = ANY($2)")
                .bind(group_id.to_string())
                .bind(ids(item_ids))
                .execute(&mut *conn)
                .await?;
        }
        EventKind::GroupTransitioned {
            group_id, from, to, ..
        } => {
            let done = sqlx::query(
                "UPDATE test_groups SET state = $3::group_state
                 WHERE id = $1 AND state = $2::group_state",
            )
            .bind(group_id.to_string())
            .bind(from.as_str())
            .bind(to.as_str())
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
            .await?;
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
            // A decision acts as CIHub: the person who asked for the state is
            // the actor of its cause.
            let done = sqlx::query(
                "UPDATE queues SET state = $2::queue_state, mode = $3::queue_mode,
                                   state_changed_at = $4,
                                   state_changed_by = (SELECT actor_user_id FROM events
                                                       WHERE queue_id = $1 AND id = $5)
                 WHERE id = $1",
            )
            .bind(event.queue_id.to_string())
            .bind(after.state.as_str())
            .bind(after.mode.as_str())
            .bind(at)
            .bind(event.cause_id.map(|id| id.to_string()))
            .execute(&mut *conn)
            .await;
            one_row(done, event)?;
        }
        EventKind::TargetPushed { .. } => {
            let head = after.target.head.as_ref().map(|head| &head.value);
            let done = sqlx::query("UPDATE queues SET target_head_sha = $2 WHERE id = $1")
                .bind(event.queue_id.to_string())
                .bind(head)
                .execute(&mut *conn)
                .await;
            one_row(done, event)?;
        }
        EventKind::QueueWindowChanged { after: window, .. } => {
            let done = sqlx::query("UPDATE queues SET window_size = $2 WHERE id = $1")
                .bind(event.queue_id.to_string())
                .bind(i32::try_from(*window).map_err(|_| mismatch())?)
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
        EventKind::PrObserved { pr_number, url, .. } => {
            sqlx::query(
                "UPDATE queue_items SET pr_url = $3
                 WHERE queue_id = $1 AND pr_number = $2 AND is_current
                   AND state NOT IN ('merged', 'failed', 'cancelled')",
            )
            .bind(event.queue_id.to_string())
            .bind(number(*pr_number)?)
            .bind(url)
            .execute(&mut *conn)
            .await?;
        }
        // The other facts change only the observed state.
        EventKind::ItemSubmitted { .. }
        | EventKind::ItemCancelRequested { .. }
        | EventKind::ItemRestartRequested { .. }
        | EventKind::TargetsReceived { .. }
        | EventKind::MergeSucceeded { .. }
        | EventKind::MergeFailed { .. }
        | EventKind::QueueControlRequested { .. }
        | EventKind::QueueSettingsChanged { .. }
        | EventKind::AccessLost
        | EventKind::AccessRestored => {}
    }

    Ok(())
}

/// The item of a refused submission, from the `item.submitted` that caused
/// it, or `None` without that fact.
async fn refused_item(
    conn: &mut PgConnection,
    event: &Event,
    item_id: ItemId,
    state: ItemState,
    reason: Reason,
) -> Result<Option<Item>, MergeQueueError> {
    let payload: Option<serde_json::Value> = sqlx::query_scalar(
        "SELECT payload FROM events WHERE queue_id = $1 AND id = $2 AND type = 'item.submitted'",
    )
    .bind(event.queue_id.to_string())
    .bind(event.cause_id.map(|id| id.to_string()))
    .fetch_optional(&mut *conn)
    .await?;

    let Some(payload) = payload else {
        return Ok(None);
    };

    let EventKind::ItemSubmitted {
        pr_number,
        head_sha,
        priority,
        no_batch,
        direct,
        submitted_via,
        profile,
        ..
    } = event_kind(EventType::ItemSubmitted.as_str(), &payload)
        .map_err(MergeQueueError::Snapshot)?
    else {
        return Ok(None);
    };

    Ok(Some(Item {
        id: item_id,
        org_id: event.org_id,
        queue_id: event.queue_id,
        pr_number,
        head_sha,
        state,
        state_reason: reason,
        priority,
        no_batch,
        profile,
        force_enqueued: direct,
        submitted_via,
        position: None,
        is_current: false,
        mergeability: Mergeability::Unknown,
        block_reasons: Vec::new(),
        group_id: None,
        verified_by_run_id: None,
        merged_sha: None,
        submitted_at: event.occurred_at,
        admitted_at: None,
        direct_merge_tried: false,
    }))
}

/// A new item, from the submission that caused it: the submission gives its
/// author and the account that submitted it. `url` is the page of its PR at
/// the last observation of the PR, when there is one.
async fn create_item(
    conn: &mut PgConnection,
    event: &Event,
    item: &Item,
    pr_number: i64,
    url: Option<&str>,
) -> Result<(), MergeQueueError> {
    // A refused submission never becomes the live item of its PR.
    if item.is_current {
        sqlx::query(
            "UPDATE queue_items SET is_current = false WHERE queue_id = $1 AND pr_number = $2",
        )
        .bind(event.queue_id.to_string())
        .bind(pr_number)
        .execute(&mut *conn)
        .await?;
    }

    let blocks = serde_json::to_value(&item.block_reasons).map_err(MergeQueueError::Snapshot)?;
    let done = sqlx::query(
        "INSERT INTO queue_items (id, org_id, queue_id, pr_number, pr_head_sha, pr_author_id,
                                  state, state_reason, state_changed_at, priority_value,
                                  no_batch, force_enqueued, submitted_by, submitted_via,
                                  submitted_at, block_reasons, is_current, sender_permission,
                                  profile, pr_url)
         SELECT $1, $2, $3, $4, $5, s.payload->>'author_id', $6::item_state, $7::reason, $8, $9,
                $10, $11, s.actor_user_id, $12::submitted_via, s.occurred_at,
                $13, $15, (s.payload->>'sender_permission')::permission, s.payload->>'profile', $16
         FROM events s
         WHERE s.queue_id = $3 AND s.id = $14 AND s.type = 'item.submitted'",
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
    .bind(item.no_batch)
    .bind(item.force_enqueued)
    .bind(item.submitted_via.as_str())
    .bind(blocks)
    .bind(event.cause_id.map(|id| id.to_string()))
    .bind(item.is_current)
    .bind(url)
    .execute(conn)
    .await;

    one_row(done, event)
}

/// Writes the last report of a check on the test SHA of its run, when it is
/// newer than the one stored. The same rows come from a fact of the log and
/// from a late report, so a replay of the log keeps the late reports.
async fn upsert_check(
    conn: &mut PgConnection,
    check: &CheckObservation,
) -> Result<(), MergeQueueError> {
    sqlx::query(
        "INSERT INTO check_observations (org_id, run_id, name, kind, required, status, conclusion,
                                         details_url, started_at, completed_at, observed_at)
         VALUES ($1, $2, $3, $4::check_kind, $5, $6::check_status, $7::check_conclusion, $8, $9,
                 $10, $11)
         ON CONFLICT (run_id, name, kind) DO UPDATE
             SET required = excluded.required, status = excluded.status,
                 conclusion = excluded.conclusion, details_url = excluded.details_url,
                 started_at = excluded.started_at, completed_at = excluded.completed_at,
                 observed_at = excluded.observed_at
             WHERE excluded.observed_at >= check_observations.observed_at",
    )
    .bind(check.org_id.to_string())
    .bind(check.run_id.to_string())
    .bind(&check.name)
    .bind(check.kind.as_str())
    .bind(check.required)
    .bind(check.status.as_str())
    .bind(check.conclusion.map(cihub_domain::CheckConclusion::as_str))
    .bind(&check.details_url)
    .bind(check.started_at)
    .bind(check.completed_at)
    .bind(check.observed_at)
    .execute(conn)
    .await?;

    Ok(())
}

/// Stores a report on the test SHA of a run of `queue` that is no longer live,
/// without a fact in the log. Does nothing when no run of the queue has this
/// test SHA.
pub(crate) async fn record_late_check(
    conn: &mut PgConnection,
    org_id: OrgId,
    queue: QueueId,
    test_sha: &str,
    check: &CheckReport,
    at: OffsetDateTime,
) -> Result<(), MergeQueueError> {
    let run: Option<(String, Vec<String>)> = sqlx::query_as(
        "SELECT id, required_statuses FROM test_runs
         WHERE queue_id = $1 AND test_sha = $2
         ORDER BY started_at DESC LIMIT 1",
    )
    .bind(queue.to_string())
    .bind(test_sha)
    .fetch_optional(&mut *conn)
    .await?;

    let Some((run_id, required)) = run else {
        return Ok(());
    };

    let observation = CheckObservation {
        org_id,
        run_id: parse_id(&run_id, &run_id)?,
        name: check.name.clone(),
        kind: check.kind,
        required: required.contains(&check.name),
        status: check.status,
        conclusion: check.conclusion,
        details_url: check.details_url.clone(),
        started_at: check.started_at,
        completed_at: check.completed_at,
        observed_at: at,
    };

    upsert_check(conn, &observation).await?;

    Ok(())
}

fn one_row(done: Result<PgQueryResult, sqlx::Error>, event: &Event) -> Result<(), MergeQueueError> {
    match done {
        Ok(done) if done.rows_affected() == 1 => Ok(()),
        Ok(_) => Err(MergeQueueError::Projection(event.id)),
        Err(err) => Err(MergeQueueError::Query(err)),
    }
}

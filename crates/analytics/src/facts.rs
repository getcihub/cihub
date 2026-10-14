use std::collections::BTreeMap;

use cihub_domain::{
    CheckConclusion, CheckKind, CheckReport, Event, EventKind, GroupId, ItemId, ItemState, Reason,
    RunId, RunStatus,
};
use cihub_store::event_kind;
use serde_json::Value;
use sqlx::PgConnection;
use time::OffsetDateTime;

use crate::AnalyticsError;

/// The key of the cursors of the facts in `log_cursors`.
pub const SUBSCRIPTION: &str = "analytics";

/// Writes the facts that `events`, in the order of their log, conclude: an
/// ended item, an ended run. The caller advances its cursor in the same
/// transaction, so that each fact is written once; a fact already written,
/// after a cursor read the log again, stays as it is.
///
/// # Errors
///
/// Postgres fails, or an event of the log does not match `cihub-domain`.
pub async fn record_facts(
    conn: &mut PgConnection,
    events: &[(i64, Event)],
) -> Result<(), AnalyticsError> {
    for (seq, event) in events {
        match &event.kind {
            EventKind::ItemTransitioned { item_id, to, .. } if to.is_terminal() => {
                record_item(conn, event, *item_id).await?;
            }
            EventKind::RunTransitioned { run_id, to, .. } if to.is_terminal() => {
                record_run(conn, event, *seq, *run_id).await?;
            }
            _ => {}
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Transition {
    at: OffsetDateTime,
    from: Option<ItemState>,
    to: ItemState,
    reason: Reason,
}

#[derive(Debug, PartialEq, Eq)]
struct StatePass {
    state: ItemState,
    entered_at: OffsetDateTime,
    left_at: OffsetDateTime,
    exit_reason: Reason,
}

#[derive(Debug, PartialEq, Eq)]
struct ItemFacts {
    conclusion: ItemState,
    reason: Reason,
    submitted_at: OffsetDateTime,
    concluded_at: OffsetDateTime,
    passes: Vec<StatePass>,
}

/// The facts of an item from its transitions, its creation first. `None` when
/// its creation left the log with its partition: its times would be wrong.
fn item_facts(transitions: &[Transition]) -> Option<ItemFacts> {
    let (first, last) = (transitions.first()?, transitions.last()?);
    if first.from.is_some() || !last.to.is_terminal() {
        return None;
    }

    let passes = transitions
        .windows(2)
        .map(|pair| StatePass {
            state: pair[0].to,
            entered_at: pair[0].at,
            left_at: pair[1].at,
            exit_reason: pair[1].reason,
        })
        .collect();
    Some(ItemFacts {
        conclusion: last.to,
        reason: last.reason,
        submitted_at: first.at,
        concluded_at: last.at,
        passes,
    })
}

async fn record_item(
    conn: &mut PgConnection,
    event: &Event,
    item: ItemId,
) -> Result<(), AnalyticsError> {
    let rows: Vec<(String, Value, OffsetDateTime)> = sqlx::query_as(
        "SELECT id, payload, occurred_at FROM events
         WHERE item_id = $1 AND type = 'item.transitioned' ORDER BY seq",
    )
    .bind(item.to_string())
    .fetch_all(&mut *conn)
    .await?;
    let mut transitions = Vec::with_capacity(rows.len());
    for (id, payload, at) in rows {
        if let EventKind::ItemTransitioned {
            from, to, reason, ..
        } = read_event(&id, "item.transitioned", &payload)?
        {
            transitions.push(Transition {
                at,
                from,
                to,
                reason,
            });
        }
    }
    let Some(facts) = item_facts(&transitions) else {
        return Ok(());
    };

    sqlx::query(
        "INSERT INTO an_item_conclusions (item_id, org_id, queue_id, conclusion, reason,
                                          submitted_at, concluded_at)
         VALUES ($1, $2, $3, $4::item_state, $5::reason, $6, $7)
             ON CONFLICT DO NOTHING",
    )
    .bind(item.to_string())
    .bind(event.org_id.to_string())
    .bind(event.queue_id.to_string())
    .bind(facts.conclusion.as_str())
    .bind(facts.reason.as_str())
    .bind(facts.submitted_at)
    .bind(facts.concluded_at)
    .execute(&mut *conn)
    .await?;
    for (pass, state) in (0_i32..).zip(&facts.passes) {
        sqlx::query(
            "INSERT INTO an_state_durations (org_id, queue_id, item_id, pass, state, entered_at,
                                             left_at, exit_reason)
             VALUES ($1, $2, $3, $4, $5::item_state, $6, $7, $8::reason)
             ON CONFLICT DO NOTHING",
        )
        .bind(event.org_id.to_string())
        .bind(event.queue_id.to_string())
        .bind(item.to_string())
        .bind(pass)
        .bind(state.state.as_str())
        .bind(state.entered_at)
        .bind(state.left_at)
        .bind(state.exit_reason.as_str())
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct CheckFact {
    name: String,
    kind: CheckKind,
    required: bool,
    conclusion: Option<CheckConclusion>,
}

/// The last report of each check, in the order of their names.
fn last_reports(required_statuses: &[String], reports: Vec<CheckReport>) -> Vec<CheckFact> {
    let last: BTreeMap<(String, CheckKind), Option<CheckConclusion>> = reports
        .into_iter()
        .map(|report| ((report.name, report.kind), report.conclusion))
        .collect();
    last.into_iter()
        .map(|((name, kind), conclusion)| CheckFact {
            required: required_statuses.contains(&name),
            name,
            kind,
            conclusion,
        })
        .collect()
}

/// What the events of a run say before its end.
struct RunStart {
    group_id: GroupId,
    attempt: u32,
    started_at: OffsetDateTime,
    /// The test commit, the required statuses and the sequence number of the
    /// assembly, once the test branch exists.
    assembly: Option<(String, Vec<String>, i64)>,
}

async fn run_start(
    conn: &mut PgConnection,
    run: RunId,
) -> Result<Option<RunStart>, AnalyticsError> {
    let rows: Vec<(i64, String, String, Value, OffsetDateTime)> = sqlx::query_as(
        "SELECT seq, id, type::text, payload, occurred_at FROM events
         WHERE run_id = $1 AND type IN ('run.transitioned', 'test_branch.assembled')
         ORDER BY seq",
    )
    .bind(run.to_string())
    .fetch_all(&mut *conn)
    .await?;
    let mut start = None;
    let mut assembly = None;
    for (seq, id, kind, payload, at) in rows {
        match read_event(&id, &kind, &payload)? {
            EventKind::RunTransitioned {
                from: None,
                group_id,
                attempt,
                ..
            } => start = Some((group_id, attempt, at)),
            EventKind::TestBranchAssembled {
                test_sha,
                required_statuses,
                ..
            } => assembly = Some((test_sha, required_statuses, seq)),
            _ => {}
        }
    }
    Ok(start.map(|(group_id, attempt, started_at)| RunStart {
        group_id,
        attempt,
        started_at,
        assembly,
    }))
}

/// The reports of the checks on `test_sha` from the assembly of a run to its
/// end: those the run counts.
async fn reports(
    conn: &mut PgConnection,
    event: &Event,
    test_sha: &str,
    seqs: (i64, i64),
) -> Result<Vec<CheckReport>, AnalyticsError> {
    let rows: Vec<(String, Value)> = sqlx::query_as(
        "SELECT id, payload FROM events
         WHERE queue_id = $1 AND seq > $2 AND seq < $3 AND type = 'check.observed'
           AND payload ->> 'test_sha' = $4
         ORDER BY seq",
    )
    .bind(event.queue_id.to_string())
    .bind(seqs.0)
    .bind(seqs.1)
    .bind(test_sha)
    .fetch_all(&mut *conn)
    .await?;
    let mut reports = Vec::with_capacity(rows.len());
    for (id, payload) in rows {
        if let EventKind::CheckObserved { check, .. } = read_event(&id, "check.observed", &payload)?
        {
            reports.push(check);
        }
    }
    Ok(reports)
}

async fn record_run(
    conn: &mut PgConnection,
    event: &Event,
    seq: i64,
    run: RunId,
) -> Result<(), AnalyticsError> {
    let EventKind::RunTransitioned { to, reason, .. } = &event.kind else {
        return Ok(());
    };
    let Some(start) = run_start(conn, run).await? else {
        return Ok(());
    };
    let checks = match &start.assembly {
        Some((test_sha, required_statuses, assembled)) => last_reports(
            required_statuses,
            reports(conn, event, test_sha, (*assembled, seq)).await?,
        ),
        None => Vec::new(),
    };

    write_run(conn, event, run, &start, (*to, *reason)).await?;
    for check in checks {
        sqlx::query(
            "INSERT INTO an_check_results (org_id, run_id, name, kind, required, conclusion)
             VALUES ($1, $2, $3, $4::check_kind, $5, $6::check_conclusion)
             ON CONFLICT DO NOTHING",
        )
        .bind(event.org_id.to_string())
        .bind(run.to_string())
        .bind(&check.name)
        .bind(check.kind.as_str())
        .bind(check.required)
        .bind(check.conclusion.map(CheckConclusion::as_str))
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

async fn write_run(
    conn: &mut PgConnection,
    event: &Event,
    run: RunId,
    start: &RunStart,
    (status, reason): (RunStatus, Reason),
) -> Result<(), AnalyticsError> {
    sqlx::query(
        "INSERT INTO an_test_run_results (run_id, org_id, queue_id, group_id, attempt, status,
                                          reason, started_at, ended_at)
         VALUES ($1, $2, $3, $4, $5, $6::run_status, $7::reason, $8, $9)
             ON CONFLICT DO NOTHING",
    )
    .bind(run.to_string())
    .bind(event.org_id.to_string())
    .bind(event.queue_id.to_string())
    .bind(start.group_id.to_string())
    .bind(i64::from(start.attempt))
    .bind(status.as_str())
    .bind(reason.as_str())
    .bind(start.started_at)
    .bind(event.occurred_at)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

fn read_event(id: &str, kind: &str, payload: &Value) -> Result<EventKind, AnalyticsError> {
    event_kind(kind, payload).map_err(|source| AnalyticsError::ReadEvent {
        id: id.to_owned(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::*;

    fn transition(
        at: OffsetDateTime,
        from: Option<ItemState>,
        to: ItemState,
        reason: Reason,
    ) -> Transition {
        Transition {
            at,
            from,
            to,
            reason,
        }
    }

    #[test]
    fn an_item_passes_through_each_state_until_its_end() {
        let submitted = datetime!(2026-10-14 09:00 UTC);
        let testing = datetime!(2026-10-14 09:05 UTC);
        let merged = datetime!(2026-10-14 09:20 UTC);
        let transitions = [
            transition(submitted, None, ItemState::Pending, Reason::Submitted),
            transition(
                testing,
                Some(ItemState::Pending),
                ItemState::Testing,
                Reason::TestRunStarted,
            ),
            transition(
                merged,
                Some(ItemState::Testing),
                ItemState::Merged,
                Reason::MergeGraphMerge,
            ),
        ];

        let facts = item_facts(&transitions).unwrap();

        assert_eq!(facts.conclusion, ItemState::Merged);
        assert_eq!(facts.reason, Reason::MergeGraphMerge);
        assert_eq!(
            (facts.submitted_at, facts.concluded_at),
            (submitted, merged)
        );
        assert_eq!(
            facts.passes,
            [
                StatePass {
                    state: ItemState::Pending,
                    entered_at: submitted,
                    left_at: testing,
                    exit_reason: Reason::TestRunStarted,
                },
                StatePass {
                    state: ItemState::Testing,
                    entered_at: testing,
                    left_at: merged,
                    exit_reason: Reason::MergeGraphMerge,
                },
            ]
        );
    }

    #[test]
    fn an_item_without_its_creation_or_its_end_has_no_facts() {
        let at = datetime!(2026-10-14 09:00 UTC);
        let ended = transition(
            at,
            Some(ItemState::Testing),
            ItemState::Failed,
            Reason::TestRunRequiredStatusFailed,
        );
        let created = transition(at, None, ItemState::Pending, Reason::Submitted);

        assert_eq!(item_facts(&[ended]), None);
        assert_eq!(item_facts(&[created]), None);
        assert_eq!(item_facts(&[]), None);
    }

    #[test]
    fn a_check_counts_its_last_report() {
        let report = |name: &str, conclusion| CheckReport {
            name: name.into(),
            kind: CheckKind::Check,
            conclusion,
            details_url: None,
        };
        let reports = vec![
            report("test", None),
            report("lint", Some(CheckConclusion::Success)),
            report("test", Some(CheckConclusion::Failure)),
        ];

        let checks = last_reports(&["test".into()], reports);

        assert_eq!(
            checks,
            [
                CheckFact {
                    name: "lint".into(),
                    kind: CheckKind::Check,
                    required: false,
                    conclusion: Some(CheckConclusion::Success),
                },
                CheckFact {
                    name: "test".into(),
                    kind: CheckKind::Check,
                    required: true,
                    conclusion: Some(CheckConclusion::Failure),
                },
            ]
        );
    }
}

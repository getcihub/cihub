use std::collections::{BTreeMap, HashMap, HashSet};

use cihub_domain::{
    CheckConclusion, CheckKind, CheckReport, Event, EventKind, GroupId, ItemId, ItemState, OrgId,
    QueueId, Reason, RunId,
};
use cihub_store::event_kind;
use serde_json::Value;
use sqlx::PgConnection;
use strum::IntoEnumIterator;
use time::OffsetDateTime;

use crate::AnalyticsError;
use crate::hourly::{Increments, LIVE_STATES, TIME_TO_MERGE};

/// The key of the cursors of the facts in `log_cursors`.
pub const SUBSCRIPTION: &str = "analytics";

/// Writes the facts that `events`, a page of the log of one queue in its
/// order, conclude: an ended item, an ended run, and adds the facts that it
/// writes to the hourly aggregate of the queue. The caller advances its
/// cursor in the same transaction, so that each fact is written once; a fact
/// already written, after a cursor read the log again, stays as it is and
/// counts once.
///
/// A page costs a fixed number of queries, whatever it concludes: a query
/// per fact was most of the time of a notifier that catches up.
///
/// # Errors
///
/// Postgres fails, or an event of the log does not match `cihub-domain`.
pub async fn record_facts(
    conn: &mut PgConnection,
    events: &[(i64, Event)],
) -> Result<(), AnalyticsError> {
    let Some((_, first)) = events.first() else {
        return Ok(());
    };

    let mut items = Vec::new();
    let mut runs = Vec::new();
    for (seq, event) in events {
        match &event.kind {
            EventKind::ItemTransitioned { item_id, to, .. } if to.is_terminal() => {
                items.push((*item_id, event));
            }
            EventKind::RunTransitioned { run_id, to, .. } if to.is_terminal() => {
                runs.push((*seq, *run_id, event));
            }
            _ => {}
        }
    }

    let mut hourly = Increments::default();
    record_items(conn, &items, &mut hourly).await?;
    record_runs(conn, &runs, &mut hourly).await?;

    hourly.write(conn, (first.org_id, first.queue_id)).await
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

impl ItemFacts {
    /// The seconds that the item spent in `state`, over its passes.
    fn seconds_in(&self, state: ItemState) -> i64 {
        self.passes
            .iter()
            .filter(|pass| pass.state == state)
            .map(|pass| (pass.left_at - pass.entered_at).whole_seconds())
            .sum()
    }
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

/// The transitions of `items` in the log, by item.
async fn transitions(
    conn: &mut PgConnection,
    items: &[String],
) -> Result<HashMap<String, Vec<Transition>>, AnalyticsError> {
    let rows: Vec<(String, String, Value, OffsetDateTime)> = sqlx::query_as(
        "SELECT item_id, id, payload, occurred_at FROM events
         WHERE item_id = ANY ($1) AND type = 'item.transitioned'
         ORDER BY seq",
    )
    .bind(items)
    .fetch_all(&mut *conn)
    .await?;

    let mut transitions: HashMap<String, Vec<Transition>> = HashMap::new();
    for (item, id, payload, at) in rows {
        if let EventKind::ItemTransitioned {
            from, to, reason, ..
        } = read_event(&id, "item.transitioned", &payload)?
        {
            transitions.entry(item).or_default().push(Transition {
                at,
                from,
                to,
                reason,
            });
        }
    }
    Ok(transitions)
}

/// The rows of `an_item_conclusions` and `an_state_durations`, a column each.
#[derive(Default)]
struct ItemRows {
    item_id: Vec<String>,
    conclusion: Vec<&'static str>,
    reason: Vec<&'static str>,
    submitted_at: Vec<OffsetDateTime>,
    concluded_at: Vec<OffsetDateTime>,
    pass_item_id: Vec<String>,
    pass: Vec<i32>,
    state: Vec<&'static str>,
    entered_at: Vec<OffsetDateTime>,
    left_at: Vec<OffsetDateTime>,
    exit_reason: Vec<&'static str>,
}

impl ItemRows {
    fn push(&mut self, item: String, facts: &ItemFacts) {
        for (pass, state) in (0_i32..).zip(&facts.passes) {
            self.pass_item_id.push(item.clone());
            self.pass.push(pass);
            self.state.push(state.state.as_str());
            self.entered_at.push(state.entered_at);
            self.left_at.push(state.left_at);
            self.exit_reason.push(state.exit_reason.as_str());
        }
        self.item_id.push(item);
        self.conclusion.push(facts.conclusion.as_str());
        self.reason.push(facts.reason.as_str());
        self.submitted_at.push(facts.submitted_at);
        self.concluded_at.push(facts.concluded_at);
    }
}

async fn record_items(
    conn: &mut PgConnection,
    items: &[(ItemId, &Event)],
    hourly: &mut Increments,
) -> Result<(), AnalyticsError> {
    let Some((_, first)) = items.first() else {
        return Ok(());
    };

    let ids: Vec<String> = items.iter().map(|(item, _)| item.to_string()).collect();
    let mut transitions = transitions(conn, &ids).await?;
    let mut rows = ItemRows::default();
    let mut ended = Vec::new();
    for id in ids {
        let facts = transitions.remove(&id).and_then(|found| item_facts(&found));
        if let Some(facts) = facts {
            rows.push(id.clone(), &facts);
            ended.push((id, facts));
        }
    }
    let written = write_items(conn, (first.org_id, first.queue_id), &rows).await?;

    for (_, facts) in ended.iter().filter(|(id, _)| written.contains(id)) {
        count_item(hourly, facts);
    }
    Ok(())
}

/// Counts an ended item in the hour of its end; a merged one adds its time to
/// merge and its time in each live state, 0 for a state that it skipped.
fn count_item(hourly: &mut Increments, facts: &ItemFacts) {
    hourly.add_conclusion(facts.concluded_at, facts.conclusion, facts.reason);
    if facts.conclusion != ItemState::Merged {
        return;
    }

    let time_to_merge = (facts.concluded_at - facts.submitted_at).whole_seconds();
    hourly.add_duration(facts.concluded_at, TIME_TO_MERGE, time_to_merge);
    for state in LIVE_STATES {
        hourly.add_duration(facts.concluded_at, state.as_str(), facts.seconds_in(state));
    }
}

/// Writes the rows, and returns the items that were not written yet.
async fn write_items(
    conn: &mut PgConnection,
    (org, queue): (OrgId, QueueId),
    rows: &ItemRows,
) -> Result<HashSet<String>, AnalyticsError> {
    let written: Vec<String> = sqlx::query_scalar(
        "INSERT INTO an_item_conclusions (item_id, org_id, queue_id, conclusion, reason,
                                          submitted_at, concluded_at)
         SELECT r.item_id, $1, $2, r.conclusion::item_state, r.reason::reason, r.submitted_at,
                r.concluded_at
         FROM unnest($3::text[], $4::text[], $5::text[], $6::timestamptz[], $7::timestamptz[])
              AS r (item_id, conclusion, reason, submitted_at, concluded_at)
         ON CONFLICT DO NOTHING
         RETURNING item_id",
    )
    .bind(org.to_string())
    .bind(queue.to_string())
    .bind(&rows.item_id)
    .bind(&rows.conclusion)
    .bind(&rows.reason)
    .bind(&rows.submitted_at)
    .bind(&rows.concluded_at)
    .fetch_all(&mut *conn)
    .await?;
    sqlx::query(
        "INSERT INTO an_state_durations (org_id, queue_id, item_id, pass, state, entered_at,
                                         left_at, exit_reason)
         SELECT $1, $2, r.item_id, r.pass, r.state::item_state, r.entered_at, r.left_at,
                r.exit_reason::reason
         FROM unnest($3::text[], $4::integer[], $5::text[], $6::timestamptz[],
                     $7::timestamptz[], $8::text[])
              AS r (item_id, pass, state, entered_at, left_at, exit_reason)
         ON CONFLICT DO NOTHING",
    )
    .bind(org.to_string())
    .bind(queue.to_string())
    .bind(&rows.pass_item_id)
    .bind(&rows.pass)
    .bind(&rows.state)
    .bind(&rows.entered_at)
    .bind(&rows.left_at)
    .bind(&rows.exit_reason)
    .execute(&mut *conn)
    .await?;
    Ok(written.into_iter().collect())
}

#[derive(Debug, PartialEq, Eq)]
struct CheckFact {
    name: String,
    kind: CheckKind,
    required: bool,
    conclusion: Option<CheckConclusion>,
    /// It passed, and failed on the attempt before of the same group.
    passed_on_retry: bool,
}

fn is_failed(conclusion: Option<CheckConclusion>) -> bool {
    conclusion.is_some_and(|conclusion| !conclusion.is_passing())
}

/// A check of an attempt of a group: the group, the attempt, the name and
/// the kind of the check.
type CheckKey = (String, i64, String, &'static str);

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
            passed_on_retry: false,
        })
        .collect()
}

/// The checks that failed in the runs of `groups` already written.
async fn failed_checks(
    conn: &mut PgConnection,
    groups: &[String],
) -> Result<HashSet<CheckKey>, AnalyticsError> {
    let failing: Vec<&str> = CheckConclusion::iter()
        .filter(|conclusion| !conclusion.is_passing())
        .map(CheckConclusion::as_str)
        .collect();
    let rows: Vec<(String, i64, String, String)> = sqlx::query_as(
        "SELECT r.group_id, r.attempt::bigint, k.name, k.kind::text
         FROM an_test_run_results r
         JOIN an_check_results k ON k.run_id = r.run_id
         WHERE r.group_id = ANY ($1) AND k.conclusion::text = ANY ($2)",
    )
    .bind(groups)
    .bind(&failing)
    .fetch_all(&mut *conn)
    .await?;

    let kinds: HashMap<String, CheckKind> = CheckKind::iter()
        .map(|kind| (kind.as_str().to_owned(), kind))
        .collect();
    Ok(rows
        .into_iter()
        .filter_map(|(group, attempt, name, kind)| {
            kinds
                .get(&kind)
                .map(|kind| (group, attempt, name, kind.as_str()))
        })
        .collect())
}

/// The checks that a run counts, each marked when it passed after it failed
/// on the attempt before; adds its failures to `failed`, for the attempt
/// after it in the same page.
fn checks_of(
    start: &RunStart,
    ended: i64,
    reports: &[(i64, String, CheckReport)],
    failed: &mut HashSet<CheckKey>,
) -> Vec<CheckFact> {
    let Some(assembly) = &start.assembly else {
        return Vec::new();
    };

    let group = start.group_id.to_string();
    let attempt = i64::from(start.attempt);
    let mut checks = last_reports(
        &assembly.required_statuses,
        reports_of(assembly, ended, reports),
    );
    for check in &mut checks {
        let before = (
            group.clone(),
            attempt - 1,
            check.name.clone(),
            check.kind.as_str(),
        );
        check.passed_on_retry =
            check.conclusion.is_some_and(CheckConclusion::is_passing) && failed.contains(&before);
    }
    failed.extend(
        checks
            .iter()
            .filter(|check| is_failed(check.conclusion))
            .map(|check| {
                (
                    group.clone(),
                    attempt,
                    check.name.clone(),
                    check.kind.as_str(),
                )
            }),
    );
    checks
}

/// The test commit, the required statuses and the sequence number of the
/// assembly of a run.
struct Assembly {
    test_sha: String,
    required_statuses: Vec<String>,
    seq: i64,
}

/// What the events of a run say before its end.
struct RunStart {
    group_id: GroupId,
    attempt: u32,
    started_at: OffsetDateTime,
    /// `None` until the test branch exists.
    assembly: Option<Assembly>,
}

/// The creation and the assembly of `runs` in the log, by run.
async fn run_starts(
    conn: &mut PgConnection,
    runs: &[String],
) -> Result<HashMap<String, RunStart>, AnalyticsError> {
    let rows: Vec<(String, i64, String, String, Value, OffsetDateTime)> = sqlx::query_as(
        "SELECT run_id, seq, id, type::text, payload, occurred_at FROM events
         WHERE run_id = ANY ($1) AND type IN ('run.transitioned', 'test_branch.assembled')
         ORDER BY seq",
    )
    .bind(runs)
    .fetch_all(&mut *conn)
    .await?;

    let mut created = HashMap::new();
    let mut assemblies = HashMap::new();
    for (run, seq, id, kind, payload, at) in rows {
        match read_event(&id, &kind, &payload)? {
            EventKind::RunTransitioned {
                from: None,
                group_id,
                attempt,
                ..
            } => {
                created.insert(run, (group_id, attempt, at));
            }
            EventKind::TestBranchAssembled {
                test_sha,
                required_statuses,
                ..
            } => {
                let assembly = Assembly {
                    test_sha,
                    required_statuses,
                    seq,
                };
                assemblies.insert(run, assembly);
            }
            _ => {}
        }
    }
    Ok(created
        .into_iter()
        .map(|(run, (group_id, attempt, started_at))| {
            let assembly = assemblies.remove(&run);
            let start = RunStart {
                group_id,
                attempt,
                started_at,
                assembly,
            };
            (run, start)
        })
        .collect())
}

/// The reports of the checks on the test commits of `starts`, between the
/// first assembly and `before`, with their sequence numbers.
async fn reports(
    conn: &mut PgConnection,
    queue: QueueId,
    starts: &HashMap<String, RunStart>,
    before: i64,
) -> Result<Vec<(i64, String, CheckReport)>, AnalyticsError> {
    let assemblies: Vec<&Assembly> = starts
        .values()
        .filter_map(|start| start.assembly.as_ref())
        .collect();
    let Some(after) = assemblies.iter().map(|assembly| assembly.seq).min() else {
        return Ok(Vec::new());
    };
    let shas: Vec<&str> = assemblies
        .iter()
        .map(|assembly| assembly.test_sha.as_str())
        .collect();

    let rows: Vec<(i64, String, Value)> = sqlx::query_as(
        "SELECT seq, id, payload FROM events
         WHERE queue_id = $1 AND seq > $2 AND seq < $3 AND type = 'check.observed'
           AND payload ->> 'test_sha' = ANY ($4)
         ORDER BY seq",
    )
    .bind(queue.to_string())
    .bind(after)
    .bind(before)
    .bind(&shas)
    .fetch_all(&mut *conn)
    .await?;

    let mut reports = Vec::with_capacity(rows.len());
    for (seq, id, payload) in rows {
        if let EventKind::CheckObserved { test_sha, check } =
            read_event(&id, "check.observed", &payload)?
        {
            reports.push((seq, test_sha, check));
        }
    }
    Ok(reports)
}

/// The reports that a run counts: on its test commit, from its assembly to
/// its end.
fn reports_of(
    assembly: &Assembly,
    ended: i64,
    reports: &[(i64, String, CheckReport)],
) -> Vec<CheckReport> {
    reports
        .iter()
        .filter(|(seq, test_sha, _)| {
            *seq > assembly.seq && *seq < ended && *test_sha == assembly.test_sha
        })
        .map(|(_, _, report)| report.clone())
        .collect()
}

/// The rows of `an_test_run_results` and `an_check_results`, a column each.
#[derive(Default)]
struct RunRows {
    run_id: Vec<String>,
    group_id: Vec<String>,
    attempt: Vec<i64>,
    status: Vec<&'static str>,
    reason: Vec<&'static str>,
    started_at: Vec<OffsetDateTime>,
    ended_at: Vec<OffsetDateTime>,
    check_run_id: Vec<String>,
    name: Vec<String>,
    kind: Vec<&'static str>,
    required: Vec<bool>,
    conclusion: Vec<Option<&'static str>>,
    passed_on_retry: Vec<bool>,
}

impl RunRows {
    fn push(&mut self, run: String, start: &RunStart, end: &Event, checks: &[CheckFact]) {
        let EventKind::RunTransitioned { to, reason, .. } = &end.kind else {
            return;
        };

        for check in checks {
            self.check_run_id.push(run.clone());
            self.name.push(check.name.clone());
            self.kind.push(check.kind.as_str());
            self.required.push(check.required);
            self.conclusion
                .push(check.conclusion.map(CheckConclusion::as_str));
            self.passed_on_retry.push(check.passed_on_retry);
        }
        self.run_id.push(run);
        self.group_id.push(start.group_id.to_string());
        self.attempt.push(i64::from(start.attempt));
        self.status.push(to.as_str());
        self.reason.push(reason.as_str());
        self.started_at.push(start.started_at);
        self.ended_at.push(end.occurred_at);
    }
}

async fn record_runs(
    conn: &mut PgConnection,
    runs: &[(i64, RunId, &Event)],
    hourly: &mut Increments,
) -> Result<(), AnalyticsError> {
    let (Some((_, _, first)), Some((last_seq, _, _))) = (runs.first(), runs.last()) else {
        return Ok(());
    };

    let ids: Vec<String> = runs.iter().map(|(_, run, _)| run.to_string()).collect();
    let starts = run_starts(conn, &ids).await?;
    let reports = reports(conn, first.queue_id, &starts, *last_seq).await?;
    let groups: Vec<String> = starts
        .values()
        .map(|start| start.group_id.to_string())
        .collect();
    let mut failed = failed_checks(conn, &groups).await?;
    let mut rows = RunRows::default();
    let mut ended = Vec::new();
    for ((seq, _, end), id) in runs.iter().zip(ids) {
        let Some(start) = starts.get(&id) else {
            continue;
        };
        let checks = checks_of(start, *seq, &reports, &mut failed);
        rows.push(id.clone(), start, end, &checks);
        ended.push((id, end.occurred_at, checks));
    }
    let written = write_runs(conn, (first.org_id, first.queue_id), &rows).await?;

    for (_, ended_at, checks) in ended.iter().filter(|(id, _, _)| written.contains(id)) {
        for check in checks {
            hourly.add_check(
                *ended_at,
                (&check.name, check.kind),
                check.required,
                (is_failed(check.conclusion), check.passed_on_retry),
            );
        }
    }
    Ok(())
}

/// Writes the rows, and returns the runs that were not written yet.
async fn write_runs(
    conn: &mut PgConnection,
    (org, queue): (OrgId, QueueId),
    rows: &RunRows,
) -> Result<HashSet<String>, AnalyticsError> {
    let written: Vec<String> = sqlx::query_scalar(
        "INSERT INTO an_test_run_results (run_id, org_id, queue_id, group_id, attempt, status,
                                          reason, started_at, ended_at)
         SELECT r.run_id, $1, $2, r.group_id, r.attempt, r.status::run_status, r.reason::reason,
                r.started_at, r.ended_at
         FROM unnest($3::text[], $4::text[], $5::bigint[], $6::text[], $7::text[],
                     $8::timestamptz[], $9::timestamptz[])
              AS r (run_id, group_id, attempt, status, reason, started_at, ended_at)
         ON CONFLICT DO NOTHING
         RETURNING run_id",
    )
    .bind(org.to_string())
    .bind(queue.to_string())
    .bind(&rows.run_id)
    .bind(&rows.group_id)
    .bind(&rows.attempt)
    .bind(&rows.status)
    .bind(&rows.reason)
    .bind(&rows.started_at)
    .bind(&rows.ended_at)
    .fetch_all(&mut *conn)
    .await?;
    sqlx::query(
        "INSERT INTO an_check_results (org_id, run_id, name, kind, required, conclusion,
                                       passed_on_retry)
         SELECT $1, r.run_id, r.name, r.kind::check_kind, r.required,
                r.conclusion::check_conclusion, r.passed_on_retry
         FROM unnest($2::text[], $3::text[], $4::text[], $5::boolean[], $6::text[],
                     $7::boolean[])
              AS r (run_id, name, kind, required, conclusion, passed_on_retry)
         ON CONFLICT DO NOTHING",
    )
    .bind(org.to_string())
    .bind(&rows.check_run_id)
    .bind(&rows.name)
    .bind(&rows.kind)
    .bind(&rows.required)
    .bind(&rows.conclusion)
    .bind(&rows.passed_on_retry)
    .execute(&mut *conn)
    .await?;
    Ok(written.into_iter().collect())
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
                    passed_on_retry: false,
                },
                CheckFact {
                    name: "test".into(),
                    kind: CheckKind::Check,
                    required: true,
                    conclusion: Some(CheckConclusion::Failure),
                    passed_on_retry: false,
                },
            ]
        );
    }

    #[test]
    fn a_check_that_passes_after_it_failed_passed_on_a_retry() {
        let group: GroupId = "group_00000000000000000000000005j0v0he".parse().unwrap();
        let at = datetime!(2026-10-14 09:00 UTC);
        let attempt = |attempt, seq, test_sha: &str| RunStart {
            group_id: group,
            attempt,
            started_at: at,
            assembly: Some(Assembly {
                test_sha: test_sha.into(),
                required_statuses: vec!["test".into()],
                seq,
            }),
        };
        let report = |test_sha: &str, conclusion| {
            let check = CheckReport {
                name: "test".into(),
                kind: CheckKind::Check,
                conclusion: Some(conclusion),
                details_url: None,
            };
            (test_sha.to_owned(), check)
        };
        let reports = [
            (2, report("a", CheckConclusion::Failure)),
            (5, report("b", CheckConclusion::Success)),
        ]
        .map(|(seq, (test_sha, check))| (seq, test_sha, check));
        let mut failed = HashSet::new();

        let first = checks_of(&attempt(1, 1, "a"), 3, &reports, &mut failed);
        let second = checks_of(&attempt(2, 4, "b"), 6, &reports, &mut failed);

        assert!(!first[0].passed_on_retry);
        assert!(is_failed(first[0].conclusion));
        assert!(second[0].passed_on_retry);
    }
}

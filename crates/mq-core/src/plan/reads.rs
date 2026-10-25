//! Read points of the PRs, the checks and the target branch, and the next wake-up.

use cihub_config::QueueSettings;
use cihub_domain::{Item, Mergeability, RunStatus, TestRun};
use time::{Duration, OffsetDateTime};

use crate::Snapshot;
use crate::snapshot::Request;

use super::Read;
use crate::plan::rules::formation::{Next, next_group};
use crate::plan::rules::readiness::not_ready_items;

/// Read points of a PR after its submission, in minutes: then every 15
/// minutes.
const READ_POINTS: [i64; 4] = [1, 3, 7, 15];

/// A run in progress without a report has its checks read this often.
pub(super) const CHECK_READ_EVERY: i64 = 10;

/// The head of the target branch is read this often while the queue has
/// work.
const TARGET_READ_EVERY: i64 = 5;

/// The last read point at or before `now`, and the next one after `now`.
/// The points depend only on the age of the item, not on the last read: a
/// read that changes nothing is not in the log. They count from the minute of
/// the submission, so that the reads of one minute go together. While the SCM
/// has not computed the mergeability, a point comes every minute.
fn read_points(item: &Item, now: OffsetDateTime) -> (Option<OffsetDateTime>, OffsetDateTime) {
    let minute = floor_to_minute(item.submitted_at);
    let at = |minutes: i64| minute + Duration::minutes(minutes);
    let age = (now - minute).whole_minutes().max(0);

    if item.mergeability == Mergeability::Unknown {
        return ((age >= 1).then(|| at(age)), at(age + 1));
    }

    let last = READ_POINTS
        .iter()
        .rev()
        .find(|&&point| point <= age)
        .map(|&point| point.max(age / 15 * 15));
    let next = READ_POINTS
        .iter()
        .find(|&&point| point > age)
        .copied()
        .unwrap_or((age / 15 + 1) * 15);

    (last.map(at), at(next))
}

/// The read of the PR of a live item, when its last point came after the
/// last observation.
pub(super) fn read_due(s: &Snapshot, item: &Item, now: OffsetDateTime) -> Option<Read> {
    let (point, _) = read_points(item, now);
    let point = point?;
    let observed = s.prs.get(&item.pr_number).map(|o| o.observed_at);

    (observed.is_none_or(|at| at < point)).then_some(Read {
        pr_number: item.pr_number,
        point,
    })
}

/// The live items whose PR CIHub reads.
pub(super) fn live_items(s: &Snapshot) -> impl Iterator<Item = &Item> {
    s.items
        .values()
        .filter(|i| i.is_current && !i.state.is_terminal())
}

/// The last point of a grid of `every` minutes from `base` at or before
/// `now`, without `base` itself, and the next point after `now`.
pub(super) fn grid(
    base: OffsetDateTime,
    every: i64,
    now: OffsetDateTime,
) -> (Option<OffsetDateTime>, OffsetDateTime) {
    let step = Duration::minutes(every);
    let steps = ((now - base).whole_minutes() / every).max(0);
    let at = |n: i64| base + step * i32::try_from(n).unwrap_or(i32::MAX);

    ((steps >= 1).then(|| at(steps)), at(steps + 1))
}

/// The last report on the test SHA of a run in progress, or its start.
pub(super) fn last_report(s: &Snapshot, run: &TestRun) -> OffsetDateTime {
    s.checks
        .get(&run.id)
        .into_iter()
        .flat_map(|checks| checks.values())
        .map(|o| o.observed_at)
        .fold(run.started_at, OffsetDateTime::max)
}

pub(super) fn runs_in_progress(s: &Snapshot) -> impl Iterator<Item = &TestRun> {
    s.runs
        .values()
        .filter(|r| r.status == RunStatus::InProgress)
}

/// The queue reads the head of its target while it has work: from the last
/// observation, or every minute while it does not know the head.
pub(super) fn target_points(
    s: &Snapshot,
    now: OffsetDateTime,
) -> Option<(Option<OffsetDateTime>, OffsetDateTime)> {
    let busy = s.items.values().any(|i| !i.state.is_terminal())
        || s.requests
            .values()
            .any(|r| matches!(r, Request::Submit { .. }));
    if !busy {
        return None;
    }

    if let Some(head) = &s.target.head {
        return Some(grid(head.observed_at, TARGET_READ_EVERY, now));
    }

    let minute = floor_to_minute(now);
    Some((Some(minute), minute + Duration::minutes(1)))
}

fn floor_to_minute(t: OffsetDateTime) -> OffsetDateTime {
    t - Duration::seconds(i64::from(t.second())) - Duration::nanoseconds(i64::from(t.nanosecond()))
}

/// The earliest of the next read points, the not-ready timeouts, the run
/// timeouts and the end of the wait of a batch.
pub(super) fn wake_at(
    s: &Snapshot,
    settings: &QueueSettings,
    now: OffsetDateTime,
) -> Option<OffsetDateTime> {
    let not_ready = Duration::hours(i64::from(settings.not_ready_timeout_hours.value));
    let testing = Duration::minutes(i64::from(settings.testing_timeout_minutes.value));
    let reads = live_items(s).map(|item| read_points(item, now).1);
    let timeouts = not_ready_items(s).map(|item| item.submitted_at + not_ready);
    let runs = runs_in_progress(s).flat_map(|r| {
        let (_, next_read) = grid(last_report(s, r), CHECK_READ_EVERY, now);
        [r.started_at + testing, next_read]
    });
    let target = target_points(s, now).map(|(_, next)| next);
    let batch = match next_group(s, &s.queue_order(), settings, now) {
        Some(Next::Wait { until, .. }) => Some(until),
        _ => None,
    };

    reads
        .chain(timeouts)
        .chain(runs)
        .chain(target)
        .chain(batch)
        .min()
}

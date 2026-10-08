//! The cost criterion of lot 12 (`DECISIONS.md` D212, D215, D223): on a fixed
//! load of the simulated world, `batched` uses at least 30 % fewer runs per
//! merged PR than `pipelined`, with a p50 time in queue at most 1.5 times as
//! long. The same load with closed PRs measures the useless assemblies (lot 7,
//! D212). `mise run load` runs both: they take a few minutes.

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

mod common;

use std::collections::{BTreeMap, BTreeSet};

use cihub_config::Setting;
use cihub_domain::{
    CheckConclusion, EventKind, ItemState, PrState, Priority, Reason, RunStatus, Strategy,
};
use common::{Rng, World, settings};
use time::{Duration, OffsetDateTime};

/// PRs per hour, minutes of a run, percent of broken PRs, percent of flaky
/// runs, hours of arrivals.
const PER_HOUR: f64 = 20.0;
const RUN_MINUTES: i64 = 20;
const BROKEN: u64 = 10;
const FLAKY: u64 = 2;
const HOURS: i64 = 8;
const SEEDS: [u64; 3] = [0x00C1_4B12, 0x0BA7_C4ED, 0x5EED_0012];
/// Percent of PRs closed while they wait, in the measure of the useless
/// assemblies; they close up to an hour after their submission.
const CLOSED: u64 = 5;

/// What a strategy cost on one seed.
#[derive(Default)]
struct Cost {
    runs: usize,
    merged: usize,
    /// Minutes from submission to merge, one per merged PR.
    times: Vec<i64>,
    /// Runs whose test branch was assembled.
    assembled: usize,
    /// Assembled runs stopped without a result because a PR that they tested
    /// was closed.
    useless: usize,
}

/// Plays the load of `seed` on a queue with `strategy`: arrivals with
/// exponential gaps, a CI that answers after `RUN_MINUTES`, fails a run that
/// tests a broken PR, and fails any run with the probability `FLAKY`. With
/// `closed`, that percent of the PRs closes while it waits.
fn play(strategy: Strategy, seed: u64, closed: u64) -> Cost {
    let mut w = World::new(settings(|l| l.strategy = Some(Setting::new(strategy))));
    let start = w.now;
    let mut rng = Rng(seed);
    let mut broken = BTreeSet::new();
    let mut arrivals = Vec::new();
    let mut at = start;
    loop {
        let gap = -rng.unit().ln() * 60.0 / PER_HOUR;
        #[expect(clippy::cast_possible_truncation, reason = "a gap of minutes")]
        let gap = Duration::seconds((gap * 60.0) as i64);
        at += gap;
        if at >= start + Duration::hours(HOURS) {
            break;
        }
        arrivals.push(at);
    }
    let mut flaky = Rng(seed ^ 0xF1A4);
    let mut close_rng = Rng(seed ^ 0xC105);
    let mut closing: Vec<(OffsetDateTime, u64)> = Vec::new();
    let mut next = 0;
    let mut pr = 0;
    let end = start + Duration::hours(HOURS * 40);
    while w.now < end {
        while next < arrivals.len() && arrivals[next] <= w.now {
            pr += 1;
            if rng.chance(BROKEN) {
                broken.insert(pr);
            }
            w.submit(pr, Priority::MEDIUM);
            if closed > 0 && close_rng.chance(closed) {
                let after = i64::try_from(1 + close_rng.next() % 60).unwrap();
                closing.push((w.now + Duration::minutes(after), pr));
            }
            next += 1;
        }
        let due: Vec<u64> = closing
            .iter()
            .filter(|(at, _)| *at <= w.now)
            .map(|(_, pr)| *pr)
            .collect();
        closing.retain(|(at, _)| *at > w.now);
        for pr in due {
            if w.state(pr).is_some_and(|s| !s.is_terminal()) {
                w.observe(pr, |p| p.state = PrState::Closed);
            }
        }
        w.assemble_all();
        let due: Vec<_> = w
            .snapshot
            .runs
            .values()
            .filter(|r| {
                r.status == RunStatus::InProgress
                    && r.started_at + Duration::minutes(RUN_MINUTES) <= w.now
            })
            .map(|r| r.id)
            .collect();
        for run_id in due {
            let Some(run) = w.snapshot.runs.get(&run_id) else {
                continue;
            };
            if run.status != RunStatus::InProgress {
                continue;
            }
            let group = &w.snapshot.groups[&run.group_id];
            let fails = group
                .predecessor_ids
                .iter()
                .chain(&group.item_ids)
                .filter_map(|id| w.snapshot.items.get(id))
                .any(|i| broken.contains(&i.pr_number))
                || flaky.chance(FLAKY);
            let conclusion = if fails {
                CheckConclusion::Failure
            } else {
                CheckConclusion::Success
            };
            w.report_run(run_id, conclusion);
        }
        w.merge_all();
        if next == arrivals.len()
            && closing.is_empty()
            && w.snapshot.items.values().all(|i| i.state.is_terminal())
        {
            break;
        }
        w.advance(1);
    }
    assert!(
        w.snapshot.items.values().all(|i| i.state.is_terminal()),
        "{strategy}: the queue did not drain"
    );
    cost(&w)
}

fn cost(w: &World) -> Cost {
    let mut submitted: BTreeMap<_, OffsetDateTime> = BTreeMap::new();
    let mut items = BTreeMap::new();
    let mut cost = Cost::default();
    for event in &w.log {
        match &event.kind {
            EventKind::ItemSubmitted { .. } => {
                submitted.insert(event.id, event.occurred_at);
            }
            EventKind::ItemTransitioned {
                item_id,
                from: None,
                ..
            } => {
                if let Some(at) = event.cause_id.and_then(|c| submitted.get(&c)) {
                    items.insert(*item_id, *at);
                }
            }
            EventKind::ItemTransitioned {
                item_id,
                to: ItemState::Merged,
                ..
            } => {
                cost.merged += 1;
                cost.times
                    .push((event.occurred_at - items[item_id]).whole_minutes());
            }
            EventKind::RunTransitioned { from: None, .. } => cost.runs += 1,
            EventKind::RunTransitioned {
                to: RunStatus::InProgress,
                ..
            } => cost.assembled += 1,
            EventKind::RunTransitioned {
                from: Some(RunStatus::InProgress),
                to: RunStatus::Interrupted | RunStatus::Cancelled,
                reason: Reason::PrerequisiteMergeItemCancelled | Reason::BatchMemberLeft,
                ..
            } => cost.useless += 1,
            _ => {}
        }
    }
    cost
}

fn total(strategy: Strategy) -> (f64, i64) {
    let mut runs = 0;
    let mut merged = 0;
    let mut times = Vec::new();
    for seed in SEEDS {
        let cost = play(strategy, seed, 0);
        runs += cost.runs;
        merged += cost.merged;
        times.extend(cost.times);
    }
    times.sort_unstable();
    #[expect(clippy::cast_precision_loss, reason = "small counts")]
    let per_merge = runs as f64 / merged as f64;
    (per_merge, times[times.len() / 2])
}

#[test]
#[ignore = "long: `mise run load`"]
fn batched_costs_fewer_runs_than_pipelined_for_a_close_time_in_queue() {
    let (pipelined_runs, pipelined_p50) = total(Strategy::Pipelined);
    let (batched_runs, batched_p50) = total(Strategy::Batched);
    assert!(
        batched_runs <= pipelined_runs * 0.7,
        "runs per merged PR: batched {batched_runs:.2}, pipelined {pipelined_runs:.2}"
    );
    #[expect(clippy::cast_precision_loss, reason = "minutes")]
    let limit = pipelined_p50 as f64 * 1.5;
    #[expect(clippy::cast_precision_loss, reason = "minutes")]
    let batched = batched_p50 as f64;
    assert!(
        batched <= limit,
        "p50 minutes: batched {batched_p50}, pipelined {pipelined_p50}; runs per merged PR: \
         batched {batched_runs:.2}, pipelined {pipelined_runs:.2}"
    );
}

/// The useless assemblies after a close (taken over from lot 7, `DECISIONS.md`
/// D212): the same load where 5 % of the PRs close while they wait. An
/// assembly is useless when its run stops without a result because a PR that
/// it tested closed; the run of the closed PR alone does not count. Lot 12
/// works on them only above 5 % of the assemblies.
#[test]
#[ignore = "long: `mise run load`"]
fn useless_assemblies_after_a_close_stay_under_5_percent() {
    for strategy in [Strategy::Pipelined, Strategy::Batched] {
        let (mut assembled, mut useless) = (0, 0);
        for seed in SEEDS {
            let cost = play(strategy, seed, CLOSED);
            assembled += cost.assembled;
            useless += cost.useless;
        }
        assert!(
            useless * 20 <= assembled,
            "{strategy}: {useless} useless assemblies out of {assembled}"
        );
    }
}

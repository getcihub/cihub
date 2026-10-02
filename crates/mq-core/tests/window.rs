//! The adaptive window (`AGENTS.md` D141): exit criterion of lot 8. A
//! `pipelined` queue under 20 % of failing PRs closes its window to the
//! floor, then opens it again once the PRs pass.

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

mod common;

use std::collections::BTreeSet;

use cihub_domain::{CheckConclusion, EventKind, ItemState, Priority, RunStatus};
use common::{World, settings};

/// A small generator with a fixed seed, so that the test gives the same
/// queue each time.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// `true` with the probability `percent` / 100.
    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }
}

/// The CI: a run fails when it tests a bad PR, as a member or as a
/// predecessor. Each round assembles, reports and merges, until no item is
/// live. Returns the window after each change.
fn drain(w: &mut World, bad: &BTreeSet<u64>) -> Vec<u32> {
    let start = w.log.len();
    for _ in 0..10_000 {
        w.assemble_all();
        let runs: Vec<_> = w
            .snapshot
            .runs
            .values()
            .filter(|r| r.status == RunStatus::InProgress)
            .map(|r| r.id)
            .collect();
        for run_id in &runs {
            // A report may end the runs above it.
            let Some(run) = w.snapshot.runs.get(run_id) else {
                continue;
            };
            if run.status != RunStatus::InProgress {
                continue;
            }
            let group = &w.snapshot.groups[&run.group_id];
            let tested = group.predecessor_ids.iter().chain(&group.item_ids);
            let fails = tested
                .filter_map(|id| w.snapshot.items.get(id))
                .any(|item| bad.contains(&item.pr_number));
            let conclusion = if fails {
                CheckConclusion::Failure
            } else {
                CheckConclusion::Success
            };
            w.report_run(*run_id, conclusion);
        }
        w.merge_all();
        if w.snapshot.items.values().all(|i| i.state.is_terminal()) {
            break;
        }
    }
    assert!(
        w.snapshot.items.values().all(|i| i.state.is_terminal()),
        "the queue did not drain"
    );
    w.log[start..]
        .iter()
        .filter_map(|e| match e.kind {
            EventKind::QueueWindowChanged { after, .. } => Some(after),
            _ => None,
        })
        .collect()
}

#[test]
fn the_window_closes_under_failures_then_opens_again() {
    let mut w = World::new(settings(|_| {}));
    let (floor, ceiling) = (
        w.settings.window_floor.value,
        w.settings.window_ceiling.value,
    );
    assert_eq!((floor, ceiling), (2, 20), "the defaults of `pipelined`");

    // First, 20 % of the PRs fail: the window comes down to the floor.
    let mut rng = Rng(0x00C1_4B00);
    let mut bad = BTreeSet::new();
    for pr in 1..=100 {
        if rng.chance(20) {
            bad.insert(pr);
        }
        w.submit(pr, Priority::MEDIUM);
    }
    let windows = drain(&mut w, &bad);
    assert!(windows.contains(&floor), "{windows:?}");
    assert!(windows.iter().all(|n| (floor..=ceiling).contains(n)));
    // Concluded items leave the snapshot: the log tells how they ended.
    let ended = |to: ItemState| {
        w.log
            .iter()
            .filter(|e| matches!(e.kind, EventKind::ItemTransitioned { to: t, .. } if t == to))
            .count()
    };
    assert_eq!(ended(ItemState::Failed), bad.len());
    assert_eq!(ended(ItemState::Merged), 100 - bad.len());

    // Then every PR passes: the window opens up to the ceiling.
    let before = w.snapshot.window.unwrap();
    for pr in 101..=140 {
        w.submit(pr, Priority::MEDIUM);
    }
    let windows = drain(&mut w, &BTreeSet::new());
    assert!(before < ceiling);
    assert_eq!(windows.last(), Some(&ceiling), "{windows:?}");
    assert_eq!(w.replay(), w.snapshot);
}

#[test]
fn the_window_stays_within_new_bounds() {
    let mut w = World::new(settings(|_| {}));
    for pr in 1..=5 {
        w.submit(pr, Priority::MEDIUM);
    }
    drain(&mut w, &BTreeSet::new());
    assert_eq!(w.snapshot.window, Some(7));
    // A file with a lower ceiling brings the window down at once.
    w.settings = settings(|l| {
        l.window_ceiling = Some(cihub_config::Setting::new(4));
    });
    w.reconcile();
    assert_eq!(w.snapshot.window, Some(4));
    assert_eq!(w.replay(), w.snapshot);
}

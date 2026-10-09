//! The adaptive window (`DECISIONS.md` D141): exit criterion of lot 8. A
//! `pipelined` queue under 20 % of failing PRs closes its window to the
//! floor, then opens it again once the PRs pass.

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

mod common;

use std::collections::BTreeSet;

use cihub_domain::{EventKind, ItemState, Priority};
use common::{Rng, World, settings};

/// Drains the queue with the CI of the world; returns the window after each
/// change.
fn drain(w: &mut World, bad: &BTreeSet<u64>) -> Vec<u32> {
    let start = w.log.len();
    w.drain(bad);
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

    let ended = |to: ItemState| {
        w.log
            .iter()
            .filter(|e| matches!(e.kind, EventKind::ItemTransitioned { to: t, .. } if t == to))
            .count()
    };
    assert_eq!(ended(ItemState::Failed), bad.len());
    assert_eq!(ended(ItemState::Merged), 100 - bad.len());

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

    w.settings = settings(|l| {
        l.window_ceiling = Some(cihub_config::Setting::new(4));
    });
    w.reconcile();

    assert_eq!(w.snapshot.window, Some(4));
    assert_eq!(w.replay(), w.snapshot);
}

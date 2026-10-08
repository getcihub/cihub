//! `pending_failure_depth`: a group in failure also waits for the runs of the
//! groups behind it that test its code.

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

mod common;

use cihub_config::Setting;
use cihub_domain::{CheckConclusion, ItemState, Priority};
use common::{World, settings};

fn world(depth: u32) -> World {
    World::new(settings(|l| {
        l.window_floor = Some(Setting::new(3));
        l.window_ceiling = Some(Setting::new(3));
        l.failure_budget_retries = Some(Setting::new(0));
        l.pending_failure_depth = Some(Setting::new(depth));
    }))
}

/// Submits PRs 1 to 3, assembles their runs, then fails the run of PR 1.
fn first_fails(w: &mut World) {
    for pr in 1..=3 {
        w.submit(pr, Priority::MEDIUM);
    }
    w.assemble_all();
    w.report(1, CheckConclusion::Failure);
}

#[test]
fn at_depth_0_the_culprit_fails_at_once() {
    let mut w = world(0);
    first_fails(&mut w);
    assert_eq!(w.ended().get(&1), Some(&ItemState::Failed));
    assert_eq!(w.replay(), w.snapshot);
}

#[test]
fn at_depth_1_the_culprit_waits_for_the_run_of_its_first_successor() {
    let mut w = world(1);
    first_fails(&mut w);
    assert_eq!(w.state(1), Some(ItemState::PendingFailure));
    w.report(3, CheckConclusion::Failure);
    assert_eq!(w.state(1), Some(ItemState::PendingFailure));
    w.report(2, CheckConclusion::Failure);
    assert_eq!(w.ended().get(&1), Some(&ItemState::Failed));
    assert_eq!(w.state(2), Some(ItemState::Testing));
    assert_eq!(w.replay(), w.snapshot);
}

#[test]
fn without_a_successor_the_culprit_does_not_wait() {
    let mut w = world(2);
    w.submit(1, Priority::MEDIUM);
    w.assemble_all();
    w.report(1, CheckConclusion::Failure);
    assert_eq!(w.ended().get(&1), Some(&ItemState::Failed));
    assert_eq!(w.replay(), w.snapshot);
}

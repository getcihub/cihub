//! Optimistic merge: a run that passes proves the groups that it tested as
//! predecessors; an evicted group still restarts the groups behind it.

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

mod common;

use cihub_config::Setting;
use cihub_domain::{CheckConclusion, ItemState, Priority, Reason, RunStatus};
use common::{World, settings, transitioned};

fn world(optimistic: bool) -> World {
    World::new(settings(|l| {
        l.window_floor = Some(Setting::new(3));
        l.window_ceiling = Some(Setting::new(3));
        l.failure_budget_retries = Some(Setting::new(0));
        l.pending_failure_depth = Some(Setting::new(1));
        l.optimistic_merging = Some(Setting::new(optimistic));
    }))
}

fn two_runs(w: &mut World) {
    w.submit(1, Priority::MEDIUM);
    w.submit(2, Priority::MEDIUM);
    w.assemble_all();
}

#[test]
fn a_run_that_passes_proves_the_group_ahead_and_stops_its_run() {
    let mut w = world(true);
    two_runs(&mut w);
    let first = w.run(1).unwrap();
    let second = w.run(2).unwrap();

    w.report(2, CheckConclusion::Success);

    let item = w.item(1).unwrap();
    assert_eq!(item.state, ItemState::TestsPassed);
    assert_eq!(item.state_reason, Reason::PassedByOptimization);
    assert_eq!(item.verified_by_run_id, Some(second));
    assert_eq!(w.snapshot.runs[&first].status, RunStatus::Interrupted);

    assert_eq!(w.merge_all(), 1);
    assert_eq!(w.merge_all(), 1);
    assert_eq!(w.ended().get(&2), Some(&ItemState::Merged));
    assert_eq!(w.replay(), w.snapshot);
}

#[test]
fn a_run_that_passes_releases_the_group_in_failure_ahead() {
    let mut w = world(true);
    two_runs(&mut w);

    w.report(1, CheckConclusion::Failure);
    assert_eq!(w.state(1), Some(ItemState::PendingFailure));

    w.report(2, CheckConclusion::Success);
    assert!(transitioned(
        &w,
        ItemState::TestsPassed,
        Reason::SuccessorTestRunPassed
    ));

    assert_eq!(w.merge_all(), 1);
    assert_eq!(w.merge_all(), 1);
    assert_eq!(w.ended().get(&1), Some(&ItemState::Merged));
    assert_eq!(w.replay(), w.snapshot);
}

#[test]
fn a_culprit_is_evicted_and_the_groups_behind_restart_without_it() {
    let mut w = world(true);
    for pr in 1..=3 {
        w.submit(pr, Priority::MEDIUM);
    }

    w.drain(&[1].into());

    let ended = w.ended();
    assert_eq!(ended.get(&1), Some(&ItemState::Failed));
    assert_eq!(ended.get(&2), Some(&ItemState::Merged));
    assert_eq!(ended.get(&3), Some(&ItemState::Merged));
    assert!(transitioned(
        &w,
        ItemState::Pending,
        Reason::PrerequisiteMergeItemFailed
    ));
    assert_eq!(w.replay(), w.snapshot);
}

#[test]
fn without_optimistic_merge_a_run_proves_only_its_group() {
    let mut w = world(false);
    two_runs(&mut w);

    w.report(2, CheckConclusion::Success);
    assert_eq!(w.state(1), Some(ItemState::Testing));
    assert_eq!(w.replay(), w.snapshot);
}

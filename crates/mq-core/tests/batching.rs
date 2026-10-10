//! Batches: their formation, their wait and their merge.

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

mod common;

use cihub_config::{Layer, Setting};
use cihub_domain::{
    Actor, CheckConclusion, EventKind, GroupKind, ItemId, ItemState, JobError, OptimizationMode,
    PrState, Priority, Reason, RunStatus, UserId,
};
use common::{World, head, settings, transitioned};
use time::Duration;
use ulid::Ulid;

/// Batches of `size` that wait 5 minutes, in a window of `window`.
fn batching(size: u32, window: u32) -> impl FnOnce(&mut Layer) {
    move |l| {
        l.window_floor = Some(Setting::new(window));
        l.window_ceiling = Some(Setting::new(window));
        l.batching_enabled = Some(Setting::new(true));
        l.batching_size = Some(Setting::new(size));
        l.batching_max_wait_minutes = Some(Setting::new(5));
    }
}

fn world(size: u32, window: u32) -> World {
    World::new(settings(batching(size, window)))
}

/// The kind and the PRs of each group, in the order of the queue.
fn groups(w: &World) -> Vec<(GroupKind, Vec<u64>)> {
    let mut groups: Vec<_> = w.snapshot.groups.values().collect();
    groups.sort_by_key(|g| w.snapshot.items[&g.item_ids[0]].position);
    groups
        .into_iter()
        .map(|g| {
            let prs = g
                .item_ids
                .iter()
                .map(|id| w.snapshot.items[id].pr_number)
                .collect();
            (g.kind, prs)
        })
        .collect()
}

fn assert_replays(w: &World) {
    assert_eq!(w.replay(), w.snapshot);
}

#[test]
fn a_full_batch_forms_at_once_and_tests_its_members_in_one_run() {
    let mut w = world(3, 2);

    for pr in 1..=3 {
        w.submit(pr, Priority::MEDIUM);
    }

    assert_eq!(groups(&w), vec![(GroupKind::Batch, vec![1, 2, 3])]);
    for pr in 1..=3 {
        assert_eq!(w.state(pr), Some(ItemState::Testing));
    }

    let branches = w.reconcile().desired.test_branches;
    assert_eq!(branches.len(), 1);
    let members: Vec<_> = branches[0].members.iter().map(|h| h.sha.clone()).collect();
    assert_eq!(members, vec![head(1, 0), head(2, 0), head(3, 0)]);
    assert_replays(&w);
}

#[test]
fn a_batch_that_is_not_full_waits_for_its_oldest_member() {
    let mut w = world(4, 2);
    w.submit(1, Priority::MEDIUM);
    let admitted = w.item(1).unwrap().admitted_at.unwrap();

    w.advance(2);
    w.submit(2, Priority::MEDIUM);
    assert!(w.snapshot.groups.is_empty());

    let wake_at = w.reconcile().wake_at.unwrap();
    assert!(wake_at <= admitted + Duration::minutes(5));

    w.now = admitted + Duration::minutes(5);
    w.reconcile();
    assert_eq!(groups(&w), vec![(GroupKind::Batch, vec![1, 2])]);
    assert_replays(&w);
}

#[test]
fn a_lone_item_forms_a_single_group_after_the_wait() {
    let mut w = world(4, 2);
    w.submit(1, Priority::MEDIUM);
    assert!(w.snapshot.groups.is_empty());

    w.advance(5);

    assert_eq!(groups(&w), vec![(GroupKind::Single, vec![1])]);
    assert_replays(&w);
}

#[test]
fn a_no_batch_item_is_tested_alone_and_cuts_the_series() {
    let mut w = world(4, 4);
    w.submit_no_batch(1);
    assert_eq!(groups(&w), vec![(GroupKind::Single, vec![1])]);

    w.submit(2, Priority::MEDIUM);
    w.submit_no_batch(3);
    assert_eq!(groups(&w).len(), 3);

    w.submit(4, Priority::MEDIUM);
    assert_eq!(groups(&w).len(), 3);

    w.advance(5);
    assert_eq!(
        groups(&w),
        vec![
            (GroupKind::Single, vec![1]),
            (GroupKind::Single, vec![2]),
            (GroupKind::Single, vec![3]),
            (GroupKind::Single, vec![4]),
        ]
    );

    assert_replays(&w);
}

#[test]
fn a_batch_takes_one_slot_of_the_window() {
    let mut w = world(2, 1);
    for pr in 1..=4 {
        w.submit(pr, Priority::MEDIUM);
    }

    assert_eq!(groups(&w), vec![(GroupKind::Batch, vec![1, 2])]);
    assert_eq!(w.state(3), Some(ItemState::Pending));
    assert_replays(&w);
}

#[test]
fn the_members_of_a_batch_that_passes_merge_in_order() {
    let mut w = world(3, 2);
    for pr in 1..=3 {
        w.submit(pr, Priority::MEDIUM);
    }

    w.assemble_all();
    w.report(1, CheckConclusion::Success);

    for pr in 1..=3 {
        assert_eq!(w.state(pr), Some(ItemState::TestsPassed));
    }

    let mut merged = Vec::new();
    while w.reconcile().desired.merges.len() == 1 {
        let next = w.reconcile().desired.merges[0].pr_number;
        assert_eq!(w.merge_all(), 1);
        merged.push(next);
    }

    assert_eq!(merged, vec![1, 2, 3]);
    assert!(transitioned(&w, ItemState::Merged, Reason::MergeGraphMerge));

    assert_replays(&w);
}

#[test]
fn the_other_members_restart_when_a_member_leaves_the_batch() {
    let mut w = world(3, 2);
    for pr in 1..=3 {
        w.submit(pr, Priority::MEDIUM);
    }

    w.observe(3, |p| p.state = PrState::Closed);
    assert!(transitioned(
        &w,
        ItemState::Pending,
        Reason::BatchMemberLeft
    ));

    w.advance(5);
    assert_eq!(groups(&w).last(), Some(&(GroupKind::Batch, vec![1, 2])));
    assert_replays(&w);
}

#[test]
fn a_push_on_the_target_restarts_the_members_left_after_a_partial_merge() {
    let mut w = world(2, 2);
    w.submit(1, Priority::MEDIUM);
    w.submit(2, Priority::MEDIUM);
    w.assemble_all();

    w.report(1, CheckConclusion::Success);
    assert_eq!(w.merge_all(), 1);

    w.push_target(Actor::User {
        user_id: UserId::from_ulid(Ulid(1)),
    });
    assert!(transitioned(
        &w,
        ItemState::Pending,
        Reason::ManualPushToTargetBranch
    ));
    assert_replays(&w);
}

/// The groups formed after `start` in the log: kind and PRs, in order.
fn formed_since(w: &World, start: usize) -> Vec<(GroupKind, Vec<u64>)> {
    let pr_of = |id: &ItemId| {
        w.log
            .iter()
            .find_map(|e| match &e.kind {
                EventKind::ItemTransitioned {
                    item_id,
                    from: None,
                    ..
                } if item_id == id => e.cause_id,
                _ => None,
            })
            .and_then(|cause| {
                w.log.iter().find_map(|e| match e.kind {
                    EventKind::ItemSubmitted { pr_number, .. } if e.id == cause => Some(pr_number),
                    _ => None,
                })
            })
            .unwrap()
    };

    w.log[start..]
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::GroupFormed { kind, item_ids, .. } => {
                Some((*kind, item_ids.iter().map(pr_of).collect()))
            }
            _ => None,
        })
        .collect()
}

fn bisecting(size: u32) -> World {
    World::new(settings(|l| {
        batching(size, 4)(l);
        l.failure_budget_retries = Some(Setting::new(0));
    }))
}

fn submit_all(w: &mut World, prs: std::ops::RangeInclusive<u64>) {
    for pr in prs {
        w.submit(pr, Priority::MEDIUM);
    }
}

fn ended(w: &World, state: ItemState) -> Vec<u64> {
    w.ended()
        .into_iter()
        .filter_map(|(pr, s)| (s == state).then_some(pr))
        .collect()
}

#[test]
fn a_bisection_isolates_the_last_pr_and_tests_it_alone() {
    use GroupKind::{Batch, Bisection};
    let mut w = bisecting(8);
    submit_all(&mut w, 1..=8);
    w.drain(&[8].into());

    assert_eq!(
        formed_since(&w, 0),
        vec![
            (Batch, (1..=8).collect()),
            (Bisection, vec![1, 2, 3, 4]),
            (Bisection, vec![5, 6, 7, 8]),
            (Bisection, vec![5, 6]),
            (Bisection, vec![7, 8]),
            (Bisection, vec![7]),
            (Bisection, vec![8]),
        ]
    );
    assert_eq!(ended(&w, ItemState::Merged), (1..=7).collect::<Vec<_>>());
    assert_eq!(ended(&w, ItemState::Failed), vec![8]);
    assert_replays(&w);
}

#[test]
fn a_bisection_isolates_the_first_pr_and_retests_the_other_half() {
    let mut w = bisecting(4);
    submit_all(&mut w, 1..=4);
    w.drain(&[1].into());

    assert_eq!(ended(&w, ItemState::Merged), vec![2, 3, 4]);
    assert_eq!(ended(&w, ItemState::Failed), vec![1]);

    let formed = formed_since(&w, 0);
    assert_eq!(formed[1], (GroupKind::Bisection, vec![1, 2]));
    assert!(formed.contains(&(GroupKind::Bisection, vec![1])));
    assert!(transitioned(
        &w,
        ItemState::Pending,
        Reason::BisectionRequired
    ));
    assert_replays(&w);
}

#[test]
fn a_flaky_failure_of_a_batch_evicts_no_pr() {
    let mut w = bisecting(4);
    submit_all(&mut w, 1..=4);
    w.drain_with(|run, _| run == 1);

    assert_eq!(ended(&w, ItemState::Merged), vec![1, 2, 3, 4]);
    assert_eq!(ended(&w, ItemState::Failed), []);
    assert_replays(&w);
}

#[test]
fn without_the_skip_the_other_half_goes_back_to_the_queue() {
    let mut w = World::new(settings(|l| {
        batching(4, 4)(l);
        l.bisection_concurrency = Some(Setting::new(1));
        l.failure_budget_retries = Some(Setting::new(0));
        l.optimization_mode = Some(Setting::new(OptimizationMode::Off));
    }));

    submit_all(&mut w, 1..=4);
    w.drain(&[4].into());

    assert_eq!(ended(&w, ItemState::Merged), vec![1, 2, 3]);
    assert_eq!(ended(&w, ItemState::Failed), vec![4]);
    assert!(transitioned(
        &w,
        ItemState::Pending,
        Reason::BisectionTestRunPassed
    ));
    assert!(formed_since(&w, 0).contains(&(GroupKind::Batch, vec![3, 4])));
    assert_replays(&w);
}

#[test]
fn a_full_bisection_pool_waits() {
    let mut w = World::new(settings(|l| {
        batching(2, 4)(l);
        l.bisection_concurrency = Some(Setting::new(1));
    }));
    submit_all(&mut w, 1..=4);

    w.assemble_all();
    let runs: Vec<_> = w.snapshot.runs.keys().copied().collect();
    for run in runs {
        w.report_run(run, CheckConclusion::Failure);
    }

    let bisections = w
        .snapshot
        .groups
        .values()
        .filter(|g| g.kind == GroupKind::Bisection)
        .count();
    assert_eq!(bisections, 1);
    assert_replays(&w);
}

#[test]
fn a_conflict_in_a_batch_is_bisected_and_evicts_only_its_pr() {
    let mut w = bisecting(4);
    submit_all(&mut w, 1..=4);

    for _ in 0..1_000 {
        let assembling: Vec<_> = w
            .snapshot
            .runs
            .values()
            .filter(|r| r.status == RunStatus::Assembling)
            .map(|r| (r.id, r.group_id))
            .collect();

        for (run_id, group_id) in assembling {
            let group = &w.snapshot.groups[&group_id];
            let conflicts = group
                .predecessor_ids
                .iter()
                .chain(&group.item_ids)
                .any(|id| w.snapshot.items.get(id).is_some_and(|i| i.pr_number == 2));

            if conflicts {
                w.fail_assembly(run_id, JobError::MergeConflict);
            } else {
                w.assemble(run_id);
            }
        }

        let running: Vec<_> = w
            .snapshot
            .runs
            .values()
            .filter(|r| r.status == RunStatus::InProgress)
            .map(|r| r.id)
            .collect();

        for run_id in &running {
            w.report_run(*run_id, CheckConclusion::Success);
        }

        w.merge_all();
        if w.snapshot.items.values().all(|i| i.state.is_terminal()) {
            break;
        }

        w.advance(1);
    }

    assert_eq!(ended(&w, ItemState::Merged), vec![1, 3, 4]);
    assert_eq!(ended(&w, ItemState::Failed), vec![2]);
    assert!(transitioned(
        &w,
        ItemState::PendingFailure,
        Reason::StartTestRunMergeConflict
    ));
    assert_replays(&w);
}

/// The exit criterion of lot 12 on the bisection: for each size of batch and
/// each place of the planted failure, the bisection isolates the failing PR in
/// at most log2(size) rounds of runs, rounded up, evicts it, and merges the
/// others.
#[test]
fn a_planted_failure_is_isolated_in_log2_rounds() {
    for size in 2..=8_u64 {
        let rounds = u64::from((size - 1).bit_width());

        for culprit in 1..=size {
            let mut w = bisecting(u32::try_from(size).unwrap());
            submit_all(&mut w, 1..=size);
            w.drain(&[culprit].into());

            let chain: Vec<Vec<u64>> = formed_since(&w, 0)
                .into_iter()
                .filter(|(kind, prs)| *kind == GroupKind::Bisection && prs.contains(&culprit))
                .map(|(_, prs)| prs)
                .collect();

            assert_eq!(chain.last(), Some(&vec![culprit]), "{size} PRs, {culprit}");
            assert!(
                u64::try_from(chain.len()).unwrap() <= rounds,
                "{size} PRs, {culprit}: {chain:?}"
            );
            assert_eq!(ended(&w, ItemState::Failed), vec![culprit]);
            assert_eq!(
                ended(&w, ItemState::Merged),
                (1..=size).filter(|pr| *pr != culprit).collect::<Vec<_>>()
            );
            assert_replays(&w);
        }
    }
}

/// The scenario of a manual test on a real repository: a `no_batch` PR behind
/// a PR that waits for its batch starts at once, with it; the PRs behind
/// still wait for their batch.
#[test]
fn a_no_batch_pr_does_not_wait_for_the_batch_ahead_of_it() {
    let mut w = world(4, 4);
    w.submit(1, Priority::MEDIUM);
    assert!(w.snapshot.groups.is_empty());

    w.submit_no_batch(2);
    assert_eq!(
        groups(&w),
        vec![(GroupKind::Single, vec![1]), (GroupKind::Single, vec![2])]
    );

    w.submit(3, Priority::MEDIUM);
    w.submit(4, Priority::MEDIUM);
    assert_eq!(groups(&w).len(), 2);

    w.advance(5);
    assert_eq!(groups(&w)[2], (GroupKind::Batch, vec![3, 4]));
    assert_replays(&w);
}

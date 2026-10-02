//! Scenarios of the queue, each one replayed from its log at the end.

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

mod common;

use cihub_config::Setting;
use cihub_domain::{
    Actor, BlockReason, CheckConclusion, CheckKind, CheckReport, EventKind, ItemState, JobError,
    Mergeability, PrState, Priority, QueueControl, QueueMode, QueueState, Reason, RunStatus,
};
use cihub_mq_core::{CheckRead, Head, Outcome, Read};
use common::{World, head, settings, window};

fn world() -> World {
    World::new(settings(window(2)))
}

fn reason(w: &World, pr: u64) -> Reason {
    w.item(pr).unwrap().state_reason
}

/// A decision of the log has the reason `want`.
fn decided_with(w: &World, want: Reason) -> bool {
    w.log
        .iter()
        .any(|e| matches!(e.kind, EventKind::ItemTransitioned { reason, .. } if reason == want))
}

fn failed_with(w: &World, want: Reason) -> bool {
    w.log.iter().any(|e| {
        matches!(e.kind, EventKind::ItemTransitioned { to: ItemState::Failed, reason, .. } if reason == want)
    })
}

fn assert_replays(w: &World) {
    assert_eq!(w.replay(), w.snapshot);
}

fn direct() -> World {
    World::new(settings(|l| {
        l.window_floor = Some(Setting::new(2));
        l.window_ceiling = Some(Setting::new(2));
        l.direct_merge_mode = Some(Setting::new(cihub_domain::DirectMergeMode::Always));
    }))
}

#[test]
fn a_direct_merge_lands_without_a_test_branch() {
    let mut w = direct();
    w.submit(1, Priority::MEDIUM);
    w.submit(2, Priority::MEDIUM);
    // The first item waits for its direct merge; no group forms meanwhile
    // (`AGENTS.md` D153).
    assert_eq!(w.state(1), Some(ItemState::Pending));
    assert!(w.snapshot.groups.is_empty());
    let merges = w.reconcile().desired.merges;
    assert_eq!(merges.len(), 1);
    assert_eq!((merges[0].pr_number, merges[0].run_id), (1, None));
    assert_eq!(merges[0].tested_sha, head(1, 0));
    assert_eq!(w.merge_all(), 1);
    assert!(decided_with(&w, Reason::MergeGraphDirectMerge));
    // The next item is first now: its direct merge cannot happen, so it goes
    // through the tests.
    let item = w.item(2).unwrap().id;
    w.fact(
        Actor::Bot,
        EventKind::MergeFailed {
            item_id: item,
            direct: true,
            error: JobError::DirectMergeUnavailable,
        },
    );
    assert_eq!(w.state(2), Some(ItemState::Testing));
    assert!(w.reconcile().desired.merges.is_empty());
    assert_replays(&w);
}

#[test]
fn no_direct_merge_by_default_or_with_a_live_group() {
    let mut w = world();
    w.submit(1, Priority::MEDIUM);
    assert_eq!(w.state(1), Some(ItemState::Testing), "off by default");
    let mut w = direct();
    w.submit(1, Priority::MEDIUM);
    let item = w.item(1).unwrap().id;
    w.fact(
        Actor::Bot,
        EventKind::MergeFailed {
            item_id: item,
            direct: true,
            error: JobError::DirectMergeUnavailable,
        },
    );
    assert_eq!(w.state(1), Some(ItemState::Testing));
    w.submit(2, Priority::MEDIUM);
    assert!(
        w.reconcile().desired.merges.is_empty(),
        "a live group: no direct merge"
    );
    assert_eq!(w.state(2), Some(ItemState::Testing));
}

#[test]
fn a_new_head_submitted_again_goes_to_the_end_of_the_queue() {
    let mut w = world();
    w.submit(1, Priority::MEDIUM);
    w.submit(2, Priority::MEDIUM);
    let old = w.item(1).unwrap().id;
    // The job that reads the PR sees the push: it submits the new head, then
    // gives the observation (`AGENTS.md` D152).
    w.fact(
        Actor::System,
        EventKind::ItemSubmitted {
            pr_number: 1,
            head_sha: head(1, 1),
            priority: Priority::MEDIUM,
            no_batch: false,
            direct: false,
            submitted_via: cihub_domain::SubmittedVia::Comment,
            author_id: cihub_domain::UserId::from_ulid(ulid::Ulid(1)),
            additions: 1,
            deletions: 0,
            changed_files: 1,
            sender_permission: None,
        },
    );
    assert_eq!(
        w.snapshot.current_item(1).map(|i| i.id),
        Some(old),
        "it waits"
    );
    w.observe(1, |pr| pr.head_sha = head(1, 1));
    assert!(decided_with(&w, Reason::PrPushedTo));
    let new = w.snapshot.current_item(1).unwrap();
    assert_ne!(new.id, old);
    assert_eq!(new.head_sha, head(1, 1));
    let order: Vec<u64> = w
        .snapshot
        .queue_order()
        .iter()
        .map(|i| i.pr_number)
        .collect();
    assert_eq!(order, [2, 1], "the new item enters behind PR 2");
    assert_replays(&w);
}

#[test]
fn a_draining_queue_refuses_new_submissions() {
    let mut w = world();
    w.submit(1, Priority::MEDIUM);
    let live = w.item(1).unwrap().id;
    w.fact(
        Actor::System,
        EventKind::QueueControlRequested {
            control: QueueControl::Drain,
        },
    );
    assert_eq!(w.snapshot.state, QueueState::Draining);
    w.submit(2, Priority::MEDIUM);
    let refused = |w: &World| {
        w.log
            .iter()
            .filter_map(|e| match e.kind {
                EventKind::ItemTransitioned {
                    item_id,
                    from: None,
                    to: ItemState::Cancelled,
                    reason,
                    ..
                } => Some((item_id, reason)),
                _ => None,
            })
            .count()
    };
    assert_eq!(refused(&w), 1);
    assert!(decided_with(&w, Reason::QueueDraining));
    // A new command on a PR in flight changes nothing (D151).
    w.submit(1, Priority::MEDIUM);
    assert_eq!(refused(&w), 1);
    assert_eq!(w.snapshot.current_item(1).map(|i| i.id), Some(live));
    assert_replays(&w);
}

#[test]
fn unmet_queue_conditions_keep_a_pr_out_of_the_queue() {
    let mut w = world();
    w.fact(
        Actor::System,
        EventKind::ItemSubmitted {
            pr_number: 1,
            head_sha: head(1, 0),
            priority: Priority::MEDIUM,
            no_batch: false,
            direct: false,
            submitted_via: cihub_domain::SubmittedVia::Api,
            author_id: cihub_domain::UserId::from_ulid(ulid::Ulid(1)),
            additions: 0,
            deletions: 0,
            changed_files: 0,
            sender_permission: None,
        },
    );
    w.observe(1, |pr| pr.unmet_conditions = vec!["label = ready".into()]);
    let item = w.item(1).unwrap();
    assert_eq!(item.state, ItemState::NotReady);
    assert_eq!(
        item.block_reasons,
        [BlockReason::QueueConditionsUnmet {
            conditions: vec!["label = ready".into()]
        }]
    );
    w.observe(1, |_| {});
    assert_eq!(w.state(1), Some(ItemState::Testing));
    // An admitted item no longer depends on its conditions (`AGENTS.md`
    // D142).
    w.observe(1, |pr| pr.unmet_conditions = vec!["label = ready".into()]);
    assert_eq!(w.state(1), Some(ItemState::Testing));
    assert_replays(&w);
}

#[test]
fn two_prs_merge_in_order() {
    let mut w = world();
    w.submit(1, Priority::MEDIUM);
    w.submit(2, Priority::MEDIUM);
    assert_eq!(w.state(1), Some(ItemState::Testing));
    assert_eq!(w.state(2), Some(ItemState::Testing));
    let branches = w.reconcile().desired.test_branches;
    assert_eq!(branches.len(), 2);
    let second = branches
        .iter()
        .find(|b| !b.predecessors.is_empty())
        .expect("PR 2 is tested on top of PR 1");
    let heads = |heads: &[Head]| -> Vec<(u64, String)> {
        heads.iter().map(|h| (h.pr_number, h.sha.clone())).collect()
    };
    assert_eq!(heads(&second.predecessors), vec![(1, head(1, 0))]);
    assert_eq!(heads(&second.members), vec![(2, head(2, 0))]);
    assert_eq!(second.name, format!("cihub-merge/pr-2/{}", second.run_id));
    assert_eq!(
        cihub_domain::run_of_test_branch(&second.name),
        Some(second.run_id)
    );

    w.assemble_all();
    w.report(2, CheckConclusion::Success);
    assert_eq!(w.state(2), Some(ItemState::TestsPassed));
    assert_eq!(w.merge_all(), 0, "PR 2 waits for PR 1");
    w.report(1, CheckConclusion::Success);
    assert_eq!(w.merge_all(), 1);
    assert_eq!(w.merge_all(), 1);
    assert!(
        w.snapshot.items.is_empty(),
        "concluded items leave the snapshot"
    );
    assert_replays(&w);
}

#[test]
fn a_culprit_fails_after_its_budget_and_its_successor_restarts() {
    let mut w = world();
    w.submit(1, Priority::MEDIUM);
    w.submit(2, Priority::MEDIUM);
    w.assemble_all();
    w.report(1, CheckConclusion::Failure);
    let first = w.item(1).unwrap().group_id.unwrap();
    assert_eq!(w.state(1), Some(ItemState::Testing), "the budget retries");
    assert_eq!(reason(&w, 1), Reason::FailureBudgetRetry);
    assert_eq!(w.snapshot.latest_run(first).unwrap().attempt, 2);

    w.assemble_all();
    w.report(1, CheckConclusion::Failure);
    assert!(failed_with(&w, Reason::TestRunRequiredStatusFailed));
    assert_eq!(w.state(1), None, "nothing cites the failed item any more");
    assert_eq!(w.state(2), Some(ItemState::Testing));
    let second = w.item(2).unwrap();
    assert!(
        w.snapshot.groups[&second.group_id.unwrap()]
            .predecessor_ids
            .is_empty()
    );
    assert_replays(&w);
}

#[test]
fn a_failure_behind_a_live_predecessor_waits() {
    let mut w = world();
    w.submit(1, Priority::MEDIUM);
    w.submit(2, Priority::MEDIUM);
    w.assemble_all();
    w.report(2, CheckConclusion::Failure);
    assert_eq!(w.state(2), Some(ItemState::PendingFailure));
    w.report(1, CheckConclusion::Success);
    w.merge_all();
    assert_eq!(
        w.state(2),
        Some(ItemState::Testing),
        "retry once merged ahead"
    );
    assert_replays(&w);
}

#[test]
fn a_failed_predecessor_restarts_its_successor() {
    let mut w = World::new(settings(|l| {
        window(2)(l);
        l.failure_budget_retries = Some(Setting::new(0));
    }));
    w.submit(1, Priority::MEDIUM);
    w.submit(2, Priority::MEDIUM);
    w.assemble_all();
    w.report(2, CheckConclusion::Success);
    w.report(1, CheckConclusion::Failure);
    assert!(failed_with(&w, Reason::TestRunRequiredStatusFailed));
    assert_eq!(w.state(2), Some(ItemState::Testing));
    assert_eq!(
        w.log
            .iter()
            .filter(|e| matches!(
                e.kind,
                EventKind::ItemTransitioned {
                    to: ItemState::Pending,
                    reason: Reason::PrerequisiteMergeItemFailed,
                    ..
                }
            ))
            .count(),
        1
    );
    assert_replays(&w);
}

#[test]
fn cancel_ends_the_run_and_restarts_successors() {
    let mut w = world();
    w.submit(1, Priority::MEDIUM);
    w.submit(2, Priority::MEDIUM);
    let run = w.run(1).unwrap();
    let item = w.item(1).unwrap().id;
    w.fact(
        Actor::System,
        EventKind::ItemCancelRequested {
            item_id: item,
            by_admin: true,
        },
    );
    assert_eq!(w.state(1), None, "the cancelled item left the snapshot");
    assert!(!w.snapshot.runs.contains_key(&run));
    assert_eq!(w.state(2), Some(ItemState::Testing));
    assert!(w.log.iter().any(|e| matches!(
        e.kind,
        EventKind::RunTransitioned {
            to: RunStatus::Cancelled,
            reason: Reason::AdminRequested,
            ..
        }
    )));
    let again = EventKind::ItemCancelRequested {
        item_id: item,
        by_admin: false,
    };
    assert_eq!(w.fact(Actor::System, again), Outcome::Ignored);
    assert_replays(&w);
}

#[test]
fn pr_changes_cancel_the_item() {
    type Edit = fn(&mut common::Pr);
    let cases: [(Edit, Reason); 4] = [
        (|p| p.state = PrState::Closed, Reason::PrClosed),
        (|p| p.head_sha = head(1, 1), Reason::PrPushedTo),
        (
            |p| p.base_branch = "release".into(),
            Reason::PrBaseBranchChanged,
        ),
        (|p| p.draft = true, Reason::PrConvertedToDraft),
    ];
    for (edit, want) in cases {
        let mut w = world();
        w.submit(1, Priority::MEDIUM);
        w.observe(1, edit);
        let cancelled = w.log.iter().any(|e| {
            matches!(e.kind, EventKind::ItemTransitioned { to: ItemState::Cancelled, reason, .. } if reason == want)
        });
        assert!(cancelled, "{want}");
        assert_replays(&w);
    }
}

#[test]
fn a_manual_merge_restarts_dependents() {
    let mut w = world();
    w.submit(1, Priority::MEDIUM);
    w.submit(2, Priority::MEDIUM);
    w.observe(1, |p| p.state = PrState::Merged);
    assert!(w.log.iter().any(|e| matches!(
        e.kind,
        EventKind::ItemTransitioned {
            reason: Reason::PrerequisiteMergeItemManuallyMerged,
            ..
        }
    )));
    assert_eq!(w.state(2), Some(ItemState::Testing));
    assert_replays(&w);
}

#[test]
fn an_external_push_restarts_every_group() {
    let mut w = world();
    w.submit(1, Priority::MEDIUM);
    w.assemble_all();
    w.report(1, CheckConclusion::Success);
    assert_eq!(w.state(1), Some(ItemState::TestsPassed));
    w.push_target(Actor::User {
        user_id: cihub_domain::UserId::from_ulid(ulid::Ulid(1)),
    });
    assert_eq!(w.state(1), Some(ItemState::Testing));
    assert!(w.log.iter().any(|e| matches!(
        e.kind,
        EventKind::ItemTransitioned {
            from: Some(ItemState::TestsPassed),
            reason: Reason::ManualPushToTargetBranch,
            ..
        }
    )));
    assert_replays(&w);
}

#[test]
fn a_late_push_changes_nothing() {
    let mut w = world();
    let old = w.snapshot.target.head.clone().unwrap().value;
    w.push_target(Actor::Bot);
    assert_eq!(
        w.push_target_to(Actor::System, old),
        Outcome::Ignored,
        "an old head"
    );
    let head = w.snapshot.target.head.clone().unwrap().value;
    assert_eq!(w.push_target_to(Actor::System, head), Outcome::Ignored);
}

#[test]
fn urgent_goes_in_front_of_groups_in_test() {
    let mut w = world();
    w.submit(1, Priority::MEDIUM);
    w.submit(2, Priority::MEDIUM);
    w.submit(3, Priority::URGENT);
    let order: Vec<u64> = w
        .snapshot
        .queue_order()
        .iter()
        .map(|i| i.pr_number)
        .collect();
    assert_eq!(order, vec![3, 1, 2]);
    assert!(w.log.iter().any(|e| matches!(
        e.kind,
        EventKind::ItemTransitioned {
            reason: Reason::SkipTheLineInterrupted,
            ..
        }
    )));
    assert_eq!(w.state(3), Some(ItemState::Testing));
    assert_replays(&w);
}

#[test]
fn high_goes_behind_groups_in_test_and_before_medium() {
    let mut w = World::new(settings(window(1)));
    w.submit(1, Priority::MEDIUM);
    w.submit(2, Priority::MEDIUM);
    w.submit(3, Priority::HIGH);
    let order: Vec<u64> = w
        .snapshot
        .queue_order()
        .iter()
        .map(|i| i.pr_number)
        .collect();
    assert_eq!(order, vec![1, 3, 2]);
    assert_replays(&w);
}

#[test]
fn a_merge_conflict_fails_the_item() {
    let mut w = world();
    w.submit(1, Priority::MEDIUM);
    let run = w.run(1).unwrap();
    w.fail_assembly(run, JobError::MergeConflict);
    assert_eq!(w.state(1), None);
    assert!(w.log.iter().any(|e| matches!(
        e.kind,
        EventKind::ItemTransitioned {
            to: ItemState::Failed,
            reason: Reason::StartTestRunMergeConflict,
            ..
        }
    )));
    assert_replays(&w);
}

#[test]
fn a_silent_check_times_out() {
    let mut w = World::new(settings(|l| {
        window(1)(l);
        l.failure_budget_retries = Some(Setting::new(0));
    }));
    w.submit(1, Priority::MEDIUM);
    w.assemble_all();
    w.advance(299);
    assert_eq!(w.state(1), Some(ItemState::Testing));
    w.advance(1);
    assert!(w.log.iter().any(|e| matches!(
        e.kind,
        EventKind::ItemTransitioned {
            to: ItemState::Failed,
            reason: Reason::TestRunTimeout,
            ..
        }
    )));
    assert_replays(&w);
}

#[test]
fn a_merge_failure_fails_the_item() {
    let mut w = world();
    w.submit(1, Priority::MEDIUM);
    w.assemble_all();
    w.report(1, CheckConclusion::Success);
    let item_id = w.item(1).unwrap().id;
    w.fact(
        Actor::System,
        EventKind::MergeFailed {
            item_id,
            direct: false,
            error: JobError::HeadChanged,
        },
    );
    assert!(w.log.iter().any(|e| matches!(
        e.kind,
        EventKind::ItemTransitioned {
            to: ItemState::Failed,
            reason: Reason::PrUpdatedAtMergeTime,
            ..
        }
    )));
    assert_replays(&w);
}

#[test]
fn a_pr_merged_outside_the_queue_is_a_manual_merge() {
    let mut w = world();
    w.submit(1, Priority::MEDIUM);
    w.submit(2, Priority::MEDIUM);
    w.assemble_all();
    w.report(1, CheckConclusion::Success);
    w.report(2, CheckConclusion::Success);
    assert_eq!(w.state(2), Some(ItemState::TestsPassed));
    let item_id = w.item(1).unwrap().id;
    w.fact(
        Actor::Bot,
        EventKind::MergeFailed {
            item_id,
            direct: false,
            error: JobError::PrAlreadyMerged,
        },
    );
    let reasons: Vec<(ItemState, Reason)> = w
        .log
        .iter()
        .filter_map(|e| match e.kind {
            EventKind::ItemTransitioned { to, reason, .. } => Some((to, reason)),
            _ => None,
        })
        .collect();
    assert!(reasons.contains(&(ItemState::Merged, Reason::ManualMerge)));
    // The successor was tested on top of the head of PR 1, not on what landed.
    assert!(reasons.contains(&(
        ItemState::Pending,
        Reason::PrerequisiteMergeItemManuallyMerged
    )));
    assert_replays(&w);
}

#[test]
fn a_blocked_pr_stays_not_ready() {
    let mut w = world();
    w.submit(1, Priority::MEDIUM);
    w.observe(1, |p| p.mergeability = Mergeability::Conflict);
    // Admitted on the mergeable observation, the item keeps its place.
    assert_eq!(w.state(1), Some(ItemState::Testing));
    w.submit(2, Priority::MEDIUM);
    w.observe(2, |p| p.mergeability = Mergeability::Behind);
    w.observe(2, |p| p.mergeability = Mergeability::Conflict);
    let item = w.item(2).unwrap();
    assert_eq!(item.state, ItemState::Testing);
    let mut w = world();
    w.fact(
        Actor::System,
        EventKind::ItemSubmitted {
            pr_number: 5,
            head_sha: head(5, 0),
            priority: Priority::MEDIUM,
            no_batch: false,
            direct: false,
            submitted_via: cihub_domain::SubmittedVia::Api,
            author_id: cihub_domain::UserId::from_ulid(ulid::Ulid(1)),
            additions: 0,
            deletions: 0,
            changed_files: 0,
            sender_permission: None,
        },
    );
    w.observe(5, |p| p.mergeability = Mergeability::Behind);
    let item = w.item(5).unwrap();
    assert_eq!(item.state, ItemState::NotReady);
    assert_eq!(
        item.block_reasons,
        vec![cihub_domain::BlockReason::BehindBase]
    );
    w.observe(5, |_| {});
    assert_eq!(w.state(5), Some(ItemState::Testing));
    assert_replays(&w);
}

#[test]
fn pause_stops_decisions_and_keeps_submissions() {
    let mut w = world();
    w.submit(1, Priority::MEDIUM);
    let control = |c| EventKind::QueueControlRequested { control: c };
    w.fact(Actor::System, control(QueueControl::Pause));
    assert_eq!(w.snapshot.state, QueueState::Paused);
    // A submission waits for the end of the pause.
    assert_eq!(w.submit(2, Priority::MEDIUM), Outcome::Applied);
    assert_eq!(w.state(2), None, "no item in pause");
    w.assemble_all();
    let run = w.run(1).unwrap();
    assert_eq!(
        w.snapshot.runs[&run].status,
        RunStatus::Assembling,
        "no decision in pause"
    );
    assert_eq!(
        w.fact(Actor::System, control(QueueControl::Pause)),
        Outcome::Ignored
    );
    w.fact(Actor::System, control(QueueControl::Resume));
    assert_eq!(
        w.state(2),
        Some(ItemState::Testing),
        "submitted at the resume"
    );
    w.report(1, CheckConclusion::Success);
    assert_eq!(w.state(1), Some(ItemState::TestsPassed));
    w.fact(Actor::System, control(QueueControl::Drain));
    assert_eq!(w.snapshot.state, QueueState::Draining);
    // A submission in drain gets a refused item (`AGENTS.md` D151).
    assert_eq!(w.submit(3, Priority::MEDIUM), Outcome::Applied);
    assert!(decided_with(&w, Reason::QueueDraining));
    w.fact(Actor::System, EventKind::AccessLost);
    assert_eq!(w.snapshot.state, QueueState::Paused);
    w.fact(Actor::System, EventKind::AccessRestored);
    assert_eq!(w.snapshot.state, QueueState::Running);
    assert_replays(&w);
}

#[test]
fn a_mode_change_waits_for_an_empty_queue() {
    let mut w = world();
    w.submit(1, Priority::MEDIUM);
    w.fact(
        Actor::System,
        EventKind::QueueSettingsChanged {
            before: QueueMode::Single,
            after: QueueMode::Parallel,
        },
    );
    assert_eq!(w.snapshot.state, QueueState::SwitchingModes);
    w.assemble_all();
    w.report(1, CheckConclusion::Success);
    w.merge_all();
    assert_eq!(w.snapshot.state, QueueState::Running);
    assert_eq!(w.snapshot.mode, QueueMode::Parallel);
    assert_replays(&w);
}

#[test]
fn disjoint_targets_are_tested_in_parallel_lanes() {
    let mut w = World::new(settings(|l| {
        window(3)(l);
        l.mode = Some(Setting::new(QueueMode::Parallel));
    }));
    for (pr, target) in [(1, "//a"), (2, "//b"), (3, "//a")] {
        w.submit(pr, Priority::MEDIUM);
        w.fact(
            Actor::System,
            EventKind::TargetsReceived {
                pr_number: pr,
                head_sha: head(pr, 0),
                targets: cihub_domain::ImpactedTargets::Targets([target.into()].into()),
            },
        );
    }
    let preds = |w: &World, pr| {
        let group = w.item(pr).unwrap().group_id.unwrap();
        w.snapshot.groups[&group].predecessor_ids.len()
    };
    assert_eq!(preds(&w, 1), 0);
    assert_eq!(preds(&w, 2), 0, "PR 2 is in another lane");
    assert_eq!(preds(&w, 3), 1, "PR 3 is behind PR 1");
    assert_replays(&w);
}

#[test]
fn restart_request_restarts_the_tests() {
    let mut w = world();
    w.submit(1, Priority::MEDIUM);
    let item_id = w.item(1).unwrap().id;
    let first = w.run(1).unwrap();
    w.fact(
        Actor::System,
        EventKind::ItemRestartRequested {
            item_id,
            by_api: true,
        },
    );
    assert_eq!(w.state(1), Some(ItemState::Testing));
    assert_ne!(w.run(1), Some(first));
    assert!(w.log.iter().any(|e| matches!(
        e.kind,
        EventKind::RunTransitioned {
            to: RunStatus::Interrupted,
            reason: Reason::TestsRestartedByApiRequest,
            ..
        }
    )));
    assert_replays(&w);
}

#[test]
fn decisions_cite_their_cause() {
    let mut w = world();
    w.submit(1, Priority::MEDIUM);
    let submitted = w
        .log
        .iter()
        .find(|e| matches!(e.kind, EventKind::ItemSubmitted { .. }));
    let created = w
        .log
        .iter()
        .find(|e| matches!(e.kind, EventKind::ItemTransitioned { from: None, .. }));
    assert_eq!(created.unwrap().cause_id, Some(submitted.unwrap().id));
    assert!(w.snapshot.requests.is_empty());
}

#[test]
fn check_finds_a_damaged_snapshot() {
    use cihub_mq_core::{Violation, check};
    let has = |violations: &[Violation], f: fn(&Violation) -> bool| violations.iter().any(f);

    let mut w = world();
    w.submit(1, Priority::MEDIUM);
    w.submit(2, Priority::MEDIUM);
    let first = w.item(1).unwrap().clone();
    let second = w.item(2).unwrap().id;
    let mut s = w.snapshot.clone();
    s.runs.get_mut(&w.run(1).unwrap()).unwrap().status = RunStatus::Interrupted;
    s.items.get_mut(&second).unwrap().state = ItemState::TestsPassed;
    s.items.get_mut(&first.id).unwrap().position = Some(i64::MAX);
    let copy = cihub_domain::Item {
        id: cihub_domain::ItemId::from_ulid(ulid::Ulid(99)),
        ..first.clone()
    };
    s.items.insert(copy.id, copy);
    let violations = check(&s, &w.settings);
    assert!(violations.contains(&Violation::TestingWithoutRun { item_id: first.id }));
    assert!(violations.contains(&Violation::PassedWithoutRun { item_id: second }));
    assert!(violations.contains(&Violation::SeveralCurrent { pr_number: 1 }));
    assert!(has(&violations, |v| matches!(
        v,
        Violation::StateMismatch { .. }
    )));
    assert!(has(&violations, |v| matches!(
        v,
        Violation::PredecessorOutOfOrder { .. }
    )));

    let violations = check(&w.snapshot, &settings(window(1)));
    assert_eq!(
        violations,
        vec![Violation::WindowExceeded {
            runs: 2,
            ceiling: 1
        }]
    );

    w.assemble_all();
    w.report(1, CheckConclusion::Success);
    let mut s = w.snapshot.clone();
    s.target.valid_bases.clear();
    let item_id = w.item(1).unwrap().id;
    assert!(check(&s, &w.settings).contains(&Violation::PassedOnStaleBase { item_id }));
}

/// Submits the PR `pr`, seen behind its base: it stays `not_ready`.
fn submit_behind(w: &mut World, pr: u64) {
    w.fact(
        Actor::System,
        EventKind::ItemSubmitted {
            pr_number: pr,
            head_sha: head(pr, 0),
            priority: Priority::MEDIUM,
            no_batch: false,
            direct: false,
            submitted_via: cihub_domain::SubmittedVia::Api,
            author_id: cihub_domain::UserId::from_ulid(ulid::Ulid(1)),
            additions: 0,
            deletions: 0,
            changed_files: 0,
            sender_permission: None,
        },
    );
    w.observe(pr, |p| p.mergeability = Mergeability::Behind);
    assert_eq!(w.state(pr), Some(ItemState::NotReady));
}

#[test]
fn a_not_ready_item_times_out() {
    let mut w = world();
    submit_behind(&mut w, 1);
    let timeout = i64::from(w.settings.not_ready_timeout_hours.value) * 60;
    w.advance(timeout - 1);
    assert_eq!(w.state(1), Some(ItemState::NotReady));
    w.advance(1);
    assert_eq!(w.state(1), None, "a cancelled item leaves the snapshot");
    let timed_out = w.log.last().unwrap();
    assert!(matches!(
        timed_out.kind,
        EventKind::ItemTransitioned {
            to: ItemState::Cancelled,
            reason: Reason::NotReadyTimeoutExpired,
            ..
        }
    ));
    assert_eq!(timed_out.cause_id, None, "the clock decided");
    assert_replays(&w);
}

#[test]
fn a_not_ready_pr_is_read_less_and_less_often() {
    let mut w = world();
    submit_behind(&mut w, 1);
    let submitted = w.item(1).unwrap().submitted_at;
    let minute = submitted
        .replace_second(0)
        .unwrap()
        .replace_nanosecond(0)
        .unwrap();
    let at = |minutes| minute + time::Duration::minutes(minutes);
    let plan = w.reconcile();
    assert!(plan.desired.observe.is_empty(), "just read");
    assert_eq!(plan.wake_at, Some(at(1)));

    w.advance(1);
    let plan = w.reconcile();
    let read = Read {
        pr_number: 1,
        point: at(1),
    };
    assert_eq!(plan.desired.observe, vec![read]);
    // The read of the target head, 5 minutes after its observation, comes
    // before the next read point of the PR.
    assert!(plan.wake_at < Some(at(7)), "{:?}", plan.wake_at);
    // A read that changes nothing is not in the log: the read stays due at
    // the same point, so that the job of this point is the same.
    w.observe(1, |p| p.mergeability = Mergeability::Behind);
    assert_eq!(w.reconcile().desired.observe, vec![read]);

    // An older item is read every 15 minutes.
    w.advance(60);
    let plan = w.reconcile();
    assert_eq!(plan.desired.observe[0].point, at(60));

    // A read that changes something comes after the point: nothing is due.
    // A mergeability that the SCM has not computed is read every minute.
    w.observe(1, |p| p.mergeability = Mergeability::Unknown);
    let plan = w.reconcile();
    assert!(plan.desired.observe.is_empty());
    assert_eq!(plan.wake_at, Some(at(62)));
    assert_replays(&w);
}

#[test]
fn every_live_pr_is_read_every_15_minutes() {
    let mut w = world();
    w.submit(1, Priority::MEDIUM);
    assert_eq!(w.state(1), Some(ItemState::Testing));
    let submitted = w.item(1).unwrap().submitted_at;
    let minute = submitted.replace_second(0).unwrap();
    w.advance(30);
    let plan = w.reconcile();
    assert_eq!(
        plan.desired.observe,
        vec![Read {
            pr_number: 1,
            point: minute + time::Duration::minutes(30),
        }]
    );
}

#[test]
fn a_silent_run_has_its_checks_read_every_10_minutes() {
    let mut w = world();
    w.submit(1, Priority::MEDIUM);
    let run_id = w.run(1).unwrap();
    let started = w.snapshot.runs[&run_id].started_at;
    w.assemble_all();
    assert!(w.reconcile().desired.check_reads.is_empty(), "just started");
    w.advance(10);
    let plan = w.reconcile();
    let read = CheckRead {
        run_id,
        test_sha: format!("test-{run_id}"),
        test_branch: w.snapshot.runs[&run_id].test_branch.clone(),
        point: started + time::Duration::minutes(10),
    };
    assert_eq!(plan.desired.check_reads, vec![read.clone()]);
    // A report moves the points: nothing is due until 10 minutes after it.
    w.fact(
        Actor::System,
        EventKind::CheckObserved {
            test_sha: format!("test-{run_id}"),
            check: CheckReport {
                name: "lint".into(),
                kind: CheckKind::Check,
                conclusion: None,
                details_url: None,
            },
        },
    );
    let reported = w.now;
    assert!(w.reconcile().desired.check_reads.is_empty());
    w.advance(25);
    let plan = w.reconcile();
    assert_eq!(
        plan.desired.check_reads,
        vec![CheckRead {
            point: reported + time::Duration::minutes(20),
            ..read
        }]
    );
    assert_replays(&w);
}

#[test]
fn a_busy_queue_reads_its_target_head_every_5_minutes() {
    let mut w = world();
    let pushed = w.snapshot.target.head.as_ref().unwrap().observed_at;
    w.advance(30);
    let plan = w.reconcile();
    assert_eq!(plan.desired.target_read, None, "nothing to do");
    assert_eq!(plan.wake_at, None);
    w.submit(1, Priority::MEDIUM);
    let plan = w.reconcile();
    assert_eq!(
        plan.desired.target_read,
        Some(pushed + time::Duration::minutes(30))
    );
    let timeout = time::Duration::minutes(i64::from(w.settings.testing_timeout_minutes.value));
    assert!(plan.wake_at < Some(w.now + timeout));

    // Without a known head, the head is read every minute.
    let mut w = World::new(w.settings.clone());
    w.snapshot.target = cihub_mq_core::TargetBranch::default();
    w.initial = w.snapshot.clone();
    w.log.clear();
    w.submit(1, Priority::MEDIUM);
    let now = w.now;
    let minute = now
        .replace_second(0)
        .unwrap()
        .replace_nanosecond(0)
        .unwrap();
    assert_eq!(w.reconcile().desired.target_read, Some(minute));
}

//! What each PR shows of its item.

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

mod common;

use cihub_config::Setting;
use cihub_domain::{
    Actor, AwaitedCheckStatus, BlockReason, CheckConclusion, CheckKind, CheckReport, CheckStatus,
    EventKind, ItemState, Mergeability, Priority, Reason, RequiredStatusesSource,
};
use cihub_mq_core::{BatchWait, Surface};
use common::{REQUIRED, World, head, settings, window};
use time::Duration;

fn shown(surfaces: &[Surface]) -> Vec<(u64, ItemState, Option<usize>, Vec<u64>)> {
    surfaces
        .iter()
        .map(|s| (s.pr_number, s.state, s.position, s.predecessors.clone()))
        .collect()
}

#[test]
fn each_live_item_shows_its_place_and_a_concluded_one_its_end() {
    let mut w = World::new(settings(window(2)));
    w.submit(1, Priority::MEDIUM);
    w.submit(2, Priority::MEDIUM);
    w.assemble_all();

    let last = w.surfaces.last().unwrap().clone();
    assert_eq!(
        shown(&last),
        [
            (1, ItemState::Testing, Some(1), vec![]),
            (2, ItemState::Testing, Some(2), vec![1]),
        ]
    );
    assert_eq!(last[0].head_sha, head(1, 0));
    assert!(last.iter().all(|s| s.batch_wait.is_none()));
    assert!(last[0].test_sha.is_some());
    assert_eq!(last[0].failing_checks, [] as [String; 0]);

    w.report(1, CheckConclusion::Failure);
    w.assemble_all();
    w.report(1, CheckConclusion::Failure);

    assert_eq!(w.state(1), None, "the failed item left the snapshot");
    let concluded = w
        .surfaces
        .iter()
        .flatten()
        .find(|s| s.pr_number == 1 && s.state == ItemState::Failed)
        .unwrap();
    assert_eq!(concluded.reason, Reason::TestRunRequiredStatusFailed);
    assert_eq!(concluded.position, None);
    assert_ne!(concluded.failing_checks, [] as [String; 0]);

    w.advance(1);
    let next = w.surfaces.last().unwrap();
    assert_eq!(next.iter().map(|s| s.pr_number).collect::<Vec<_>>(), [2]);
}

#[test]
fn a_not_ready_item_shows_why() {
    let mut w = World::new(settings(window(2)));
    w.submit_only(5, 0, Priority::MEDIUM);
    w.observe(5, |pr| pr.mergeability = Mergeability::Conflict);

    let last = w.surfaces.last().unwrap();
    assert_eq!(last[0].state, ItemState::NotReady);
    assert_eq!(last[0].position, None);
    assert_eq!(last[0].block_reasons, [BlockReason::MergeConflict]);
}

/// A blocked PR names the required check that blocks it.
#[test]
fn a_blocked_pr_names_its_required_checks() {
    let mut w = World::new(settings(window(2)));
    w.submit_only(5, 0, Priority::MEDIUM);
    let report = |name: &str, conclusion: Option<CheckConclusion>| CheckReport {
        name: name.into(),
        kind: CheckKind::Check,
        status: if conclusion.is_some() {
            CheckStatus::Completed
        } else {
            CheckStatus::InProgress
        },
        conclusion,
        details_url: None,
        started_at: None,
        completed_at: None,
    };

    w.observe(5, |pr| {
        pr.mergeability = Mergeability::Blocked;
        pr.required_checks = vec!["build".into(), "lint".into(), "e2e".into(), "unit".into()];
        pr.checks = vec![
            report("build", Some(CheckConclusion::Failure)),
            report("lint", None),
            report("unit", Some(CheckConclusion::Success)),
        ];
    });

    assert_eq!(
        w.item(5).unwrap().block_reasons,
        [
            BlockReason::FailingRequiredCheck {
                check: "build".into()
            },
            BlockReason::AwaitingRequiredCheck {
                check: "lint".into(),
                status: AwaitedCheckStatus::Running
            },
            BlockReason::AwaitingRequiredCheck {
                check: "e2e".into(),
                status: AwaitedCheckStatus::Missing
            },
        ]
    );

    w.observe(5, |pr| {
        pr.mergeability = Mergeability::Blocked;
        pr.required_checks = vec!["unit".into()];
        pr.checks = vec![report("unit", Some(CheckConclusion::Success))];
    });

    assert_eq!(
        w.item(5).unwrap().block_reasons,
        [BlockReason::Undetermined]
    );
}

#[test]
fn an_item_in_test_shows_the_draft_pr_of_its_run() {
    let mut w = World::new(settings(window(2)));
    w.submit(1, Priority::MEDIUM);
    let run_id = w.run(1).unwrap();

    w.fact(
        Actor::System,
        EventKind::TestBranchAssembled {
            run_id,
            test_sha: format!("test-{run_id}"),
            draft_pr_number: Some(42),
            required_statuses: vec![REQUIRED.into()],
            required_statuses_source: RequiredStatusesSource::RepoConfig,
        },
    );

    let last = w.surfaces.last().unwrap();
    assert_eq!(last[0].state, ItemState::Testing);
    assert_eq!(last[0].draft_pr_number, Some(42));
}

/// The members of the batch that waits, and only them, show when it closes.
#[test]
fn the_members_of_a_waiting_batch_show_when_it_closes() {
    let mut w = World::new(settings(|l| {
        window(1)(l);
        l.batching_enabled = Some(Setting::new(true));
        l.batching_size = Some(Setting::new(3));
        l.batching_max_wait_minutes = Some(Setting::new(5));
    }));

    let waits = |w: &World| -> Vec<(u64, Option<BatchWait>)> {
        w.surfaces
            .last()
            .unwrap()
            .iter()
            .map(|s| (s.pr_number, s.batch_wait))
            .collect()
    };

    for pr in 1..=3 {
        w.submit(pr, Priority::MEDIUM);
    }

    assert_eq!(
        w.state(1),
        Some(ItemState::Testing),
        "a full batch waits for nothing"
    );
    assert_eq!(waits(&w), [(1, None), (2, None), (3, None)]);

    w.submit(4, Priority::MEDIUM);
    let closes_at = w.item(4).unwrap().admitted_at.unwrap() + Duration::minutes(5);
    let wait = |members| {
        Some(BatchWait {
            members,
            size: 3,
            closes_at,
        })
    };
    assert_eq!(waits(&w)[3], (4, wait(1)));

    w.submit(5, Priority::MEDIUM);
    assert_eq!(
        waits(&w),
        [(1, None), (2, None), (3, None), (4, wait(2)), (5, wait(2))]
    );

    w.now = closes_at;
    w.reconcile();
    assert_eq!(w.state(4), Some(ItemState::Pending));
    assert!(waits(&w).iter().all(|(_, wait)| wait.is_none()));
}

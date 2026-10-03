//! What each PR shows of its item (`DECISIONS.md` D156).

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

mod common;

use cihub_domain::{
    BlockReason, CheckConclusion, CheckKind, CheckReport, ItemState, Mergeability, Priority, Reason,
};
use cihub_mq_core::Surface;
use common::{World, head, settings, window};

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
    assert!(last[0].test_branch.is_some());
    assert!(last[0].failing_checks.is_empty());

    // The budget retries, then the item fails: it leaves the snapshot, and
    // the reconciliation that concluded it still shows its end.
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
    assert!(!concluded.failing_checks.is_empty());
    // The next reconciliation shows only the live item.
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

/// A blocked PR names the required check that blocks it (`DECISIONS.md` D158).
#[test]
fn a_blocked_pr_names_its_required_checks() {
    let mut w = World::new(settings(window(2)));
    w.submit_only(5, 0, Priority::MEDIUM);
    let report = |name: &str, conclusion| CheckReport {
        name: name.into(),
        kind: CheckKind::Check,
        conclusion,
        details_url: None,
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
                status: "pending".into()
            },
            BlockReason::AwaitingRequiredCheck {
                check: "e2e".into(),
                status: "missing".into()
            },
        ]
    );
    // Every required check passed: the SCM does not say why.
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

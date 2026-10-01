//! Random queues: submissions, successes, failures, cancellations, pushes on
//! the PRs and on the target. After each step, `plan` reaches a fixed point
//! and `check` finds nothing (the world asserts both); at the end, the replay
//! of the log gives the same snapshot, also from a snapshot read back from
//! JSON in the middle of the log.

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

mod common;

use cihub_config::Setting;
use cihub_domain::{
    Actor, CheckConclusion, EventKind, JobError, PrState, Priority, RunId, RunStatus,
};
use cihub_mq_core::{Outcome, Snapshot, apply};
use common::{World, head, settings};
use proptest::prelude::*;

#[derive(Clone, Debug)]
enum Action {
    Submit(u64, u8),
    Assemble(usize),
    Conflict(usize),
    Report(usize, bool),
    Merge,
    Cancel(usize),
    PushPr(usize),
    ManualMerge(usize),
    Restart(usize),
    ExternalPush,
    Advance(i64),
    /// Assembles every run, passes every run in progress, then merges.
    Drive,
}

fn action() -> impl Strategy<Value = Action> {
    prop_oneof![
        3 => (1..8_u64, prop::sample::select(vec![0_u8, 10, 100, 200]))
            .prop_map(|(pr, p)| Action::Submit(pr, p)),
        3 => any::<usize>().prop_map(Action::Assemble),
        1 => any::<usize>().prop_map(Action::Conflict),
        4 => (any::<usize>(), prop::bool::weighted(0.7)).prop_map(|(k, ok)| Action::Report(k, ok)),
        2 => Just(Action::Merge),
        1 => any::<usize>().prop_map(Action::Cancel),
        1 => any::<usize>().prop_map(Action::PushPr),
        1 => any::<usize>().prop_map(Action::ManualMerge),
        1 => any::<usize>().prop_map(Action::Restart),
        1 => Just(Action::ExternalPush),
        1 => (1..400_i64).prop_map(Action::Advance),
        2 => Just(Action::Drive),
    ]
}

fn runs(w: &World, status: RunStatus) -> Vec<RunId> {
    w.snapshot
        .runs
        .values()
        .filter(|r| r.status == status)
        .map(|r| r.id)
        .collect()
}

fn pick<T: Copy>(items: &[T], k: usize) -> Option<T> {
    (!items.is_empty()).then(|| items[k % items.len()])
}

fn live_prs(w: &World) -> Vec<u64> {
    w.snapshot
        .items
        .values()
        .filter(|i| i.is_current && !i.state.is_terminal())
        .map(|i| i.pr_number)
        .collect()
}

fn step(w: &mut World, action: &Action) {
    match *action {
        Action::Submit(pr, priority) => {
            w.submit(pr, Priority(priority));
        }
        Action::Assemble(k) => {
            if let Some(run) = pick(&runs(w, RunStatus::Assembling), k) {
                w.assemble(run);
            }
        }
        Action::Conflict(k) => {
            if let Some(run) = pick(&runs(w, RunStatus::Assembling), k) {
                w.fail_assembly(run, JobError::MergeConflict);
            }
        }
        Action::Report(k, ok) => {
            if let Some(run) = pick(&runs(w, RunStatus::InProgress), k) {
                let conclusion = if ok {
                    CheckConclusion::Success
                } else {
                    CheckConclusion::Failure
                };
                w.report_run(run, conclusion);
            }
        }
        Action::Merge => {
            w.merge_all();
        }
        Action::Cancel(k) => {
            if let Some(pr) = pick(&live_prs(w), k) {
                let item_id = w.item(pr).unwrap().id;
                w.fact(
                    Actor::System,
                    EventKind::ItemCancelRequested {
                        item_id,
                        by_admin: false,
                    },
                );
            }
        }
        Action::PushPr(k) => {
            if let Some(pr) = pick(&live_prs(w), k) {
                w.observe(pr, |p| p.head_sha = head(pr, 1));
            }
        }
        Action::ManualMerge(k) => {
            if let Some(pr) = pick(&live_prs(w), k) {
                w.observe(pr, |p| p.state = PrState::Merged);
                w.push_target(Actor::User {
                    user_id: cihub_domain::UserId::from_ulid(ulid::Ulid(1)),
                });
            }
        }
        Action::Restart(k) => {
            if let Some(pr) = pick(&live_prs(w), k) {
                let item_id = w.item(pr).unwrap().id;
                w.fact(
                    Actor::System,
                    EventKind::ItemRestartRequested {
                        item_id,
                        by_api: false,
                    },
                );
            }
        }
        Action::ExternalPush => {
            w.push_target(Actor::User {
                user_id: cihub_domain::UserId::from_ulid(ulid::Ulid(1)),
            });
        }
        Action::Advance(minutes) => w.advance(minutes),
        Action::Drive => {
            w.assemble_all();
            for run in runs(w, RunStatus::InProgress) {
                w.report_run(run, CheckConclusion::Success);
            }
            w.merge_all();
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn random_queues_stay_consistent_and_replay(
        actions in prop::collection::vec(action(), 1..60),
        window in 1..4_u32,
        retries in 0..2_u32,
        split in 0.0..1.0_f64,
    ) {
        let mut w = World::new(settings(|l| {
            l.window_floor = Some(Setting::new(window));
            l.window_ceiling = Some(Setting::new(window));
            l.failure_budget_retries = Some(Setting::new(retries));
        }));
        for action in &actions {
            step(&mut w, action);
        }
        prop_assert_eq!(w.replay(), w.snapshot.clone());

        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_precision_loss,
            clippy::cast_sign_loss,
            reason = "an index in a short log"
        )]
        let middle = (w.log.len() as f64 * split) as usize;
        let mut snapshot = w.initial.clone();
        for event in &w.log[..middle] {
            apply(&mut snapshot, event).unwrap();
        }
        let json = serde_json::to_value(&snapshot).unwrap();
        let mut snapshot: Snapshot = serde_json::from_value(json).unwrap();
        for event in &w.log[middle..] {
            prop_assert_eq!(apply(&mut snapshot, event), Ok(Outcome::Applied));
        }
        prop_assert_eq!(snapshot, w.snapshot);
    }
}

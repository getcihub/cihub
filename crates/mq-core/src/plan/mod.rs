//! `plan`: the pure reconciler. It reads the snapshot, the settings and the
//! time, and gives the decisions of the queue and the desired state.
//!
//! `plan` takes one step at a time, applies it to a copy of the snapshot
//! with `apply`, and stops when no rule gives a step. A replay of the
//! decisions with `apply` thus gives the same state.

use cihub_config::QueueSettings;
use cihub_domain::{Actor, Event, EventId, EventKind, ItemId, QueueState, RunId};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

mod desired;
mod events;
mod reads;
mod rules;

pub use rules::formation::current_window;
pub use rules::readiness::conditions_of;

use desired::{desired, surfaces};
use reads::wake_at;
use rules::bisection::bisection;
use rules::formation::formation;
use rules::intake::{mode_switch, not_ready_timeout, pr_changes, requests, window_bounds};
use rules::readiness::{admission, readiness, validity};
use rules::runs::{
    group_follows_members, group_follows_run, members_follow_group, optimistic, pending_failure,
    run_conclusion, runs_follow_group,
};

use crate::apply::apply_seen;
use crate::ids::IdGen;
use crate::surface::{Surface, surface};
use crate::{ApplyError, Snapshot};
/// The decisions of one reconciliation and the state that the SCM must reach.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub decisions: Vec<Event>,
    pub desired: Desired,
    /// When the queue must be reconciled again without a change: a read
    /// that comes due, a timeout. `None` waits for a change.
    pub wake_at: Option<OffsetDateTime>,
}

/// The desired state: the jobs make the SCM converge to it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Desired {
    pub test_branches: Vec<TestBranch>,
    pub merges: Vec<Merge>,
    /// The PRs whose state CIHub must read again now: a webhook may be lost,
    /// and a `not_ready` item waits for a change that no webhook may report.
    pub observe: Vec<Read>,
    /// The checks that CIHub must read again now on the test SHA of a run in
    /// progress that no report reached for a while.
    pub check_reads: Vec<CheckRead>,
    /// The read point of the head of the target branch, when a read is due.
    pub target_read: Option<OffsetDateTime>,
    /// What each PR must show: its live item, or the item that this
    /// reconciliation concluded, by PR number.
    pub surfaces: Vec<Surface>,
}

/// A read of a PR that is due since its read point. The same point gives
/// the same read, so that a job reads the PR once per point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Read {
    pub pr_number: u64,
    pub point: OffsetDateTime,
}

/// A read of the checks of the test SHA of a run in progress, due since its
/// read point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckRead {
    pub run_id: RunId,
    pub test_sha: String,
    pub test_branch: String,
    pub point: OffsetDateTime,
}

/// A test branch that must exist: `base_sha`, then the heads merged in
/// order: the predecessors of the group, then its members.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestBranch {
    pub run_id: RunId,
    pub name: String,
    pub base_sha: String,
    pub predecessors: Vec<Head>,
    pub members: Vec<Head>,
}

/// The head of a PR, as the assembly merges it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Head {
    pub pr_number: u64,
    pub sha: String,
}

/// A PR that must be merged, with the `sha` guard on its head.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Merge {
    pub item_id: ItemId,
    pub pr_number: u64,
    pub head_sha: String,
    /// The run that proved the item, and its test SHA; `None` for a direct
    /// merge, whose tested SHA is the head.
    pub run_id: Option<RunId>,
    pub tested_sha: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PlanError {
    /// A decision that the tables refuse: a bug of the core.
    #[error("plan made a decision that apply refuses: {0}")]
    Refused(#[from] ApplyError),
    #[error("plan found no fixed point after {0} decisions")]
    NoFixedPoint(usize),
}

/// Events of one step, with the fact that caused them.
struct Step {
    cause: Option<EventId>,
    kinds: Vec<EventKind>,
}

impl Step {
    #[expect(
        clippy::unnecessary_wraps,
        reason = "each rule returns `Option<Step>`; this keeps their last line short"
    )]
    fn one(cause: Option<EventId>, kind: EventKind) -> Option<Self> {
        Some(Self {
            cause,
            kinds: vec![kind],
        })
    }
}

/// Computes the decisions of the queue and the desired state.
///
/// # Errors
///
/// A decision that `apply` refuses, or no fixed point: both are bugs of the
/// core.
pub fn plan(
    snapshot: &Snapshot,
    settings: &QueueSettings,
    now: OffsetDateTime,
) -> Result<Plan, PlanError> {
    let mut state = snapshot.clone();
    let mut ids = IdGen::new(snapshot, now);
    let mut decisions = Vec::new();
    let mut concluded = Vec::new();
    let limit = 64 * (snapshot.items.len() + snapshot.requests.len() + snapshot.groups.len() + 1);

    while let Some(step) = next_step(&state, settings, now, &mut ids) {
        for kind in step.kinds {
            if decisions.len() >= limit {
                return Err(PlanError::NoFixedPoint(limit));
            }

            let event = Event {
                id: EventId::from_ulid(ids.next()),
                org_id: state.org_id,
                queue_id: state.queue_id,
                actor: Actor::System,
                occurred_at: now,
                cause_id: step.cause,
                kind,
            };

            if let EventKind::ItemTransitioned { item_id, to, .. } = &event.kind
                && to.is_terminal()
            {
                let item_id = *item_id;
                apply_seen(&mut state, &event, &mut |next| {
                    let order = next.queue_order();
                    concluded.extend(
                        next.items
                            .get(&item_id)
                            .map(|item| surface(next, &order, item)),
                    );
                })?;
            } else {
                apply_seen(&mut state, &event, &mut |_| {})?;
            }

            decisions.push(event);
        }
    }

    let surfaces = surfaces(&state, settings, now, concluded);
    let desired = desired(&state, settings, now, surfaces);

    Ok(Plan {
        decisions,
        desired,
        wake_at: wake_at(&state, settings, now),
    })
}

type Rule = fn(&Snapshot, &QueueSettings, OffsetDateTime, &mut IdGen) -> Option<Step>;

/// Rules in the order they apply; the first one that gives a step wins.
const RULES: &[Rule] = &[
    requests,
    mode_switch,
    window_bounds,
    pr_changes,
    not_ready_timeout,
    run_conclusion,
    group_follows_run,
    group_follows_members,
    members_follow_group,
    optimistic,
    runs_follow_group,
    pending_failure,
    validity,
    bisection,
    readiness,
    admission,
    formation,
];

fn next_step(
    s: &Snapshot,
    settings: &QueueSettings,
    now: OffsetDateTime,
    ids: &mut IdGen,
) -> Option<Step> {
    if s.state == QueueState::Paused {
        return requests(s, settings, now, ids);
    }
    RULES.iter().find_map(|rule| rule(s, settings, now, ids))
}

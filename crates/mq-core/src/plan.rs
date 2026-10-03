//! `plan`: the pure reconciler. It reads the snapshot, the settings and the
//! time, and gives the decisions of the queue and the desired state.
//!
//! `plan` takes one step at a time, applies it to a copy of the snapshot
//! with `apply`, and stops when no rule gives a step. A replay of the
//! decisions with `apply` thus gives the same state.

use std::collections::BTreeMap;

use cihub_config::QueueSettings;
use cihub_domain::{
    Actor, BlockReason, CheckConclusion, DirectMergeMode, Event, EventId, EventKind, GroupId,
    GroupKind, GroupState, Item, ItemId, ItemState, JobError, Lifecycle, Mergeability, PrState,
    QueueMode, QueueState, Reason, ReviewDecision, RunId, RunStatus, TestGroup, TestRun,
    test_branch,
};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};

use crate::apply::apply_seen;
use crate::graph::{insertion_index, position_at, predecessors, renumber};
use crate::ids::IdGen;
use crate::snapshot::{PrObservation, Request, Subject};
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
    /// progress that no report reached for a while (`DECISIONS.md` D127).
    pub check_reads: Vec<CheckRead>,
    /// The read point of the head of the target branch, when a read is due.
    pub target_read: Option<OffsetDateTime>,
    /// What each PR must show: its live item, or the item that this
    /// reconciliation concluded (`DECISIONS.md` D156), by PR number.
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
    /// The run that proved the item, and its test SHA (`DECISIONS.md` D124);
    /// `None` for a direct merge, whose tested SHA is the head (D153).
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
            // A concluded item leaves the snapshot with its decision: its PR
            // shows it from here.
            if let EventKind::ItemTransitioned { item_id, to, .. } = &event.kind
                && to.is_terminal()
            {
                let item_id = *item_id;
                apply_seen(&mut state, &event, &mut |next| {
                    if let Some(item) = next.items.get(&item_id) {
                        concluded.push(surface(next, item));
                    }
                })?;
            } else {
                apply_seen(&mut state, &event, &mut |_| {})?;
            }
            decisions.push(event);
        }
    }
    let mut desired = desired(&state, settings, now);
    desired.surfaces = surfaces(&state, concluded);
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
    runs_follow_group,
    pending_failure,
    validity,
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

/// Optional fields of an item transition.
#[derive(Default)]
struct ItemExtras {
    position: Option<i64>,
    verified_by_run_id: Option<RunId>,
    merged_sha: Option<String>,
}

fn item_to(item: &Item, to: ItemState, reason: Reason) -> EventKind {
    item_to_with(item, to, reason, ItemExtras::default())
}

fn item_to_with(item: &Item, to: ItemState, reason: Reason, extras: ItemExtras) -> EventKind {
    EventKind::ItemTransitioned {
        item_id: item.id,
        from: Some(item.state),
        to,
        reason,
        position: extras.position,
        verified_by_run_id: extras.verified_by_run_id,
        merged_sha: extras.merged_sha,
    }
}

fn group_to(group: &TestGroup, to: GroupState, reason: Reason) -> EventKind {
    EventKind::GroupTransitioned {
        group_id: group.id,
        from: group.state,
        to,
        reason,
    }
}

fn run_to(run: &TestRun, to: RunStatus, reason: Reason, failing: Vec<String>) -> EventKind {
    EventKind::RunTransitioned {
        run_id: run.id,
        group_id: run.group_id,
        from: Some(run.status),
        to,
        reason,
        attempt: run.attempt,
        test_branch: run.test_branch.clone(),
        failure_causing_statuses: failing,
    }
}

fn new_run(
    group: &TestGroup,
    first_pr: u64,
    attempt: u32,
    reason: Reason,
    settings: &QueueSettings,
    ids: &mut IdGen,
) -> EventKind {
    let run_id = RunId::from_ulid(ids.next());
    EventKind::RunTransitioned {
        run_id,
        group_id: group.id,
        from: None,
        to: RunStatus::Assembling,
        reason,
        attempt,
        test_branch: test_branch(&settings.branch_prefix.value, first_pr, run_id),
        failure_causing_statuses: Vec::new(),
    }
}

/// The reason of a failed assembly.
const fn assembly_reason(error: JobError) -> Reason {
    match error {
        JobError::MergeConflict => Reason::StartTestRunMergeConflict,
        JobError::ConfigNotFound => Reason::StartTestRunConfigNotFound,
        JobError::ConfigParsingFailure => Reason::StartTestRunConfigParsingFailure,
        JobError::ConfigBadVersion => Reason::StartTestRunConfigBadVersion,
        JobError::ConfigMissingRequiredStatuses => {
            Reason::StartTestRunConfigMissingRequiredStatuses
        }
        JobError::ConfigBadRequiredStatuses => Reason::StartTestRunConfigBadRequiredStatuses,
        JobError::NoRequiredStatuses => Reason::StartTestRunNoRequiredStatuses,
        JobError::RetryExhausted => Reason::StartTestRunRetryExhausted,
        JobError::DraftPrsNotAllowed => Reason::StartTestRunDraftPrsNotAllowed,
        JobError::PermissionsBlockTestBranch => Reason::StartTestRunPermissionsBlockTestBranch,
        // A merge error from the assembly job is an invalid report.
        JobError::InvalidEvent
        | JobError::MergeRejected
        | JobError::RateLimitExceeded
        | JobError::BranchProtectionUnmet
        | JobError::MergeAlreadyInProgress
        | JobError::RepeatedServerErrors
        | JobError::HeadChanged
        | JobError::PrAlreadyMerged
        | JobError::DirectMergeUnavailable => Reason::StartTestRunInvalidEvent,
    }
}

/// The reason of a failed merge. A refusal of a direct merge is
/// `direct_merge_rejected` (`DECISIONS.md` D122).
const fn merge_reason(error: JobError, direct: bool) -> Reason {
    match error {
        JobError::RateLimitExceeded => Reason::MergeRateLimitExceeded,
        JobError::BranchProtectionUnmet => Reason::MergeBranchProtectionUnmet,
        JobError::MergeAlreadyInProgress => Reason::MergeAlreadyInProgress,
        JobError::RepeatedServerErrors => Reason::MergeRepeatedServerErrors,
        JobError::HeadChanged => Reason::PrUpdatedAtMergeTime,
        _ if direct => Reason::DirectMergeRejected,
        _ => Reason::MergeRejected,
    }
}

/// The new item of a submission: `not_ready`, or `cancelled` at once while
/// the queue drains or switches its mode (`DECISIONS.md` D151).
fn submitted(s: &Snapshot, ids: &mut IdGen) -> EventKind {
    let (to, reason) = match s.state {
        QueueState::Draining => (ItemState::Cancelled, Reason::QueueDraining),
        QueueState::SwitchingModes => (ItemState::Cancelled, Reason::QueueSwitchingModes),
        QueueState::Running | QueueState::Paused => (ItemState::NotReady, Reason::Submitted),
    };
    EventKind::ItemTransitioned {
        item_id: ItemId::from_ulid(ids.next()),
        from: None,
        to,
        reason,
        position: None,
        verified_by_run_id: None,
        merged_sha: None,
    }
}

/// A decision for the first request that can receive one. In `paused`,
/// only the requests on the queue itself.
fn requests(s: &Snapshot, _: &QueueSettings, _: OffsetDateTime, ids: &mut IdGen) -> Option<Step> {
    let (id, request) = s.requests.iter().find(|(_, r)| match r {
        // A submission of a new head waits for the cancel of the live item
        // of its PR (`DECISIONS.md` D152).
        Request::Submit { pr_number, .. } => {
            s.state != QueueState::Paused && s.current_item(*pr_number).is_none()
        }
        Request::Control(_) | Request::AccessLost | Request::AccessRestored => true,
        _ => s.state != QueueState::Paused,
    })?;
    let item = |item_id: &ItemId| s.items.get(item_id);
    let run = |run_id: &RunId| s.runs.get(run_id);
    let kind = match request {
        Request::Submit { .. } => submitted(s, ids),
        Request::Cancel { item_id, by_admin } => {
            let reason = if *by_admin {
                Reason::AdminRequested
            } else {
                Reason::UserRequested
            };
            item_to(item(item_id)?, ItemState::Cancelled, reason)
        }
        Request::Restart { item_id, by_api } => {
            let reason = if *by_api {
                Reason::TestsRestartedByApiRequest
            } else {
                Reason::TestsRestartedByUser
            };
            item_to(item(item_id)?, ItemState::Pending, reason)
        }
        Request::Assembled { run_id } => run_to(
            run(run_id)?,
            RunStatus::InProgress,
            Reason::TestBranchAssembled,
            Vec::new(),
        ),
        Request::AssemblyFailed { run_id, error } => run_to(
            run(run_id)?,
            RunStatus::Failed,
            assembly_reason(*error),
            Vec::new(),
        ),
        Request::MergeSucceeded {
            item_id,
            merged_sha,
        } => {
            let item = item(item_id)?;
            let reason = if item.state == ItemState::Pending {
                Reason::MergeGraphDirectMerge
            } else {
                Reason::MergeGraphMerge
            };
            item_to_with(
                item,
                ItemState::Merged,
                reason,
                ItemExtras {
                    merged_sha: Some(merged_sha.clone()),
                    ..ItemExtras::default()
                },
            )
        }
        // Someone merged the PR outside the queue (`DECISIONS.md` D132).
        Request::MergeFailed {
            item_id,
            error: JobError::PrAlreadyMerged,
            ..
        } => item_to(item(item_id)?, ItemState::Merged, Reason::ManualMerge),
        Request::MergeFailed {
            item_id,
            direct,
            error,
        } => item_to(
            item(item_id)?,
            ItemState::Failed,
            merge_reason(*error, *direct),
        ),
        Request::Control(control) => queue_to(s, control.target(), Reason::AdminRequested),
        Request::AccessLost => queue_to(s, QueueState::Paused, Reason::AccessLost),
        Request::AccessRestored => queue_to(s, QueueState::Running, Reason::AccessRestored),
    };
    Step::one(Some(*id), kind)
}

fn queue_to(s: &Snapshot, to: QueueState, reason: Reason) -> EventKind {
    EventKind::QueueTransitioned {
        from: s.state,
        to,
        reason,
    }
}

/// A mode change waits in `switching_modes` until no item is live.
fn mode_switch(s: &Snapshot, _: &QueueSettings, _: OffsetDateTime, _: &mut IdGen) -> Option<Step> {
    match s.state {
        QueueState::Running if s.requested_mode != s.mode => Step::one(
            None,
            queue_to(s, QueueState::SwitchingModes, Reason::ModeChanged),
        ),
        QueueState::SwitchingModes if s.items.values().all(|i| i.state.is_terminal()) => Step::one(
            None,
            queue_to(s, QueueState::Running, Reason::ModeSwitchCompleted),
        ),
        _ => None,
    }
}

/// The window comes back within the bounds of the settings when they
/// change.
fn window_bounds(
    s: &Snapshot,
    settings: &QueueSettings,
    _: OffsetDateTime,
    _: &mut IdGen,
) -> Option<Step> {
    let window = s.window?;
    Step::one(None, window_to(s, settings, window)?)
}

/// The item leaves the queue when its PR changes on the SCM.
fn pr_changes(
    s: &Snapshot,
    settings: &QueueSettings,
    _: OffsetDateTime,
    _: &mut IdGen,
) -> Option<Step> {
    s.items
        .values()
        .filter(|i| i.is_current && !i.state.is_terminal())
        .find_map(|item| {
            let observed = s.prs.get(&item.pr_number)?;
            let pr = &observed.value;
            let (to, reason) = if pr.state == PrState::Merged {
                // The merge job reports the merge that the queue asked for.
                if merge_ready(s, item)
                    || direct_merge(s, settings).is_some_and(|d| d.id == item.id)
                {
                    return None;
                }
                (ItemState::Merged, Reason::ManualMerge)
            } else if pr.state == PrState::Closed {
                (ItemState::Cancelled, Reason::PrClosed)
            } else if pr.head_sha != item.head_sha {
                (ItemState::Cancelled, Reason::PrPushedTo)
            } else if pr.base_branch != s.target_branch {
                (ItemState::Cancelled, Reason::PrBaseBranchChanged)
            } else if pr.draft {
                (ItemState::Cancelled, Reason::PrConvertedToDraft)
            } else if item.submitted_via == cihub_domain::SubmittedVia::Label
                && !pr.labels.contains(&settings.enqueue_label.value)
            {
                (ItemState::Cancelled, Reason::LabelRemoved)
            } else {
                return None;
            };
            Step::one(Some(observed.fact_id), item_to(item, to, reason))
        })
}

/// An item that stays `not_ready` too long leaves the queue.
fn not_ready_timeout(
    s: &Snapshot,
    settings: &QueueSettings,
    now: OffsetDateTime,
    _: &mut IdGen,
) -> Option<Step> {
    let timeout = Duration::hours(i64::from(settings.not_ready_timeout_hours.value));
    let item = not_ready_items(s).find(|i| now >= i.submitted_at + timeout)?;
    Step::one(
        None,
        item_to(item, ItemState::Cancelled, Reason::NotReadyTimeoutExpired),
    )
}

/// A run in progress passes when every required status passes, fails at
/// the first failure, and times out after `testing_timeout_minutes`.
fn run_conclusion(
    s: &Snapshot,
    settings: &QueueSettings,
    now: OffsetDateTime,
    _: &mut IdGen,
) -> Option<Step> {
    let timeout = Duration::minutes(i64::from(settings.testing_timeout_minutes.value));
    s.runs
        .values()
        .filter(|r| r.status == RunStatus::InProgress)
        .find_map(|run| {
            let checks = s.checks.get(&run.id);
            let observed = checks.into_iter().flat_map(|c| c.values());
            let failing: Vec<_> = observed
                .clone()
                .filter(|o| o.value.required)
                .filter(|o| o.value.conclusion.is_some_and(|c| !c.is_passing()))
                .collect();
            if let Some(first) = failing.first() {
                let names = failing.iter().map(|o| o.value.name.clone()).collect();
                return Step::one(
                    Some(first.fact_id),
                    run_to(
                        run,
                        RunStatus::Failed,
                        Reason::TestRunRequiredStatusFailed,
                        names,
                    ),
                );
            }
            let passing: Option<Vec<EventId>> = run
                .required_statuses
                .iter()
                .map(|name| {
                    observed
                        .clone()
                        .find(|o| {
                            o.value.name == *name
                                && o.value.conclusion.is_some_and(CheckConclusion::is_passing)
                        })
                        .map(|o| o.fact_id)
                })
                .collect();
            if let Some(facts) = passing {
                return Step::one(
                    facts.into_iter().max(),
                    run_to(run, RunStatus::Succeeded, Reason::TestRunPassed, Vec::new()),
                );
            }
            (now >= run.started_at + timeout).then(|| Step {
                cause: None,
                kinds: vec![run_to(
                    run,
                    RunStatus::TimedOut,
                    Reason::TestRunTimeout,
                    Vec::new(),
                )],
            })
        })
}

/// A group in `testing` follows the end of its latest run. A run that
/// passes outside a bisection opens the window by one.
fn group_follows_run(
    s: &Snapshot,
    settings: &QueueSettings,
    _: OffsetDateTime,
    _: &mut IdGen,
) -> Option<Step> {
    s.live_groups()
        .filter(|g| g.state == GroupState::Testing)
        .find_map(|group| {
            let run = s.latest_run(group.id)?;
            let reason = run.failure_reason;
            let (to, reason) = match (run.status, reason) {
                (RunStatus::Succeeded, _) => (GroupState::TestsPassed, Reason::TestRunPassed),
                (RunStatus::Failed | RunStatus::TimedOut, Some(r))
                    if GroupState::allowed_reasons(
                        Some(GroupState::Testing),
                        GroupState::Failed,
                    )
                    .contains(&r) =>
                {
                    (GroupState::Failed, r)
                }
                (RunStatus::Failed | RunStatus::TimedOut, Some(r)) => {
                    (GroupState::PendingFailure, r)
                }
                _ => return None,
            };
            let mut kinds = vec![group_to(group, to, reason)];
            if to == GroupState::TestsPassed && group.kind != GroupKind::Bisection {
                let wider = current_window(s, settings).saturating_add(1);
                kinds.extend(window_to(s, settings, wider));
            }
            Some(Step {
                cause: s.cause(Subject::Run(run.id)),
                kinds,
            })
        })
}

/// A group concludes, or is given up, when one of its members leaves it.
fn group_follows_members(
    s: &Snapshot,
    _: &QueueSettings,
    _: OffsetDateTime,
    _: &mut IdGen,
) -> Option<Step> {
    s.live_groups().find_map(|group| {
        let members: Vec<&Item> = group
            .item_ids
            .iter()
            .filter_map(|id| s.items.get(id))
            .collect();
        let merged_by_queue =
            |i: &Item| i.state == ItemState::Merged && i.state_reason != Reason::ManualMerge;
        if members.iter().all(|i| merged_by_queue(i)) {
            return Step::one(
                members.last().and_then(|i| s.cause(Subject::Item(i.id))),
                group_to(group, GroupState::Merged, Reason::MergeGraphMerge),
            );
        }
        let left = members.iter().find(|i| {
            !merged_by_queue(i) && (i.state.is_terminal() || i.group_id != Some(group.id))
        })?;
        let to = if left.state == ItemState::Failed && group.kind == GroupKind::Single {
            GroupState::Failed
        } else {
            GroupState::Dissolved
        };
        Step::one(
            s.cause(Subject::Item(left.id)),
            group_to(group, to, left.state_reason),
        )
    })
}

/// The members of a group take its state, with its reason.
fn members_follow_group(
    s: &Snapshot,
    _: &QueueSettings,
    _: OffsetDateTime,
    _: &mut IdGen,
) -> Option<Step> {
    s.items
        .values()
        .filter(|i| !i.state.is_terminal())
        .find_map(|item| {
            let group = s.groups.get(&item.group_id?)?;
            let to = group.state.member_state()?;
            if item.state == to {
                return None;
            }
            let verified_by_run_id = (to == ItemState::TestsPassed)
                .then(|| s.latest_run(group.id).map(|r| r.id))
                .flatten();
            Step::one(
                s.cause(Subject::Group(group.id)),
                item_to_with(
                    item,
                    to,
                    group.state_reason,
                    ItemExtras {
                        verified_by_run_id,
                        ..ItemExtras::default()
                    },
                ),
            )
        })
}

/// A live run ends when its group no longer tests it.
fn runs_follow_group(
    s: &Snapshot,
    _: &QueueSettings,
    _: OffsetDateTime,
    _: &mut IdGen,
) -> Option<Step> {
    s.runs
        .values()
        .filter(|r| !r.status.is_terminal())
        .find_map(|run| {
            let group = s.groups.get(&run.group_id)?;
            let latest = s.latest_run(group.id).map(|r| r.id);
            if group.state == GroupState::Testing && latest == Some(run.id) {
                return None;
            }
            let reason = group.state_reason;
            let to = if RunStatus::allowed_reasons(Some(run.status), RunStatus::Cancelled)
                .contains(&reason)
            {
                RunStatus::Cancelled
            } else {
                RunStatus::Interrupted
            };
            Step::one(
                s.cause(Subject::Group(group.id)),
                run_to(run, to, reason, Vec::new()),
            )
        })
}

/// At depth 0, a group in failure whose predecessors all merged is the
/// culprit: the failure budget restarts it when the window has room, then
/// it fails, and the window closes by half.
fn pending_failure(
    s: &Snapshot,
    settings: &QueueSettings,
    _: OffsetDateTime,
    ids: &mut IdGen,
) -> Option<Step> {
    s.live_groups()
        .filter(|g| g.state == GroupState::PendingFailure)
        .find_map(|group| {
            let all_merged = group.predecessor_ids.iter().all(|id| {
                s.items
                    .get(id)
                    .is_some_and(|i| i.state == ItemState::Merged)
            });
            if !all_merged {
                return None;
            }
            let run = s.latest_run(group.id)?;
            let cause = s.cause(Subject::Group(group.id));
            if run.attempt <= settings.failure_budget_retries.value {
                if !window_has_room(s, settings) {
                    return None;
                }
                let first_pr = s.items.get(group.item_ids.first()?)?.pr_number;
                return Some(Step {
                    cause,
                    kinds: vec![
                        group_to(group, GroupState::Testing, Reason::FailureBudgetRetry),
                        new_run(
                            group,
                            first_pr,
                            run.attempt + 1,
                            Reason::FailureBudgetRetry,
                            settings,
                            ids,
                        ),
                    ],
                });
            }
            let mut kinds = vec![group_to(group, GroupState::Failed, run.failure_reason?)];
            kinds.extend(window_to(s, settings, current_window(s, settings) / 2));
            Some(Step { cause, kinds })
        })
}

/// A run stays valid only while the predecessors of its group and its base
/// do not change; otherwise the group restarts.
fn validity(s: &Snapshot, _: &QueueSettings, _: OffsetDateTime, _: &mut IdGen) -> Option<Step> {
    let order = s.queue_order();
    s.live_groups().find_map(|group| {
        let first = group.item_ids.first()?;
        let index = order.iter().position(|i| i.id == *first)?;
        let cited: Vec<&Item> = group
            .predecessor_ids
            .iter()
            .filter_map(|id| s.items.get(id))
            .collect();
        let left = cited.iter().find_map(|i| {
            let reason = match (i.state, i.state_reason) {
                (ItemState::Failed, _) => Reason::PrerequisiteMergeItemFailed,
                (ItemState::Cancelled, _) => Reason::PrerequisiteMergeItemCancelled,
                (ItemState::Merged, Reason::ManualMerge) => {
                    Reason::PrerequisiteMergeItemManuallyMerged
                }
                _ => return None,
            };
            Some((reason, s.cause(Subject::Item(i.id))))
        });
        let (reason, cause) = if let Some(left) = left {
            left
        } else if !s.target.valid_bases.contains(&group.base_sha) {
            (
                Reason::ManualPushToTargetBranch,
                s.target.head.as_ref().map(|h| h.fact_id),
            )
        } else {
            let live: Vec<ItemId> = cited
                .iter()
                .filter(|i| i.state != ItemState::Merged)
                .map(|i| i.id)
                .collect();
            let expected: Vec<ItemId> = predecessors(s, &order, index)
                .into_iter()
                .filter(|id| !group.item_ids.contains(id))
                .collect();
            if live == expected {
                return None;
            }
            let new = expected.iter().find(|id| !live.contains(id));
            (
                Reason::SkipTheLineInterrupted,
                new.and_then(|id| s.cause(Subject::Item(*id))),
            )
        };
        Step::one(cause, group_to(group, GroupState::Dissolved, reason))
    })
}

/// Mergeability and block reasons of a `not_ready` item: the mergeability on
/// the SCM, the impacted targets in parallel mode, and the `queue_conditions`
/// that the job that read the PR found unmet (`DECISIONS.md` D142).
fn readiness_of(s: &Snapshot, item: &Item, pr: &PrObservation) -> Vec<BlockReason> {
    let mut blocks = match pr.mergeability {
        Mergeability::Mergeable => Vec::new(),
        Mergeability::Behind => vec![BlockReason::BehindBase],
        Mergeability::Conflict => vec![BlockReason::MergeConflict],
        Mergeability::Blocked if pr.review_decision == ReviewDecision::ChangesRequested => {
            vec![BlockReason::ChangesRequested]
        }
        Mergeability::Blocked => blocking_checks(pr),
        Mergeability::Unknown => vec![BlockReason::Undetermined],
    };
    let has_targets = s
        .targets
        .get(&item.pr_number)
        .is_some_and(|t| t.value.head_sha == item.head_sha);
    if s.mode == QueueMode::Parallel && !has_targets && blocks.is_empty() {
        blocks.push(BlockReason::Undetermined);
    }
    if !pr.unmet_conditions.is_empty() {
        blocks.push(BlockReason::QueueConditionsUnmet {
            conditions: pr.unmet_conditions.clone(),
        });
    }
    blocks
}

/// The required checks that block a PR: failed, or not concluded yet; else
/// the SCM does not say why the PR is blocked (`DECISIONS.md` D158).
fn blocking_checks(pr: &PrObservation) -> Vec<BlockReason> {
    let blocks: Vec<BlockReason> = pr
        .required_checks
        .iter()
        .filter_map(|name| {
            let report = pr.checks.iter().find(|c| c.name == *name);
            match report.map(|c| c.conclusion) {
                Some(Some(conclusion)) if conclusion.is_passing() => None,
                Some(Some(_)) => Some(BlockReason::FailingRequiredCheck {
                    check: name.clone(),
                }),
                Some(None) => Some(BlockReason::AwaitingRequiredCheck {
                    check: name.clone(),
                    status: "pending".into(),
                }),
                None => Some(BlockReason::AwaitingRequiredCheck {
                    check: name.clone(),
                    status: "missing".into(),
                }),
            }
        })
        .collect();
    if blocks.is_empty() {
        vec![BlockReason::Undetermined]
    } else {
        blocks
    }
}

fn not_ready_items(s: &Snapshot) -> impl Iterator<Item = &Item> {
    s.items
        .values()
        .filter(|i| i.state == ItemState::NotReady && i.is_current)
}

fn readiness(s: &Snapshot, _: &QueueSettings, _: OffsetDateTime, _: &mut IdGen) -> Option<Step> {
    not_ready_items(s).find_map(|item| {
        let observed = s.prs.get(&item.pr_number)?;
        let blocks = readiness_of(s, item, &observed.value);
        if item.mergeability == observed.value.mergeability && item.block_reasons == blocks {
            return None;
        }
        Step::one(
            Some(observed.fact_id),
            EventKind::ItemReadinessChanged {
                item_id: item.id,
                mergeability: observed.value.mergeability,
                block_reasons: blocks,
            },
        )
    })
}

/// A ready item enters the queue at the place its priority gives.
fn admission(s: &Snapshot, _: &QueueSettings, _: OffsetDateTime, _: &mut IdGen) -> Option<Step> {
    if s.state != QueueState::Running {
        return None;
    }
    let item = not_ready_items(s).find(|i| {
        s.prs
            .get(&i.pr_number)
            .is_some_and(|o| o.value.mergeability == i.mergeability)
            && i.block_reasons.is_empty()
    })?;
    let cause = s.prs.get(&item.pr_number).map(|o| o.fact_id);
    let order = s.queue_order();
    let index = insertion_index(&order, item.priority);
    let kind = match position_at(&order, index) {
        Some(position) => item_to_with(
            item,
            ItemState::Pending,
            Reason::MergeItemNowReady,
            ItemExtras {
                position: Some(position),
                ..ItemExtras::default()
            },
        ),
        None => EventKind::QueueRenumbered {
            positions: renumber(&order, index).into_iter().collect(),
        },
    };
    Step::one(cause, kind)
}

/// Forms a `single` group for the first pending item whose predecessors are
/// all in groups, while the window has room.
fn formation(
    s: &Snapshot,
    settings: &QueueSettings,
    _: OffsetDateTime,
    ids: &mut IdGen,
) -> Option<Step> {
    let base = &s.target.head.as_ref()?.value;
    // The item that the queue merges directly waits for the answer of the
    // merge job (`DECISIONS.md` D153).
    if !window_has_room(s, settings) || direct_merge(s, settings).is_some() {
        return None;
    }
    let order = s.queue_order();
    let (index, item) = order.iter().enumerate().find(|(index, item)| {
        item.state == ItemState::Pending
            && predecessors(s, &order, *index).iter().all(|id| {
                s.items
                    .get(id)
                    .is_some_and(|p| p.state != ItemState::Pending)
            })
    })?;
    let group = TestGroup {
        id: GroupId::from_ulid(ids.next()),
        org_id: s.org_id,
        queue_id: s.queue_id,
        kind: GroupKind::Single,
        state: GroupState::Testing,
        state_reason: Reason::TestRunStarted,
        item_ids: vec![item.id],
        parent_group_id: None,
        predecessor_ids: predecessors(s, &order, index),
        base_sha: base.clone(),
    };
    let run = new_run(
        &group,
        item.pr_number,
        1,
        Reason::TestRunStarted,
        settings,
        ids,
    );
    Some(Step {
        cause: s.cause(Subject::Item(item.id)),
        kinds: vec![
            EventKind::GroupFormed {
                group_id: group.id,
                kind: group.kind,
                item_ids: group.item_ids,
                predecessor_ids: group.predecessor_ids,
                base_sha: group.base_sha,
                parent_group_id: None,
            },
            run,
        ],
    })
}

/// The speculation window: the floor until its first change.
fn current_window(s: &Snapshot, settings: &QueueSettings) -> u32 {
    s.window.unwrap_or(settings.window_floor.value)
}

/// The decision that moves the window to `after`, within the bounds of the
/// settings, or `None` when the window does not change.
fn window_to(s: &Snapshot, settings: &QueueSettings, after: u32) -> Option<EventKind> {
    let before = current_window(s, settings);
    let after = after.clamp(settings.window_floor.value, settings.window_ceiling.value);
    (after != before).then_some(EventKind::QueueWindowChanged { before, after })
}

/// The window is the number of runs in progress outside bisections.
fn window_has_room(s: &Snapshot, settings: &QueueSettings) -> bool {
    let window = current_window(s, settings);
    let running = s
        .live_groups()
        .filter(|g| g.kind != GroupKind::Bisection && s.live_run(g.id).is_some())
        .count();
    running < usize::try_from(window).unwrap_or(usize::MAX)
}

/// The item that the queue merges directly, without a test branch
/// (`DECISIONS.md` D153): with `direct_merge_mode: always`, the first item of a
/// running queue without a live group, when it is `pending` and no direct
/// merge of it was tried.
fn direct_merge<'a>(s: &'a Snapshot, settings: &QueueSettings) -> Option<&'a Item> {
    if settings.direct_merge_mode.value != DirectMergeMode::Always
        || s.state != QueueState::Running
        || s.live_groups().next().is_some()
    {
        return None;
    }
    let first = *s.queue_order().first()?;
    (first.state == ItemState::Pending && !first.direct_merge_tried).then_some(first)
}

/// A tested item merges once its predecessors, and the members ahead of it
/// in its group, have merged.
fn merge_ready(s: &Snapshot, item: &Item) -> bool {
    let merged = |id: &ItemId| {
        s.items
            .get(id)
            .is_some_and(|i| i.state == ItemState::Merged)
    };
    item.state == ItemState::TestsPassed
        && item
            .group_id
            .and_then(|g| s.groups.get(&g))
            .is_some_and(|g| {
                g.state == GroupState::TestsPassed
                    && g.predecessor_ids.iter().all(merged)
                    && g.item_ids
                        .iter()
                        .take_while(|id| **id != item.id)
                        .all(merged)
            })
}

/// Read points of a PR after its submission, in minutes: then every 15
/// minutes.
const READ_POINTS: [i64; 4] = [1, 3, 7, 15];

/// A run in progress without a report has its checks read this often.
const CHECK_READ_EVERY: i64 = 10;

/// The head of the target branch is read this often while the queue has
/// work.
const TARGET_READ_EVERY: i64 = 5;

/// The last read point at or before `now`, and the next one after `now`.
/// The points depend only on the age of the item, not on the last read: a
/// read that changes nothing is not in the log. They count from the minute of
/// the submission, so that the reads of one minute go together. While the SCM
/// has not computed the mergeability, a point comes every minute.
fn read_points(item: &Item, now: OffsetDateTime) -> (Option<OffsetDateTime>, OffsetDateTime) {
    let minute = floor_to_minute(item.submitted_at);
    let at = |minutes: i64| minute + Duration::minutes(minutes);
    let age = (now - minute).whole_minutes().max(0);
    if item.mergeability == Mergeability::Unknown {
        return ((age >= 1).then(|| at(age)), at(age + 1));
    }
    let last = READ_POINTS
        .iter()
        .rev()
        .find(|&&point| point <= age)
        .map(|&point| point.max(age / 15 * 15));
    let next = READ_POINTS
        .iter()
        .find(|&&point| point > age)
        .copied()
        .unwrap_or((age / 15 + 1) * 15);
    (last.map(at), at(next))
}

/// The read of the PR of a live item, when its last point came after the
/// last observation.
fn read_due(s: &Snapshot, item: &Item, now: OffsetDateTime) -> Option<Read> {
    let (point, _) = read_points(item, now);
    let point = point?;
    let observed = s.prs.get(&item.pr_number).map(|o| o.observed_at);
    (observed.is_none_or(|at| at < point)).then_some(Read {
        pr_number: item.pr_number,
        point,
    })
}

/// The live items whose PR CIHub reads (`DECISIONS.md` D127).
fn live_items(s: &Snapshot) -> impl Iterator<Item = &Item> {
    s.items
        .values()
        .filter(|i| i.is_current && !i.state.is_terminal())
}

/// The last point of a grid of `every` minutes from `base` at or before
/// `now`, without `base` itself, and the next point after `now`.
fn grid(
    base: OffsetDateTime,
    every: i64,
    now: OffsetDateTime,
) -> (Option<OffsetDateTime>, OffsetDateTime) {
    let step = Duration::minutes(every);
    let steps = ((now - base).whole_minutes() / every).max(0);
    let at = |n: i64| base + step * i32::try_from(n).unwrap_or(i32::MAX);
    ((steps >= 1).then(|| at(steps)), at(steps + 1))
}

/// The last report on the test SHA of a run in progress, or its start.
fn last_report(s: &Snapshot, run: &TestRun) -> OffsetDateTime {
    s.checks
        .get(&run.id)
        .into_iter()
        .flat_map(|checks| checks.values())
        .map(|o| o.observed_at)
        .fold(run.started_at, OffsetDateTime::max)
}

fn runs_in_progress(s: &Snapshot) -> impl Iterator<Item = &TestRun> {
    s.runs
        .values()
        .filter(|r| r.status == RunStatus::InProgress)
}

/// The queue reads the head of its target while it has work: from the last
/// observation, or every minute while it does not know the head.
fn target_points(
    s: &Snapshot,
    now: OffsetDateTime,
) -> Option<(Option<OffsetDateTime>, OffsetDateTime)> {
    let busy = s.items.values().any(|i| !i.state.is_terminal())
        || s.requests
            .values()
            .any(|r| matches!(r, Request::Submit { .. }));
    if !busy {
        return None;
    }
    if let Some(head) = &s.target.head {
        return Some(grid(head.observed_at, TARGET_READ_EVERY, now));
    }
    let minute = floor_to_minute(now);
    Some((Some(minute), minute + Duration::minutes(1)))
}

fn floor_to_minute(t: OffsetDateTime) -> OffsetDateTime {
    t - Duration::seconds(i64::from(t.second())) - Duration::nanoseconds(i64::from(t.nanosecond()))
}

/// The earliest of the next read points, the not-ready timeouts and the run
/// timeouts.
fn wake_at(s: &Snapshot, settings: &QueueSettings, now: OffsetDateTime) -> Option<OffsetDateTime> {
    let not_ready = Duration::hours(i64::from(settings.not_ready_timeout_hours.value));
    let testing = Duration::minutes(i64::from(settings.testing_timeout_minutes.value));
    let reads = live_items(s).map(|item| read_points(item, now).1);
    let timeouts = not_ready_items(s).map(|item| item.submitted_at + not_ready);
    let runs = runs_in_progress(s).flat_map(|r| {
        let (_, next_read) = grid(last_report(s, r), CHECK_READ_EVERY, now);
        [r.started_at + testing, next_read]
    });
    let target = target_points(s, now).map(|(_, next)| next);
    reads.chain(timeouts).chain(runs).chain(target).min()
}

fn desired(s: &Snapshot, settings: &QueueSettings, now: OffsetDateTime) -> Desired {
    let heads = |ids: &[ItemId]| -> Vec<Head> {
        ids.iter()
            .filter_map(|id| s.items.get(id))
            .map(|item| Head {
                pr_number: item.pr_number,
                sha: item.head_sha.clone(),
            })
            .collect()
    };
    let test_branches = s
        .runs
        .values()
        .filter(|r| !r.status.is_terminal())
        .filter_map(|run| {
            let group = s.groups.get(&run.group_id)?;
            Some(TestBranch {
                run_id: run.id,
                name: run.test_branch.clone(),
                base_sha: group.base_sha.clone(),
                predecessors: heads(&group.predecessor_ids),
                members: heads(&group.item_ids),
            })
        })
        .collect();
    let merges = if s.state == QueueState::Paused {
        Vec::new()
    } else {
        s.queue_order()
            .into_iter()
            .filter(|i| merge_ready(s, i))
            .filter_map(|i| {
                let run = s.runs.get(&i.verified_by_run_id?)?;
                Some(Merge {
                    item_id: i.id,
                    pr_number: i.pr_number,
                    head_sha: i.head_sha.clone(),
                    run_id: Some(run.id),
                    tested_sha: run.test_sha.clone()?,
                })
            })
            .chain(direct_merge(s, settings).map(|i| Merge {
                item_id: i.id,
                pr_number: i.pr_number,
                head_sha: i.head_sha.clone(),
                run_id: None,
                tested_sha: i.head_sha.clone(),
            }))
            .collect()
    };
    let observe = live_items(s)
        .filter_map(|item| read_due(s, item, now))
        .collect();
    let check_reads = runs_in_progress(s)
        .filter_map(|run| {
            let (point, _) = grid(last_report(s, run), CHECK_READ_EVERY, now);
            Some(CheckRead {
                run_id: run.id,
                test_sha: run.test_sha.clone()?,
                test_branch: run.test_branch.clone(),
                point: point?,
            })
        })
        .collect();
    let target_read = target_points(s, now).and_then(|(point, _)| point);
    Desired {
        test_branches,
        merges,
        observe,
        check_reads,
        target_read,
        surfaces: Vec::new(),
    }
}

/// The surface of each PR: its live item, else the last item that this
/// reconciliation concluded.
fn surfaces(s: &Snapshot, concluded: Vec<Surface>) -> Vec<Surface> {
    let mut by_pr: BTreeMap<u64, Surface> = concluded
        .into_iter()
        .map(|surface| (surface.pr_number, surface))
        .collect();
    for item in live_items(s) {
        by_pr.insert(item.pr_number, surface(s, item));
    }
    by_pr.into_values().collect()
}

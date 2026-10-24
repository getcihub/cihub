//! Rules that make the runs, the groups and their members follow each other.

use cihub_config::QueueSettings;
use cihub_domain::{
    CheckConclusion, EventId, GroupKind, GroupState, Item, ItemState, Lifecycle, Reason, RunStatus,
    TestGroup,
};
use time::{Duration, OffsetDateTime};

use crate::Snapshot;
use crate::ids::IdGen;
use crate::snapshot::Subject;

use crate::plan::Step;
use crate::plan::events::{ItemExtras, group_to, item_to_with, new_run, run_to};
use crate::plan::rules::formation::{bisection_room, current_window, window_has_room, window_to};

/// A run in progress passes when every required status passes, fails at
/// the first failure, and times out after `testing_timeout_minutes`.
pub(in crate::plan) fn run_conclusion(
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
pub(in crate::plan) fn group_follows_run(
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
                (RunStatus::Failed, Some(r @ Reason::StartTestRunMergeConflict))
                    if group.item_ids.len() > 1 =>
                {
                    (GroupState::PendingFailure, r)
                }
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
                let wider = current_window(s.window, settings).saturating_add(1);
                kinds.extend(window_to(s, settings, wider));
            }

            Some(Step {
                cause: s.cause(Subject::Run(run.id)),
                kinds,
            })
        })
}

/// With `optimistic_merging`, a run that passes proves the groups that it
/// tested as predecessors: a group still in `testing` passes with it
/// (`passed_by_optimization`), and its run stops; a group in
/// `pending_failure` is released (`successor_test_run_passed`). Their members
/// record that run. A group in bisection is not proved this way: its halves
/// are. A group that is evicted still restarts the groups behind
/// it.
pub(in crate::plan) fn optimistic(
    s: &Snapshot,
    settings: &QueueSettings,
    _: OffsetDateTime,
    _: &mut IdGen,
) -> Option<Step> {
    if !settings.optimistic_merging.value {
        return None;
    }
    s.live_groups()
        .filter(|g| g.state == GroupState::TestsPassed)
        .find_map(|proof| {
            let run = s
                .latest_run(proof.id)
                .filter(|r| r.status == RunStatus::Succeeded)?;

            let proved = |g: &&TestGroup| {
                let mut live = g
                    .item_ids
                    .iter()
                    .filter_map(|id| s.items.get(id))
                    .filter(|i| !i.state.is_terminal())
                    .peekable();
                live.peek().is_some() && live.all(|i| proof.predecessor_ids.contains(&i.id))
            };

            let group = s
                .live_groups()
                .filter(|g| matches!(g.state, GroupState::Testing | GroupState::PendingFailure))
                .filter(|g| children(s, g).next().is_none())
                .find(proved)?;

            let reason = if group.state == GroupState::Testing {
                Reason::PassedByOptimization
            } else {
                Reason::SuccessorTestRunPassed
            };

            let mut kinds = vec![group_to(group, GroupState::TestsPassed, reason)];
            kinds.extend(
                group
                    .item_ids
                    .iter()
                    .filter_map(|id| s.items.get(id))
                    .filter(|i| i.group_id == Some(group.id) && !i.state.is_terminal())
                    .map(|i| {
                        item_to_with(
                            i,
                            ItemState::TestsPassed,
                            reason,
                            ItemExtras {
                                verified_by_run_id: Some(run.id),
                                ..ItemExtras::default()
                            },
                        )
                    }),
            );

            Some(Step {
                cause: s.cause(Subject::Run(run.id)),
                kinds,
            })
        })
}

/// A group concludes, or is given up, when one of its members leaves it.
pub(in crate::plan) fn group_follows_members(
    s: &Snapshot,
    _: &QueueSettings,
    _: OffsetDateTime,
    _: &mut IdGen,
) -> Option<Step> {
    s.live_groups().find_map(|group| {
        let bisecting = children(s, group).next().is_some();
        let members: Vec<&Item> = group
            .item_ids
            .iter()
            .filter_map(|id| s.items.get(id))
            .filter(|i| !bisecting || i.group_id == Some(group.id))
            .collect();

        if members.is_empty() {
            return None;
        }

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

        let reason = if members.len() > 1 && left.state.is_terminal() {
            Reason::BatchMemberLeft
        } else {
            left.state_reason
        };

        Step::one(s.cause(Subject::Item(left.id)), group_to(group, to, reason))
    })
}

/// The members of a group take its state, with its reason.
pub(in crate::plan) fn members_follow_group(
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
pub(in crate::plan) fn runs_follow_group(
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

/// At depth 0, a group of one PR in failure whose predecessors all merged is
/// the culprit: the failure budget restarts it when the window (or the
/// bisection pool) has room, then it fails, and the window closes by half,
/// once per failure: a bisection already closed it.
pub(in crate::plan) fn pending_failure(
    s: &Snapshot,
    settings: &QueueSettings,
    _: OffsetDateTime,
    ids: &mut IdGen,
) -> Option<Step> {
    s.live_groups()
        .filter(|g| g.state == GroupState::PendingFailure && g.item_ids.len() == 1)
        .find_map(|group| {
            if !predecessors_merged(s, group) || successors_testing(s, settings, group) {
                return None;
            }

            let run = s.latest_run(group.id)?;
            let cause = s.cause(Subject::Group(group.id));
            let item = s.items.get(group.item_ids.first()?)?;
            let budget = settings
                .of_profile(item.profile.as_deref())
                .failure_budget_retries
                .value;

            if run.attempt <= budget {
                let room = if group.kind == GroupKind::Bisection {
                    bisection_room(s, settings) > 0
                } else {
                    window_has_room(s, settings)
                };

                if !room {
                    return None;
                }

                let first_pr = item.pr_number;
                return Some(Step {
                    cause,
                    kinds: vec![
                        group_to(group, GroupState::Testing, Reason::FailureBudgetRetry),
                        new_run(
                            group.id,
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
            if group.kind != GroupKind::Bisection {
                kinds.extend(window_to(
                    s,
                    settings,
                    current_window(s.window, settings) / 2,
                ));
            }

            Some(Step { cause, kinds })
        })
}

pub(in crate::plan) fn predecessors_merged(s: &Snapshot, group: &TestGroup) -> bool {
    group.predecessor_ids.iter().all(|id| {
        s.items
            .get(id)
            .is_some_and(|i| i.state == ItemState::Merged)
    })
}

/// With `pending_failure_depth: N > 0`, a group in failure also waits for the
/// runs of the first `N` groups behind it whose runs test its members: their
/// runs only, not their final state, so that no two groups wait for each
/// other. The successors are read again at each reconciliation, so that a
/// group formed after the failure counts.
pub(in crate::plan) fn successors_testing(
    s: &Snapshot,
    settings: &QueueSettings,
    group: &TestGroup,
) -> bool {
    let depth = usize::try_from(settings.pending_failure_depth.value).unwrap_or(usize::MAX);
    if depth == 0 {
        return false;
    }

    let position = |g: &TestGroup| {
        g.item_ids
            .iter()
            .filter_map(|id| s.items.get(id))
            .filter_map(|i| i.position)
            .min()
    };

    let mut successors: Vec<&TestGroup> = s
        .live_groups()
        .filter(|g| {
            g.predecessor_ids
                .iter()
                .any(|id| group.item_ids.contains(id))
        })
        .collect();
    successors.sort_by_key(|g| position(g));

    successors
        .into_iter()
        .take(depth)
        .any(|g| s.live_run(g.id).is_some())
}

/// The halves that the bisection of a group formed.
pub(in crate::plan) fn children<'a>(
    s: &'a Snapshot,
    group: &TestGroup,
) -> impl Iterator<Item = &'a TestGroup> {
    let id = group.id;
    s.groups
        .values()
        .filter(move |g| g.parent_group_id == Some(id))
}

/// The members still in a group, in queue order: a bisection moves the
/// tested half to a child.
pub(in crate::plan) fn waiting<'a>(s: &'a Snapshot, group: &TestGroup) -> Vec<&'a Item> {
    let mut items: Vec<&Item> = group
        .item_ids
        .iter()
        .filter_map(|id| s.items.get(id))
        .filter(|i| i.group_id == Some(group.id) && !i.state.is_terminal())
        .collect();
    items.sort_by_key(|i| i.position);
    items
}

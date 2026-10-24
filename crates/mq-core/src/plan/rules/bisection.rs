//! Rules that bisect a failed group.

use cihub_config::QueueSettings;
use cihub_domain::{
    EventKind, GroupId, GroupKind, GroupState, Item, ItemState, OptimizationMode, Reason, TestGroup,
};
use time::OffsetDateTime;

use crate::Snapshot;
use crate::graph::predecessors;
use crate::ids::IdGen;
use crate::snapshot::Subject;

use crate::plan::Step;
use crate::plan::events::{group_to, item_to, new_run};
use crate::plan::rules::formation::{bisection_room, current_window, window_to};
use crate::plan::rules::runs::{children, predecessors_merged, successors_testing, waiting};

/// A group of several PRs in failure whose predecessors all merged is bisected
/// at once, without the failure budget: the first half is tested in a child
/// group, the other half waits in the group. When the child passes, the other
/// half holds the failure: it is split in turn, and a half of one PR is tested
/// alone, because an inferred failure never evicts a PR; with
/// `optimization_mode: off`, it goes back to the queue instead. When the child
/// fails or is given up, the other half goes back to the queue. The first split
/// closes the window by half.
pub(in crate::plan) fn bisection(
    s: &Snapshot,
    settings: &QueueSettings,
    _: OffsetDateTime,
    ids: &mut IdGen,
) -> Option<Step> {
    s.live_groups()
        .filter(|g| {
            g.state == GroupState::PendingFailure
                && g.item_ids.len() > 1
                && predecessors_merged(s, g)
        })
        .filter(|g| children(s, g).next().is_some() || !successors_testing(s, settings, g))
        .find_map(|group| {
            let cause = s.cause(Subject::Group(group.id));
            let waiting = waiting(s, group);

            if waiting.is_empty() {
                return Step::one(
                    cause,
                    group_to(group, GroupState::Bisected, Reason::BisectionRequired),
                );
            }

            let children: Vec<&TestGroup> = children(s, group).collect();
            if children.iter().any(|c| c.state == GroupState::Testing) {
                return None;
            }

            let passed = !children.is_empty()
                && children
                    .iter()
                    .all(|c| matches!(c.state, GroupState::TestsPassed | GroupState::Merged));
            let reason = if passed {
                Reason::BisectionTestRunPassed
            } else {
                Reason::BisectionRequired
            };

            let skip =
                settings.optimization_mode.value == OptimizationMode::BisectionSkipRedundantTests;
            if !children.is_empty() && (!passed || !skip) {
                let mut kinds: Vec<EventKind> = waiting
                    .iter()
                    .map(|i| item_to(i, ItemState::Pending, reason))
                    .collect();
                kinds.push(group_to(
                    group,
                    GroupState::Bisected,
                    Reason::BisectionRequired,
                ));
                return Some(Step { cause, kinds });
            }

            let mut kinds = split(s, settings, ids, group, &waiting, reason)?;
            if children.is_empty() {
                kinds.extend(
                    s.live_groups()
                        .filter(|g| g.id != group.id)
                        .filter(|g| {
                            g.predecessor_ids
                                .iter()
                                .any(|id| group.item_ids.contains(id))
                        })
                        .map(|g| {
                            group_to(
                                g,
                                GroupState::Dissolved,
                                Reason::PrerequisiteMergeItemFailed,
                            )
                        }),
                );
                if group.kind == GroupKind::Batch {
                    kinds.extend(window_to(
                        s,
                        settings,
                        current_window(s.window, settings) / 2,
                    ));
                }
            }

            Some(Step { cause, kinds })
        })
}

/// Cuts `waiting` in two halves, the first rounded up, and tests each one in a
/// new child group of `parent`, at the same time while the bisection pool has
/// room: the second half is tested on top of the first. A half that finds no
/// room waits in `parent`.
fn split(
    s: &Snapshot,
    settings: &QueueSettings,
    ids: &mut IdGen,
    parent: &TestGroup,
    waiting: &[&Item],
    reason: Reason,
) -> Option<Vec<EventKind>> {
    let room = bisection_room(s, settings);
    if room == 0 {
        return None;
    }

    let base = &s.target.head.as_ref()?.value;
    let order = s.queue_order();
    let (first_half, second_half) = waiting.split_at(waiting.len().div_ceil(2));
    let mut kinds = Vec::new();

    for half in [first_half, second_half]
        .into_iter()
        .filter(|h| !h.is_empty())
        .take(room)
    {
        let first = half.first()?;
        let index = order.iter().position(|i| i.id == first.id)?;
        let group_id = GroupId::from_ulid(ids.next());

        kinds.extend(half.iter().map(|i| item_to(i, ItemState::Pending, reason)));
        kinds.push(EventKind::GroupFormed {
            group_id,
            kind: GroupKind::Bisection,
            item_ids: half.iter().map(|i| i.id).collect(),
            predecessor_ids: predecessors(s, &order, index),
            base_sha: base.clone(),
            parent_group_id: Some(parent.id),
        });
        kinds.push(new_run(
            group_id,
            first.pr_number,
            1,
            Reason::TestRunStarted,
            settings,
            ids,
        ));
    }

    Some(kinds)
}

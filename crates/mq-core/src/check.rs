//! `check`: the consistency rules of `docs/RELIABILITY.md`, on a snapshot
//! after `plan`.

use std::collections::BTreeMap;

use cihub_config::QueueSettings;
use cihub_domain::{GroupId, GroupKind, GroupState, ItemId, ItemState, RunStatus};

use crate::Snapshot;

/// A broken consistency rule.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Violation {
    #[error("item {item_id} is testing without a group that has a run in progress")]
    TestingWithoutRun { item_id: ItemId },
    #[error("item {item_id} is in more than one live group")]
    SeveralGroups { item_id: ItemId },
    #[error("item {item_id} is {item_state} in group {group_id}, which is {group_state}")]
    StateMismatch {
        item_id: ItemId,
        group_id: GroupId,
        item_state: ItemState,
        group_state: GroupState,
    },
    #[error("predecessor {item_id} of group {group_id} is neither merged nor live ahead of it")]
    PredecessorOutOfOrder { group_id: GroupId, item_id: ItemId },
    #[error("{runs} runs in progress, over the window ceiling {ceiling}")]
    WindowExceeded { runs: usize, ceiling: u32 },
    #[error("item {item_id} is tests_passed without a successful run")]
    PassedWithoutRun { item_id: ItemId },
    #[error("item {item_id} is tests_passed on a base that a push replaced")]
    PassedOnStaleBase { item_id: ItemId },
    #[error("PR {pr_number} has more than one current item")]
    SeveralCurrent { pr_number: u64 },
}

/// Returns every rule that `snapshot` breaks.
pub fn check(snapshot: &Snapshot, settings: &QueueSettings) -> Vec<Violation> {
    let mut violations = Vec::new();
    let live_groups: Vec<_> = snapshot.live_groups().collect();
    let order = snapshot.queue_order();
    let position = |id: &ItemId| order.iter().position(|i| i.id == *id);

    for item in snapshot.items.values() {
        let containing: Vec<_> = live_groups
            .iter()
            .filter(|g| g.item_ids.contains(&item.id))
            .collect();
        let groups = containing
            .iter()
            .filter(|g| !containing.iter().any(|c| c.parent_group_id == Some(g.id)))
            .count();

        if groups > 1 {
            violations.push(Violation::SeveralGroups { item_id: item.id });
        }

        let group = item.group_id.and_then(|g| snapshot.groups.get(&g));
        if item.state == ItemState::Testing
            && group.is_none_or(|g| snapshot.live_run(g.id).is_none())
        {
            violations.push(Violation::TestingWithoutRun { item_id: item.id });
        }

        if let Some(group) = group.filter(|g| !g.state.is_terminal()) {
            let expected = group.state.member_state().unwrap_or(ItemState::TestsPassed);
            let merged_ahead =
                group.state == GroupState::TestsPassed && item.state == ItemState::Merged;
            if item.state != expected && !merged_ahead {
                violations.push(Violation::StateMismatch {
                    item_id: item.id,
                    group_id: group.id,
                    item_state: item.state,
                    group_state: group.state,
                });
            }
        }

        if item.state == ItemState::TestsPassed {
            let verified = item
                .verified_by_run_id
                .and_then(|r| snapshot.runs.get(&r))
                .is_some_and(|r| r.status == RunStatus::Succeeded);
            if !verified {
                violations.push(Violation::PassedWithoutRun { item_id: item.id });
            }
            if group.is_some_and(|g| !snapshot.target.valid_bases.contains(&g.base_sha)) {
                violations.push(Violation::PassedOnStaleBase { item_id: item.id });
            }
        }
    }

    for group in &live_groups {
        let own = group.item_ids.first().and_then(position);
        for id in &group.predecessor_ids {
            let merged = snapshot
                .items
                .get(id)
                .is_some_and(|i| i.state == ItemState::Merged);
            let ahead = matches!((position(id), own), (Some(p), Some(o)) if p < o);
            if !merged && !ahead {
                violations.push(Violation::PredecessorOutOfOrder {
                    group_id: group.id,
                    item_id: *id,
                });
            }
        }
    }

    let runs = live_groups
        .iter()
        .filter(|g| g.kind != GroupKind::Bisection && snapshot.live_run(g.id).is_some())
        .count();
    let ceiling = settings.window_ceiling.value;
    if runs > usize::try_from(ceiling).unwrap_or(usize::MAX) {
        violations.push(Violation::WindowExceeded { runs, ceiling });
    }

    let mut current = BTreeMap::<u64, usize>::new();
    for item in snapshot.items.values().filter(|i| i.is_current) {
        *current.entry(item.pr_number).or_default() += 1;
    }

    violations.extend(
        current
            .into_iter()
            .filter(|(_, n)| *n > 1)
            .map(|(pr_number, _)| Violation::SeveralCurrent { pr_number }),
    );

    violations
}

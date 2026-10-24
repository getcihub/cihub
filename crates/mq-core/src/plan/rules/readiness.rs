//! Rules on the readiness of the items and their admission to the queue.

use cihub_config::QueueSettings;
use cihub_domain::{
    AwaitedCheckStatus, BlockReason, EventKind, GroupState, Item, ItemId, ItemState, Mergeability,
    QueueMode, QueueState, Reason, ReviewDecision,
};
use time::OffsetDateTime;

use crate::Snapshot;
use crate::graph::{insertion_index, position_at, predecessors, renumber};
use crate::ids::IdGen;
use crate::snapshot::{PrObservation, Subject};

use crate::plan::Step;
use crate::plan::events::{ItemExtras, group_to, item_to_with};

/// A run stays valid only while the predecessors of its group and its base
/// do not change; otherwise the group restarts.
pub(in crate::plan) fn validity(
    s: &Snapshot,
    _: &QueueSettings,
    _: OffsetDateTime,
    _: &mut IdGen,
) -> Option<Step> {
    let order = s.queue_order();
    s.live_groups().find_map(|group| {
        let index = group
            .item_ids
            .iter()
            .find_map(|id| order.iter().position(|i| i.id == *id))?;

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
/// that the job that read the PR found unmet.
fn readiness_of(s: &Snapshot, item: &Item, pr: &PrObservation) -> Vec<BlockReason> {
    let mut blocks = mergeability_blocks(pr);

    let has_targets = s
        .targets
        .get(&item.pr_number)
        .is_some_and(|t| t.value.head_sha == item.head_sha);
    if s.mode == QueueMode::Parallel && !has_targets && blocks.is_empty() {
        blocks.push(BlockReason::Undetermined);
    }

    blocks.extend(unmet_conditions(pr));

    blocks
}

/// What keeps the PR out of its queue now, submitted or not: the reasons of
/// `readiness_of` that the PR alone gives.
#[must_use]
pub fn conditions_of(pr: &PrObservation) -> Vec<BlockReason> {
    let mut blocks = mergeability_blocks(pr);

    blocks.extend(unmet_conditions(pr));

    blocks
}

fn mergeability_blocks(pr: &PrObservation) -> Vec<BlockReason> {
    match pr.mergeability {
        Mergeability::Mergeable => Vec::new(),
        Mergeability::Behind => vec![BlockReason::BehindBase],
        Mergeability::Conflict => vec![BlockReason::MergeConflict],
        Mergeability::Blocked if pr.review_decision == ReviewDecision::ChangesRequested => {
            vec![BlockReason::ChangesRequested]
        }
        Mergeability::Blocked => blocking_checks(pr),
        Mergeability::Unknown => vec![BlockReason::Undetermined],
    }
}

fn unmet_conditions(pr: &PrObservation) -> Option<BlockReason> {
    (!pr.unmet_conditions.is_empty()).then(|| BlockReason::QueueConditionsUnmet {
        conditions: pr.unmet_conditions.clone(),
    })
}

/// The required checks that block a PR: failed, or not concluded yet; else
/// the SCM does not say why the PR is blocked.
fn blocking_checks(pr: &PrObservation) -> Vec<BlockReason> {
    let blocks: Vec<BlockReason> = pr
        .required_checks
        .iter()
        .filter_map(|name| {
            let report = pr.checks.iter().find(|c| c.name == *name);
            match report.and_then(|c| c.conclusion) {
                Some(conclusion) if conclusion.is_passing() => None,
                Some(_) => Some(BlockReason::FailingRequiredCheck {
                    check: name.clone(),
                }),
                None => Some(BlockReason::AwaitingRequiredCheck {
                    check: name.clone(),
                    status: AwaitedCheckStatus::of(report),
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

pub(in crate::plan) fn not_ready_items(s: &Snapshot) -> impl Iterator<Item = &Item> {
    s.items
        .values()
        .filter(|i| i.state == ItemState::NotReady && i.is_current)
}

pub(in crate::plan) fn readiness(
    s: &Snapshot,
    _: &QueueSettings,
    _: OffsetDateTime,
    _: &mut IdGen,
) -> Option<Step> {
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
pub(in crate::plan) fn admission(
    s: &Snapshot,
    _: &QueueSettings,
    _: OffsetDateTime,
    _: &mut IdGen,
) -> Option<Step> {
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

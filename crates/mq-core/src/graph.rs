//! Queue order and merge graph (`docs/ALGORITHMS.md`, « Ordre » and « Graphe
//! de merge »).

use cihub_domain::{ImpactedTargets, Item, ItemId, ItemState, Priority, QueueMode};

use crate::Snapshot;

/// Gap between two positions after a renumbering.
pub(crate) const POSITION_STEP: i64 = 1000;

static ALL: ImpactedTargets = ImpactedTargets::All;

/// The target set of an item: `All` in single mode, or when no upload
/// exists for its head.
pub(crate) fn targets<'a>(snapshot: &'a Snapshot, item: &Item) -> &'a ImpactedTargets {
    if snapshot.mode == QueueMode::Single {
        return &ALL;
    }
    snapshot
        .targets
        .get(&item.pr_number)
        .filter(|t| t.value.head_sha == item.head_sha)
        .map_or(&ALL, |t| &t.value.targets)
}

fn overlap(a: &ImpactedTargets, b: &ImpactedTargets) -> bool {
    match (a, b) {
        (ImpactedTargets::All, _) | (_, ImpactedTargets::All) => true,
        (ImpactedTargets::Targets(a), ImpactedTargets::Targets(b)) => !a.is_disjoint(b),
    }
}

/// The predecessors of `order[index]`, in queue order: the items ahead of it
/// that a chain of items ahead of it, with overlapping target sets, links to
/// it.
pub(crate) fn predecessors(snapshot: &Snapshot, order: &[&Item], index: usize) -> Vec<ItemId> {
    let ahead = &order[..index];
    let sets: Vec<_> = order[..=index]
        .iter()
        .map(|i| targets(snapshot, i))
        .collect();
    let mut linked = vec![false; index + 1];
    linked[index] = true;
    let mut frontier = vec![index];
    while let Some(k) = frontier.pop() {
        for j in 0..index {
            if !linked[j] && overlap(sets[j], sets[k]) {
                linked[j] = true;
                frontier.push(j);
            }
        }
    }
    ahead
        .iter()
        .zip(linked)
        .filter_map(|(i, l)| l.then_some(i.id))
        .collect()
}

/// Where a new item with `priority` enters `order`. `urgent` goes in front
/// of every item that is not `urgent`, groups in test included; any other
/// value goes in front of the first item that is not in test and has a
/// higher value.
pub(crate) fn insertion_index(order: &[&Item], priority: Priority) -> usize {
    order
        .iter()
        .position(|i| {
            if priority == Priority::URGENT {
                i.priority > Priority::URGENT
            } else {
                i.state == ItemState::Pending && i.priority > priority
            }
        })
        .unwrap_or(order.len())
}

/// The position for a new item at `index` in `order`, or `None` when the
/// interval is exhausted and the queue must be renumbered first.
pub(crate) fn position_at(order: &[&Item], index: usize) -> Option<i64> {
    let before = index.checked_sub(1).and_then(|k| order[k].position);
    let after = order.get(index).and_then(|i| i.position);
    match (before, after) {
        (None, None) => Some(POSITION_STEP),
        (Some(b), None) => b.checked_add(POSITION_STEP),
        (None, Some(a)) => a.checked_sub(POSITION_STEP),
        (Some(b), Some(a)) => (a - b >= 2).then(|| b + (a - b) / 2),
    }
}

/// New positions for `order` that leave a free step at `index`.
pub(crate) fn renumber(order: &[&Item], index: usize) -> Vec<(ItemId, i64)> {
    order
        .iter()
        .zip(1_i64..)
        .map(|(item, k)| {
            let slot = if k > i64::try_from(index).unwrap_or(i64::MAX) {
                k + 1
            } else {
                k
            };
            (item.id, slot * POSITION_STEP)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use cihub_domain::{ItemId, ItemState, Mergeability, OrgId, QueueId, Reason, SubmittedVia};
    use ulid::Ulid;

    use super::*;

    fn item(n: u128, position: Option<i64>, state: ItemState, priority: Priority) -> Item {
        Item {
            id: ItemId::from_ulid(Ulid(n)),
            org_id: OrgId::from_ulid(Ulid(1)),
            queue_id: QueueId::from_ulid(Ulid(2)),
            pr_number: u64::try_from(n).unwrap(),
            head_sha: format!("head-{n}"),
            state,
            state_reason: Reason::Submitted,
            priority,
            no_batch: false,
            force_enqueued: false,
            submitted_via: SubmittedVia::Api,
            position,
            is_current: true,
            mergeability: Mergeability::Mergeable,
            block_reasons: Vec::new(),
            group_id: None,
            verified_by_run_id: None,
            merged_sha: None,
            submitted_at: time::OffsetDateTime::UNIX_EPOCH,
            direct_merge_tried: false,
        }
    }

    #[test]
    fn positions_take_the_middle_or_ask_for_a_renumbering() {
        let a = item(1, Some(1000), ItemState::Testing, Priority::MEDIUM);
        let b = item(2, Some(1001), ItemState::Pending, Priority::MEDIUM);
        let c = item(3, Some(3000), ItemState::Pending, Priority::MEDIUM);

        let order = [&a, &b, &c];

        assert_eq!(position_at(&[], 0), Some(POSITION_STEP));
        assert_eq!(position_at(&order, 0), Some(0));
        assert_eq!(position_at(&order, 3), Some(4000));
        assert_eq!(position_at(&order, 2), Some(2000));
        assert_eq!(position_at(&order, 1), None);

        let positions = renumber(&order, 1);
        assert_eq!(
            positions.iter().map(|p| p.1).collect::<Vec<_>>(),
            vec![1000, 3000, 4000]
        );
    }

    #[test]
    fn priorities_choose_the_insertion_point() {
        let a = item(1, Some(1000), ItemState::Testing, Priority::LOW);
        let b = item(2, Some(2000), ItemState::Pending, Priority::HIGH);
        let c = item(3, Some(3000), ItemState::Pending, Priority::MEDIUM);

        let order = [&a, &b, &c];

        assert_eq!(insertion_index(&order, Priority::URGENT), 0);
        assert_eq!(insertion_index(&order, Priority::HIGH), 2);
        assert_eq!(insertion_index(&order, Priority::MEDIUM), 3);
        assert_eq!(insertion_index(&order, Priority(1)), 1);
    }
}

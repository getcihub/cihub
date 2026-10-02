//! What a PR shows of its item (`DECISIONS.md` D156): the desired state of its
//! status, its comment and its labels, in the terms of the queue. The text
//! comes from the caller.

use std::collections::BTreeSet;

use cihub_domain::{BlockReason, Item, ItemId, ItemState, Reason};

use crate::Snapshot;

/// The state of one item, as its PR shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Surface {
    pub item_id: ItemId,
    pub pr_number: u64,
    /// The head that the item tests: the status goes there.
    pub head_sha: String,
    pub state: ItemState,
    pub reason: Reason,
    /// The rank of the item in the queue, from 1; `None` out of the queue.
    pub position: Option<usize>,
    /// The PRs that the test of the item merges before it.
    pub predecessors: Vec<u64>,
    /// The test branch of the last run of its group.
    pub test_branch: Option<String>,
    /// The required statuses that failed the last run of its group.
    pub failing_checks: Vec<String>,
    pub block_reasons: Vec<BlockReason>,
    /// The labels that the PR carries, as last observed.
    pub labels: BTreeSet<String>,
}

/// The surface of `item` in `s`.
pub(crate) fn surface(s: &Snapshot, item: &Item) -> Surface {
    let position = s
        .queue_order()
        .iter()
        .position(|i| i.id == item.id)
        .map(|rank| rank + 1);
    let group = item.group_id.and_then(|id| s.groups.get(&id));
    let run = item.group_id.and_then(|id| s.latest_run(id));
    let predecessors = group
        .map(|g| {
            g.predecessor_ids
                .iter()
                .filter_map(|id| s.items.get(id))
                .map(|i| i.pr_number)
                .collect()
        })
        .unwrap_or_default();
    Surface {
        item_id: item.id,
        pr_number: item.pr_number,
        head_sha: item.head_sha.clone(),
        state: item.state,
        reason: item.state_reason,
        position,
        predecessors,
        test_branch: run.map(|r| r.test_branch.clone()),
        failing_checks: run
            .map(|r| r.failure_causing_statuses.clone())
            .unwrap_or_default(),
        block_reasons: item.block_reasons.clone(),
        labels: s
            .prs
            .get(&item.pr_number)
            .map(|pr| pr.value.labels.clone())
            .unwrap_or_default(),
    }
}

//! What a PR shows of its item (`DECISIONS.md` D156): the desired state of its
//! status, its comment and its labels, in the terms of the queue. The text
//! comes from the caller.

use std::collections::BTreeSet;

use cihub_domain::{BlockReason, Item, ItemState, Reason};
use time::OffsetDateTime;

use crate::Snapshot;

/// The state of one item, as its PR shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Surface {
    pub pr_number: u64,
    /// The head that the item tests: the status goes there.
    pub head_sha: String,
    pub state: ItemState,
    pub reason: Reason,
    /// The rank of the item in the queue, from 1; `None` out of the queue.
    pub position: Option<usize>,
    /// The PRs that the test of the item merges before it.
    pub predecessors: Vec<u64>,
    /// The commit that the last run of its group tests, once assembled.
    pub test_sha: Option<String>,
    /// The draft PR of the last run of its group, in draft PR mode.
    pub draft_pr_number: Option<u64>,
    /// The required statuses that failed the last run of its group.
    pub failing_checks: Vec<String>,
    pub block_reasons: Vec<BlockReason>,
    /// The labels that the PR carries, as last observed.
    pub labels: BTreeSet<String>,
    /// The batch that waits for more PRs, for its members only
    /// (`DECISIONS.md` D241).
    pub batch_wait: Option<BatchWait>,
}

/// A batch that is not full and waits for more PRs (`DECISIONS.md` D210).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BatchWait {
    /// The number of its members now.
    pub members: usize,
    /// When it stops waiting for more PRs: its oldest member has then waited
    /// `batching.max_wait_minutes`. Not when its test starts: a closed batch
    /// still waits for room in the window.
    pub closes_at: OffsetDateTime,
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
        pr_number: item.pr_number,
        head_sha: item.head_sha.clone(),
        state: item.state,
        reason: item.state_reason,
        position,
        predecessors,
        test_sha: run.and_then(|r| r.test_sha.clone()),
        draft_pr_number: run.and_then(|r| r.draft_pr_number),
        failing_checks: run
            .map(|r| r.failure_causing_statuses.clone())
            .unwrap_or_default(),
        block_reasons: item.block_reasons.clone(),
        labels: s
            .prs
            .get(&item.pr_number)
            .map(|pr| pr.value.labels.clone())
            .unwrap_or_default(),
        batch_wait: None,
    }
}

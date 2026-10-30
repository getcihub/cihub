use serde::{Deserialize, Serialize};

use crate::{GroupId, ItemId, ItemState, OrgId, QueueId, Reason};

/// The unit that is tested and held in `pending_failure`: one PR or a batch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestGroup {
    /// The group.
    pub id: GroupId,
    /// The organization of the queue.
    pub org_id: OrgId,
    /// The queue that formed the group.
    pub queue_id: QueueId,
    /// One PR, a batch, or a half of a failed batch.
    pub kind: GroupKind,
    /// Where the group is in its life.
    pub state: GroupState,
    /// Reason of the last transition; the members receive the same one.
    pub state_reason: Reason,
    /// Members in queue order; each one merges as its own commit.
    pub item_ids: Vec<ItemId>,
    /// The failed batch that a bisection half comes from.
    pub parent_group_id: Option<GroupId>,
    /// Items the group is tested on top of, in queue order. A run stays valid
    /// only while this set and `base_sha` do not change.
    pub predecessor_ids: Vec<ItemId>,
    /// The head of the target branch that the test branch starts from.
    pub base_sha: String,
}

closed_enum! {
    /// What a test group holds.
    pub enum GroupKind {
        /// One PR.
        Single,
        /// Several PRs tested together.
        Batch,
        /// A half of a failed batch, tested to find the PR at fault.
        Bisection,
    }
}

closed_enum! {
    /// State of a test group. A batch never reaches `failed`: it is bisected.
    pub enum GroupState {
        /// A run of the group is in progress.
        Testing,
        /// The run failed; the queue waits to know who is at fault.
        PendingFailure,
        /// A run proved the group; its members merge in order.
        TestsPassed,
        /// Every member merged.
        Merged,
        /// The group is at fault.
        Failed,
        /// The bisection of the group ended: its members left to its halves.
        Bisected,
        /// The queue gave up the group: a member left or a predecessor changed.
        Dissolved,
    }
}

impl GroupState {
    /// Whether the group can change state no more.
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Merged | Self::Failed | Self::Bisected | Self::Dissolved
        )
    }

    /// The state that the members of a group in this state take. `None` for
    /// a merged group: its members merged one by one before it.
    pub const fn member_state(self) -> Option<ItemState> {
        match self {
            Self::Testing => Some(ItemState::Testing),
            Self::PendingFailure => Some(ItemState::PendingFailure),
            Self::TestsPassed => Some(ItemState::TestsPassed),
            Self::Failed => Some(ItemState::Failed),
            Self::Bisected | Self::Dissolved => Some(ItemState::Pending),
            Self::Merged => None,
        }
    }
}

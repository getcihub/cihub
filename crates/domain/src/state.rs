closed_enum! {
    /// Public state of a queue item.
    pub enum ItemState {
        NotReady,
        Pending,
        Testing,
        TestsPassed,
        PendingFailure,
        Merged,
        Failed,
        Cancelled,
    }
}

impl ItemState {
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Merged | Self::Failed | Self::Cancelled)
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

closed_enum! {
    pub enum QueueState {
        /// Accepts and merges.
        Running,
        /// Accepts nothing, merges nothing, keeps the order.
        Paused,
        /// Accepts nothing, lets in-flight items merge.
        Draining,
        /// Entered when the mode changes with items in flight; behaves as
        /// `Draining` until the queue is empty.
        SwitchingModes,
    }
}

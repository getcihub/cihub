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

//! Closed value sets of queue settings and records. Defaults and ranges are
//! validated in `cihub-config`, not here.

closed_enum! {
    pub enum Strategy {
        Sequential,
        Pipelined,
        Batched,
        Parallel,
    }
}

closed_enum! {
    pub enum QueueMode {
        Single,
        Parallel,
    }
}

closed_enum! {
    pub enum MergeMethod {
        Squash,
        MergeCommit,
        Rebase,
        FastForward,
    }
}

closed_enum! {
    pub enum Trigger {
        DraftPr,
        Push,
        Inplace,
    }
}

closed_enum! {
    pub enum DirectMergeMode {
        Off,
        Always,
    }
}

closed_enum! {
    pub enum OptimizationMode {
        Off,
        BisectionSkipRedundantTests,
    }
}

closed_enum! {
    pub enum GroupKind {
        Single,
        Batch,
        Bisection,
    }
}

closed_enum! {
    pub enum RunStatus {
        Assembling,
        InProgress,
        Succeeded,
        Failed,
        /// The queue replaced the run: the group was given up or proved by a
        /// successor.
        Interrupted,
        /// A member of the group was cancelled.
        Cancelled,
        TimedOut,
    }
}

impl RunStatus {
    pub const fn is_terminal(self) -> bool {
        !matches!(self, Self::Assembling | Self::InProgress)
    }
}

closed_enum! {
    /// Kind of a report on a commit. Two reports of different kinds can have
    /// the same name.
    pub enum CheckKind {
        /// A check with a run: a GitHub check run, a GitLab pipeline job.
        Check,
        /// A commit status.
        Status,
    }
}

closed_enum! {
    /// Progress of a check or a commit status.
    pub enum CheckStatus {
        Queued,
        InProgress,
        Completed,
    }
}

closed_enum! {
    /// Terminal outcome of a check or a commit status.
    pub enum CheckConclusion {
        Success,
        Neutral,
        Skipped,
        Failure,
        Error,
        Cancelled,
        TimedOut,
        ActionRequired,
        Stale,
    }
}

impl CheckConclusion {
    /// A required check passes on `success`, `neutral` or `skipped`; any other
    /// conclusion fails the run.
    pub const fn is_passing(self) -> bool {
        matches!(self, Self::Success | Self::Neutral | Self::Skipped)
    }
}

closed_enum! {
    pub enum SubmittedVia {
        Comment,
        Label,
        Api,
    }
}

closed_enum! {
    pub enum MergeCommitTitle {
        PrTitle,
        PrTitleAndNumber,
    }
}

closed_enum! {
    /// How `inplace` updates the PR branch on the target.
    pub enum UpdateMethod {
        Merge,
    }
}

closed_enum! {
    /// Condition sets that receive the branch protection rules.
    pub enum BranchProtectionInjection {
        Both,
        Merge,
        None,
    }
}

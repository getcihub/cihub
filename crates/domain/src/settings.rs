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
        Interrupted,
        Cancelled,
        TimedOut,
    }
}

closed_enum! {
    pub enum CheckKind {
        CheckRun,
        Status,
    }
}

closed_enum! {
    /// Terminal outcome of a check run or commit status.
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
        Ui,
        Cli,
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
        Rebase,
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

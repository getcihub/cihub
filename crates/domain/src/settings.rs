//! Closed value sets of the queue settings. Defaults and ranges are
//! validated in `cihub-config`, not here.

closed_enum! {
    /// The first setting of a queue: a table of default values for the
    /// others (`specs/CONFIGURATION.md`).
    pub enum Strategy {
        /// One PR tested at a time.
        Sequential,
        /// PRs tested ahead of the merges, each on top of those before it.
        Pipelined,
        /// As `pipelined`, with batches of PRs.
        Batched,
        /// Batches in independent lines by impacted targets.
        Parallel,
    }
}

closed_enum! {
    /// How a queue orders its PRs.
    pub enum QueueMode {
        /// Every PR in one line.
        Single,
        /// A line per set of impacted targets: PRs that change different
        /// targets do not wait for each other.
        Parallel,
    }
}

closed_enum! {
    /// How the queue lands a PR on the target branch.
    pub enum MergeMethod {
        /// One commit with the changes of the PR.
        Squash,
        /// A merge commit with the PR as its second parent.
        MergeCommit,
        /// The commits of the PR, replayed on the target.
        Rebase,
        /// The target moves to the tested commit, with no new commit
        /// (`sequential` strategy only).
        FastForward,
    }
}

closed_enum! {
    /// How the CI of the user learns that a test branch exists.
    pub enum Trigger {
        /// A draft PR from the test branch, for a CI that runs on PRs.
        DraftPr,
        /// The push of the test branch, for a CI that runs on pushes.
        Push,
        /// No test branch: the PR branch itself is updated onto the target
        /// (a later lot; refused until then).
        Inplace,
    }
}

closed_enum! {
    /// Whether the queue merges a PR at admission without a test branch, when
    /// its head already contains the target and its checks passed.
    pub enum DirectMergeMode {
        /// Every PR goes through a test branch.
        Off,
        /// A PR that meets the conditions merges directly.
        Always,
    }
}

closed_enum! {
    /// The optimizations of the tests that the queue may apply.
    pub enum OptimizationMode {
        /// No optimization: a half that waits goes back to the queue and is
        /// tested as a batch.
        Off,
        /// When one half of a bisection passes, the other half holds the
        /// failure: it is split again without a run of its own.
        BisectionSkipRedundantTests,
    }
}

closed_enum! {
    /// The title of the commit that a merge creates.
    pub enum MergeCommitTitle {
        /// The title of the PR.
        PrTitle,
        /// The title of the PR, then its number.
        PrTitleAndNumber,
    }
}

closed_enum! {
    /// How `inplace` updates the PR branch on the target.
    pub enum UpdateMethod {
        /// A merge of the target into the PR branch.
        Merge,
    }
}

closed_enum! {
    /// Condition sets that receive the branch protection rules.
    pub enum BranchProtectionInjection {
        /// The checks that the rules require go to the conditions of the PR
        /// and to the required statuses of the test commit.
        Both,
        /// The checks go to the required statuses of the test commit only.
        Merge,
        /// The rules of the branch add no check.
        None,
    }
}

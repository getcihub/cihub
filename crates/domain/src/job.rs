closed_enum! {
    /// What a job reports when it fails. A job never reports a reason: `plan`
    /// turns the error into one.
    pub enum JobError {
        /// Assembly: the job received an event it cannot use.
        InvalidEvent,
        /// Assembly: a merge into the test branch returned 409.
        MergeConflict,
        /// Assembly: the target branch has no configuration file.
        ConfigNotFound,
        /// Assembly: the configuration file is not valid.
        ConfigParsingFailure,
        /// Assembly: the configuration file has an unknown `version`.
        ConfigBadVersion,
        /// Assembly: the configuration file gives no required statuses.
        ConfigMissingRequiredStatuses,
        /// Assembly: the required statuses of the configuration are not valid.
        ConfigBadRequiredStatuses,
        /// Assembly: no source gives a required status.
        NoRequiredStatuses,
        /// Assembly: every retry failed.
        RetryExhausted,
        /// Assembly: the repository does not allow draft PRs.
        DraftPrsNotAllowed,
        /// Assembly: branch protection refused the test branch.
        PermissionsBlockTestBranch,
        /// Merge: the SCM refused the merge.
        MergeRejected,
        /// Merge: the rate limit did not reset in time.
        RateLimitExceeded,
        /// Merge: branch protection refused the merge.
        BranchProtectionUnmet,
        /// Merge: the SCM kept a merge of the PR in progress.
        MergeAlreadyInProgress,
        /// Merge: server errors on every retry.
        RepeatedServerErrors,
        /// Merge: the head of the PR changed since the test (the `sha` guard).
        HeadChanged,
        /// Merge: the PR was already merged when the job read it; the queue
        /// did not merge it (`AGENTS.md` D132).
        PrAlreadyMerged,
        /// The direct merge cannot happen now: a required status is not green
        /// on the head of the PR, the PR does not contain the head of the
        /// target, or it is not mergeable (`AGENTS.md` D153).
        DirectMergeUnavailable,
    }
}

closed_enum! {
    /// The priority of a job on the budget of its connection, highest first
    /// (`AGENTS.md` D59). When the budget runs low, `speculate` waits first and
    /// `merge` last.
    pub enum JobClass {
        /// A reply that a person waits for: a command, a status, a comment.
        Interactive,
        /// A merge of a PR that passed its tests.
        Merge,
        /// A test branch built ahead of the merges.
        Speculate,
        /// A read of the repair loop.
        Repair,
    }
}

closed_enum! {
    /// Where a job is in its life.
    pub enum JobState {
        /// Waits for its `run_after`, then for a worker.
        Queued,
        /// A worker holds it until `locked_until`; past that time, another
        /// worker takes it back.
        Running,
        /// Its executor finished it.
        Done,
        /// Its executor reported an error that a retry cannot fix.
        Failed,
        /// Every retry failed: it waits for an operator.
        Dead,
    }
}

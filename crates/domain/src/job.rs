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
    }
}

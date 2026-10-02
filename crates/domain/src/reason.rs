//! Closed vocabulary of transition reasons: one enum for items, groups, runs
//! and queues. The tables of `transition` say which reason goes with which
//! state change.

closed_enum! {
    /// Why an item, a group, a run or a queue changed state. The `failed`
    /// reasons carry the prefix of their family.
    pub enum Reason {
        /// The PR was submitted to the queue.
        Submitted,
        /// The item became mergeable and has its impacted targets.
        MergeItemNowReady,
        /// A test run started for the group of the item.
        TestRunStarted,
        /// The failure budget restarts the run of a group in failure.
        FailureBudgetRetry,
        /// A push outside the queue changed the target branch.
        ManualPushToTargetBranch,
        /// A predecessor failed; the item restarts without it.
        PrerequisiteMergeItemFailed,
        /// A predecessor was cancelled; the item restarts without it.
        PrerequisiteMergeItemCancelled,
        /// A predecessor was merged outside the queue.
        PrerequisiteMergeItemManuallyMerged,
        /// A user asked to restart the tests.
        TestsRestartedByUser,
        /// An API request asked to restart the tests.
        TestsRestartedByApiRequest,
        /// An `urgent` item went in front of the group.
        SkipTheLineInterrupted,
        /// The failed batch is split in two; this half is tested again.
        BisectionRequired,
        /// The tested half of a bisection passed; the other half is tested.
        BisectionTestRunPassed,
        /// Every required status passed on the test SHA.
        TestRunPassed,
        /// A run of a successor passed and releases the group in failure.
        SuccessorTestRunPassed,
        /// A run of a successor passed and proves the group (optimistic merge).
        PassedByOptimization,
        /// The queue merged the PR after its run passed.
        MergeGraphMerge,
        /// The queue merged the PR at admission, without a test branch.
        MergeGraphDirectMerge,
        /// The PR was merged outside the queue.
        ManualMerge,
        /// A user cancelled the item.
        UserRequested,
        /// An administrator cancelled the item or changed the queue state.
        AdminRequested,
        /// The PR was closed.
        PrClosed,
        /// The head of the PR changed.
        PrPushedTo,
        /// The base branch of the PR changed.
        PrBaseBranchChanged,
        /// The PR became a draft.
        PrConvertedToDraft,
        /// The label that submitted the PR was removed.
        LabelRemoved,
        /// The item stayed `not_ready` longer than `not_ready_timeout_hours`.
        NotReadyTimeoutExpired,
        /// The assembly received an event it cannot use.
        StartTestRunInvalidEvent,
        /// A merge into the test branch returned 409.
        StartTestRunMergeConflict,
        /// The target branch has no configuration file.
        StartTestRunConfigNotFound,
        /// The configuration file is not valid.
        StartTestRunConfigParsingFailure,
        /// The configuration file has an unknown `version`.
        StartTestRunConfigBadVersion,
        /// The configuration file gives no required statuses.
        StartTestRunConfigMissingRequiredStatuses,
        /// The required statuses of the configuration file are not valid.
        StartTestRunConfigBadRequiredStatuses,
        /// No source gives a required status.
        StartTestRunNoRequiredStatuses,
        /// The assembly failed after all its retries.
        StartTestRunRetryExhausted,
        /// The repository does not allow draft PRs.
        StartTestRunDraftPrsNotAllowed,
        /// Branch protection refused the test branch.
        StartTestRunPermissionsBlockTestBranch,
        /// A required status failed on the test SHA.
        TestRunRequiredStatusFailed,
        /// The run lasted longer than `testing_timeout_minutes`.
        TestRunTimeout,
        /// A bisection proved the failure without a run.
        TestRunFailedByOptimization,
        /// The SCM refused the merge of the PR.
        MergeRejected,
        /// The SCM refused the direct merge of the PR.
        DirectMergeRejected,
        /// The rate limit did not reset in time for the merge.
        MergeRateLimitExceeded,
        /// Branch protection refused the merge.
        MergeBranchProtectionUnmet,
        /// The SCM kept a merge of the PR in progress.
        MergeAlreadyInProgress,
        /// The SCM returned server errors on every retry of the merge.
        MergeRepeatedServerErrors,
        /// The head of the PR changed between the test and the merge.
        PrUpdatedAtMergeTime,
        /// Assembly created the test commit.
        TestBranchAssembled,
        /// CIHub lost its access to the repository.
        AccessLost,
        /// CIHub has its access to the repository again.
        AccessRestored,
        /// The mode of the queue changed with items in flight.
        ModeChanged,
        /// The queue is empty after a mode change.
        ModeSwitchCompleted,
        /// A submission came while the queue drains: it accepts nothing new
        /// (`AGENTS.md` D151).
        QueueDraining,
        /// A submission came while the queue switches its mode (D151).
        QueueSwitchingModes,
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use strum::IntoEnumIterator;

    use super::*;

    #[test]
    fn serde_name_matches_as_str_and_from_str() {
        for reason in Reason::iter() {
            let json = serde_json::to_string(&reason).unwrap();
            assert_eq!(json, format!("\"{}\"", reason.as_str()));
            assert_eq!(Reason::from_str(reason.as_str()), Ok(reason));
        }
        assert_eq!(Reason::MergeRejected.as_str(), "merge_rejected");
    }
}

//! Closed vocabulary of transition reasons: one enum for items, groups, runs
//! and queues. The tables of `transition` say which reason goes with which
//! state change.

closed_enum! {
    /// Why an item, a group, a run or a queue changed state. The `failed`
    /// reasons carry the prefix of their family.
    pub enum Reason {
        /// The PR was submitted to the queue.
        Submitted,
        /// The item became mergeable and has its affected targets.
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
        /// Another member of the batch left the queue; the run tested its code.
        BatchMemberLeft,
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
        /// (`DECISIONS.md` D151).
        QueueDraining,
        /// A submission came while the queue switches its mode (D151).
        QueueSwitchingModes,
    }
}

impl Reason {
    /// The sentence that a person reads for this reason: the PR comment and
    /// status, the API (`state_description`) and the CLI. In English, without
    /// a capital or a final period, to follow `Failed: ` or `Cancelled: `. A
    /// free rendering: the stable contract is `as_str` (`DECISIONS.md` D167).
    pub fn describe(self) -> &'static str {
        match self {
            Self::Submitted => "the PR was submitted to the queue",
            Self::MergeItemNowReady => "the PR is ready to be tested",
            Self::TestRunStarted => "a test run started",
            Self::FailureBudgetRetry => "the tests failed and run again within the failure budget",
            Self::ManualPushToTargetBranch => "a push outside the queue changed the target branch",
            Self::PrerequisiteMergeItemFailed => "a PR ahead in the queue failed",
            Self::PrerequisiteMergeItemCancelled => "a PR ahead in the queue was cancelled",
            Self::PrerequisiteMergeItemManuallyMerged => {
                "a PR ahead in the queue was merged outside the queue"
            }
            Self::BatchMemberLeft => "a PR of the same batch left the queue",
            Self::TestsRestartedByUser => "a user restarted the tests",
            Self::TestsRestartedByApiRequest => "an API request restarted the tests",
            Self::SkipTheLineInterrupted => "an urgent PR went to the front of the queue",
            Self::BisectionRequired => "the batch failed and is split to find the cause",
            Self::BisectionTestRunPassed => "the tested half of the batch passed",
            Self::TestRunPassed => "every required check passed",
            Self::SuccessorTestRunPassed => "the tests of a PR behind it passed",
            Self::PassedByOptimization => "the tests of a PR behind it passed and include this PR",
            Self::MergeGraphMerge => "the queue merged the PR after its tests passed",
            Self::MergeGraphDirectMerge => "the queue merged the PR without a test branch",
            Self::ManualMerge => "the PR was merged outside the queue",
            Self::UserRequested => "a user asked for it",
            Self::AdminRequested => "an administrator asked for it",
            Self::PrClosed => "the PR was closed",
            Self::PrPushedTo => "the PR received a new commit after its submission",
            Self::PrBaseBranchChanged => "the base branch of the PR changed",
            Self::PrConvertedToDraft => "the PR became a draft",
            Self::LabelRemoved => "the label that submitted the PR was removed",
            Self::NotReadyTimeoutExpired => "the PR waited too long to become ready",
            Self::StartTestRunInvalidEvent => "the test branch received an event it cannot use",
            Self::StartTestRunMergeConflict => {
                "the PR conflicts with the target branch or with a PR ahead of it"
            }
            Self::StartTestRunConfigParsingFailure => "the configuration file is not valid",
            Self::StartTestRunConfigBadVersion => "the configuration file has an unknown version",
            Self::StartTestRunConfigMissingRequiredStatuses => {
                "the configuration file gives no required checks"
            }
            Self::StartTestRunConfigBadRequiredStatuses => {
                "the required checks of the configuration file are not valid"
            }
            Self::StartTestRunNoRequiredStatuses => "no required check is configured",
            Self::StartTestRunRetryExhausted => {
                "the test branch could not be built after all retries"
            }
            Self::StartTestRunDraftPrsNotAllowed => "the repository does not allow draft PRs",
            Self::StartTestRunPermissionsBlockTestBranch => {
                "branch protection refused the test branch"
            }
            Self::TestRunRequiredStatusFailed => "a required check failed",
            Self::TestRunTimeout => "the tests took longer than the time limit",
            Self::MergeRejected => "the merge of the PR was refused",
            Self::DirectMergeRejected => "the direct merge of the PR was refused",
            Self::MergeRateLimitExceeded => {
                "the API rate limit did not reset in time for the merge"
            }
            Self::MergeBranchProtectionUnmet => "branch protection refused the merge",
            Self::MergeAlreadyInProgress => "another merge of the PR is still in progress",
            Self::MergeRepeatedServerErrors => "the merge failed with server errors on every retry",
            Self::PrUpdatedAtMergeTime => "the PR changed between its tests and its merge",
            Self::TestBranchAssembled => "the test branch was built",
            Self::AccessLost => "CIHub lost its access to the repository",
            Self::AccessRestored => "CIHub has its access to the repository again",
            Self::ModeChanged => "the queue mode changed",
            Self::ModeSwitchCompleted => "the queue mode change is complete",
            Self::QueueDraining => "the queue is draining and accepts no new PR",
            Self::QueueSwitchingModes => "the queue is changing its mode and accepts no new PR",
        }
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

    #[test]
    fn each_reason_has_its_own_short_sentence() {
        let mut seen = std::collections::BTreeSet::new();
        for reason in Reason::iter() {
            let text = reason.describe();
            assert!(!text.is_empty() && text.len() <= 100, "{reason}: {text}");
            assert!(!text.ends_with('.'), "{reason}: {text}");
            assert!(seen.insert(text), "{reason}: {text} is used twice");
        }
        assert_eq!(
            Reason::PrPushedTo.describe(),
            "the PR received a new commit after its submission"
        );
    }
}

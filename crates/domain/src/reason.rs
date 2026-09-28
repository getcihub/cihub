//! Closed vocabulary of transition reasons, one enum per target state, so
//! that a reason can only go with the state it explains.

closed_enum! {
    pub enum PendingReason {
        MergeItemNowReady,
        ManualPushToTargetBranch,
        PrerequisiteMergeItemFailed,
        PrerequisiteMergeItemCancelled,
        PrerequisiteMergeItemManuallyMerged,
        TestsRestartedByUser,
        TestsRestartedByApiRequest,
        SkipTheLineInterrupted,
        BisectionRequired,
        BisectionTestRunPassed,
    }
}

closed_enum! {
    pub enum TestsPassedReason {
        TestRunPassed,
        SuccessorTestRunPassed,
        PassedByOptimization,
    }
}

closed_enum! {
    pub enum MergedReason {
        MergeGraphMerge,
        MergeGraphDirectMerge,
        /// The PR was merged outside the queue.
        ManualMerge,
    }
}

closed_enum! {
    pub enum CancelledReason {
        UserRequested,
        AdminRequested,
        PrClosed,
        PrPushedTo,
        PrBaseBranchChanged,
        PrConvertedToDraft,
        LabelRemoved,
        NotReadyTimeoutExpired,
    }
}

closed_enum! {
    pub enum FailedReason {
        StartTestRunInvalidEvent,
        StartTestRunMergeConflict,
        StartTestRunConfigNotFound,
        StartTestRunConfigParsingFailure,
        StartTestRunConfigBadVersion,
        StartTestRunConfigMissingRequiredStatuses,
        StartTestRunConfigBadRequiredStatuses,
        StartTestRunNoRequiredStatuses,
        StartTestRunRetryExhausted,
        StartTestRunDraftPrsNotAllowed,
        StartTestRunPermissionsBlockTestBranch,
        TestRunRequiredStatusFailed,
        TestRunTimeout,
        TestRunFailedByOptimization,
        GithubApiMergePrFailed,
        GithubApiDirectMergePrFailed,
        GithubApiRateLimitExceeded,
        GithubApiBranchProtectionUnmet,
        GithubApiMergeAlreadyInProgress,
        GithubApiRepeatedServerErrors,
        PrUpdatedAtMergeTime,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::str::FromStr;

    use strum::IntoEnumIterator;

    use super::*;

    fn names<E: IntoEnumIterator + Into<&'static str>>() -> Vec<&'static str> {
        E::iter().map(Into::into).collect()
    }

    #[test]
    fn reason_names_are_unique_across_states() {
        // `events.reason` is one column for all target states.
        let all: Vec<_> = [
            names::<PendingReason>(),
            names::<TestsPassedReason>(),
            names::<MergedReason>(),
            names::<CancelledReason>(),
            names::<FailedReason>(),
        ]
        .concat();
        let unique: HashSet<_> = all.iter().collect();
        assert_eq!(unique.len(), all.len());
    }

    #[test]
    fn serde_name_matches_as_str_and_from_str() {
        for reason in FailedReason::iter() {
            let json = serde_json::to_string(&reason).unwrap();
            assert_eq!(json, format!("\"{}\"", reason.as_str()));
            assert_eq!(FailedReason::from_str(reason.as_str()), Ok(reason));
        }
        assert_eq!(
            FailedReason::GithubApiMergePrFailed.as_str(),
            "github_api_merge_pr_failed"
        );
    }
}

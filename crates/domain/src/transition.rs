//! Tables of the permitted transitions (`AGENTS.md` D6): for each (state
//! before, state after), the reasons that may explain it. `None` before is a
//! creation.

use std::fmt;

use crate::{GroupState, ItemState, QueueState, Reason, RunStatus};

/// A state machine whose transitions carry a `Reason`.
pub trait Lifecycle: Copy + Eq + fmt::Display + 'static {
    /// The reasons that permit `from` → `to`; empty when the transition does
    /// not exist.
    fn allowed_reasons(from: Option<Self>, to: Self) -> &'static [Reason];
}

/// A transition that the table does not permit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransitionError<S> {
    pub from: Option<S>,
    pub to: S,
    pub reason: Reason,
}

impl<S: fmt::Display> fmt::Display for TransitionError<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.from {
            Some(from) => write!(f, "transition {from} -> {}", self.to)?,
            None => write!(f, "creation in {}", self.to)?,
        }
        write!(f, " ({}) is not allowed", self.reason)
    }
}

impl<S: fmt::Debug + fmt::Display> std::error::Error for TransitionError<S> {}

/// Checks `from` → `to` with `reason` against the table of `S`.
///
/// # Errors
///
/// The table does not permit this reason for this transition.
pub fn check_transition<S: Lifecycle>(
    from: Option<S>,
    to: S,
    reason: Reason,
) -> Result<(), TransitionError<S>> {
    if S::allowed_reasons(from, to).contains(&reason) {
        Ok(())
    } else {
        Err(TransitionError { from, to, reason })
    }
}

/// Concatenates reason families at compile time; `N` must be the total length.
const fn concat<const N: usize>(parts: &[&[Reason]]) -> [Reason; N] {
    let mut out = [Reason::Submitted; N];
    let mut n = 0;
    let mut i = 0;
    while i < parts.len() {
        let mut j = 0;
        while j < parts[i].len() {
            out[n] = parts[i][j];
            n += 1;
            j += 1;
        }
        i += 1;
    }
    assert!(n == N, "N is not the total length of the parts");
    out
}

use Reason as R;

/// Cancellation of an item that is in the queue.
const CANCELLED: [Reason; 7] = [
    R::UserRequested,
    R::AdminRequested,
    R::PrClosed,
    R::PrPushedTo,
    R::PrBaseBranchChanged,
    R::PrConvertedToDraft,
    R::LabelRemoved,
];
const NOT_READY_CANCELLED: [Reason; 8] = concat(&[&[R::NotReadyTimeoutExpired], &CANCELLED]);
const START_TEST_RUN: [Reason; 11] = [
    R::StartTestRunInvalidEvent,
    R::StartTestRunMergeConflict,
    R::StartTestRunConfigNotFound,
    R::StartTestRunConfigParsingFailure,
    R::StartTestRunConfigBadVersion,
    R::StartTestRunConfigMissingRequiredStatuses,
    R::StartTestRunConfigBadRequiredStatuses,
    R::StartTestRunNoRequiredStatuses,
    R::StartTestRunRetryExhausted,
    R::StartTestRunDraftPrsNotAllowed,
    R::StartTestRunPermissionsBlockTestBranch,
];
const TESTING_FAILED: [Reason; 12] = concat(&[&START_TEST_RUN, &[R::TestRunFailedByOptimization]]);
const RUN_FAILED: [Reason; 3] = [
    R::TestRunRequiredStatusFailed,
    R::TestRunTimeout,
    R::TestRunFailedByOptimization,
];
/// Failure of the merge call of a tested item.
const MERGE_FAILED: [Reason; 6] = [
    R::MergeRejected,
    R::MergeRateLimitExceeded,
    R::MergeBranchProtectionUnmet,
    R::MergeAlreadyInProgress,
    R::MergeRepeatedServerErrors,
    R::PrUpdatedAtMergeTime,
];
/// Something ahead of the item changed.
const UPSTREAM_CHANGED: [Reason; 4] = [
    R::ManualPushToTargetBranch,
    R::PrerequisiteMergeItemFailed,
    R::PrerequisiteMergeItemCancelled,
    R::PrerequisiteMergeItemManuallyMerged,
];
const RESTARTED: [Reason; 2] = [R::TestsRestartedByUser, R::TestsRestartedByApiRequest];
const TESTING_TO_PENDING: [Reason; 7] =
    concat(&[&UPSTREAM_CHANGED, &RESTARTED, &[R::SkipTheLineInterrupted]]);
const PENDING_FAILURE_TO_PENDING: [Reason; 9] = concat(&[
    &TESTING_TO_PENDING,
    &[R::BisectionRequired, R::BisectionTestRunPassed],
]);
const TESTS_PASSED_TO_PENDING: [Reason; 5] =
    concat(&[&[R::SkipTheLineInterrupted], &UPSTREAM_CHANGED]);
/// The reason that the item which causes the give-up receives, other than a
/// cancellation.
const GIVEN_UP: [Reason; 17] = concat(&[
    &PENDING_FAILURE_TO_PENDING,
    &[R::ManualMerge, R::StartTestRunMergeConflict],
    &MERGE_FAILED,
]);
const DISSOLVED: [Reason; 24] = concat(&[&GIVEN_UP, &CANCELLED]);
const INTERRUPTED: [Reason; 18] = concat(&[&[R::PassedByOptimization], &GIVEN_UP]);

impl Lifecycle for ItemState {
    fn allowed_reasons(from: Option<Self>, to: Self) -> &'static [Reason] {
        use ItemState as S;
        match (from, to) {
            (None, S::NotReady) => &[R::Submitted],
            (Some(S::NotReady), S::Pending) => &[R::MergeItemNowReady],
            (Some(S::Pending), S::Testing) => &[R::TestRunStarted],
            (Some(S::Pending), S::Merged) => &[R::MergeGraphDirectMerge, R::ManualMerge],
            (Some(S::Pending), S::Failed) => &[R::DirectMergeRejected],
            (Some(S::Testing), S::TestsPassed) => &[R::TestRunPassed, R::PassedByOptimization],
            (Some(S::Testing), S::PendingFailure) => {
                &[R::TestRunRequiredStatusFailed, R::TestRunTimeout]
            }
            (Some(S::Testing), S::Failed) => &TESTING_FAILED,
            (Some(S::Testing), S::Pending) => &TESTING_TO_PENDING,
            (Some(S::PendingFailure), S::Pending) => &PENDING_FAILURE_TO_PENDING,
            (Some(S::PendingFailure), S::Testing) => &[R::FailureBudgetRetry],
            (Some(S::PendingFailure), S::TestsPassed) => &[R::SuccessorTestRunPassed],
            (Some(S::PendingFailure), S::Failed) => &RUN_FAILED,
            (Some(S::TestsPassed), S::Merged) => &[R::MergeGraphMerge, R::ManualMerge],
            (Some(S::TestsPassed), S::Pending) => &TESTS_PASSED_TO_PENDING,
            (Some(S::TestsPassed), S::Failed) => &MERGE_FAILED,
            (Some(S::NotReady), S::Cancelled) => &NOT_READY_CANCELLED,
            (Some(S::NotReady | S::Testing | S::PendingFailure), S::Merged) => &[R::ManualMerge],
            (Some(S::Pending | S::Testing | S::PendingFailure | S::TestsPassed), S::Cancelled) => {
                &CANCELLED
            }
            _ => &[],
        }
    }
}

impl Lifecycle for GroupState {
    fn allowed_reasons(from: Option<Self>, to: Self) -> &'static [Reason] {
        use GroupState as S;
        match (from, to) {
            (None, S::Testing) => &[R::TestRunStarted],
            (Some(S::Testing), S::TestsPassed) => &[R::TestRunPassed, R::PassedByOptimization],
            (Some(S::Testing), S::PendingFailure) => {
                &[R::TestRunRequiredStatusFailed, R::TestRunTimeout]
            }
            (Some(S::Testing), S::Failed) => &TESTING_FAILED,
            (Some(S::PendingFailure), S::Testing) => &[R::FailureBudgetRetry],
            (Some(S::PendingFailure), S::TestsPassed) => &[R::SuccessorTestRunPassed],
            (Some(S::PendingFailure), S::Failed) => &RUN_FAILED,
            (Some(S::PendingFailure), S::Bisected) => &[R::BisectionRequired],
            (Some(S::TestsPassed), S::Merged) => &[R::MergeGraphMerge],
            (Some(S::TestsPassed), S::Failed) => &MERGE_FAILED,
            (Some(S::Testing | S::PendingFailure | S::TestsPassed), S::Dissolved) => &DISSOLVED,
            _ => &[],
        }
    }
}

impl Lifecycle for RunStatus {
    fn allowed_reasons(from: Option<Self>, to: Self) -> &'static [Reason] {
        use RunStatus as S;
        match (from, to) {
            (None, S::Assembling) => &[R::TestRunStarted, R::FailureBudgetRetry],
            (Some(S::Assembling), S::InProgress) => &[R::TestBranchAssembled],
            (Some(S::Assembling), S::Failed) => &START_TEST_RUN,
            (Some(S::InProgress), S::Succeeded) => &[R::TestRunPassed],
            (Some(S::InProgress), S::Failed) => &[R::TestRunRequiredStatusFailed],
            (Some(S::InProgress), S::TimedOut) => &[R::TestRunTimeout],
            (Some(S::Assembling | S::InProgress), S::Interrupted) => &INTERRUPTED,
            (Some(S::Assembling | S::InProgress), S::Cancelled) => &CANCELLED,
            _ => &[],
        }
    }
}

impl Lifecycle for QueueState {
    fn allowed_reasons(from: Option<Self>, to: Self) -> &'static [Reason] {
        use QueueState as S;
        match (from, to) {
            (Some(S::Running | S::Draining | S::SwitchingModes), S::Paused) => {
                &[R::AdminRequested, R::AccessLost]
            }
            (Some(S::Paused), S::Running) => &[R::AdminRequested, R::AccessRestored],
            (Some(S::Running), S::Draining) | (Some(S::Draining), S::Running) => {
                &[R::AdminRequested]
            }
            (Some(S::Running), S::SwitchingModes) => &[R::ModeChanged],
            (Some(S::SwitchingModes), S::Running) => &[R::ModeSwitchCompleted],
            _ => &[],
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use strum::IntoEnumIterator;

    use super::*;

    /// A row of a table in `docs/LOTS.md`: states before, state after, and
    /// the reasons, written as the stored names. A name that ends with `*`
    /// is a prefix.
    type Row<S> = (&'static [Option<S>], S, &'static [&'static str]);

    fn expand(names: &[&str]) -> HashSet<Reason> {
        names
            .iter()
            .flat_map(|name| match name.strip_suffix('*') {
                Some(prefix) => Reason::iter()
                    .filter(|r| r.as_str().starts_with(prefix))
                    .collect::<Vec<_>>(),
                None => vec![name.parse().unwrap()],
            })
            .collect()
    }

    fn assert_table<S: Lifecycle + IntoEnumIterator + fmt::Debug>(rows: &[Row<S>]) {
        let froms: Vec<Option<S>> = std::iter::once(None).chain(S::iter().map(Some)).collect();
        for &from in &froms {
            for to in S::iter() {
                let want: HashSet<Reason> = rows
                    .iter()
                    .filter(|(f, t, _)| f.contains(&from) && *t == to)
                    .flat_map(|(_, _, names)| expand(names))
                    .collect();
                for reason in Reason::iter() {
                    let got = check_transition(from, to, reason);
                    assert_eq!(
                        got.is_ok(),
                        want.contains(&reason),
                        "{from:?} -> {to:?} ({reason})"
                    );
                }
            }
        }
    }

    const CANCELLED: &[&str] = &[
        "user_requested",
        "admin_requested",
        "pr_closed",
        "pr_pushed_to",
        "pr_base_branch_changed",
        "pr_converted_to_draft",
        "label_removed",
    ];
    const UPSTREAM: &[&str] = &[
        "manual_push_to_target_branch",
        "prerequisite_merge_item_failed",
        "prerequisite_merge_item_cancelled",
        "prerequisite_merge_item_manually_merged",
    ];
    const RESTARTED: &[&str] = &["tests_restarted_by_user", "tests_restarted_by_api_request"];
    const MERGE_FAILED: &[&str] = &[
        "merge_rejected",
        "merge_rate_limit_exceeded",
        "merge_branch_protection_unmet",
        "merge_already_in_progress",
        "merge_repeated_server_errors",
        "pr_updated_at_merge_time",
    ];
    const BISECTION: &[&str] = &["bisection_required", "bisection_test_run_passed"];
    const RUN_FAILED: &[&str] = &[
        "test_run_required_status_failed",
        "test_run_timeout",
        "test_run_failed_by_optimization",
    ];

    #[test]
    fn item_table() {
        use ItemState as S;
        let live = &[
            Some(S::NotReady),
            Some(S::Pending),
            Some(S::Testing),
            Some(S::PendingFailure),
            Some(S::TestsPassed),
        ];
        assert_table::<ItemState>(&[
            (&[None], S::NotReady, &["submitted"]),
            (&[Some(S::NotReady)], S::Pending, &["merge_item_now_ready"]),
            (&[Some(S::Pending)], S::Testing, &["test_run_started"]),
            (
                &[Some(S::Pending)],
                S::Merged,
                &["merge_graph_direct_merge"],
            ),
            (&[Some(S::Pending)], S::Failed, &["direct_merge_rejected"]),
            (
                &[Some(S::Testing)],
                S::TestsPassed,
                &["test_run_passed", "passed_by_optimization"],
            ),
            (
                &[Some(S::Testing)],
                S::PendingFailure,
                &["test_run_required_status_failed", "test_run_timeout"],
            ),
            (
                &[Some(S::Testing)],
                S::Failed,
                &["start_test_run_*", "test_run_failed_by_optimization"],
            ),
            (&[Some(S::Testing)], S::Pending, UPSTREAM),
            (&[Some(S::Testing)], S::Pending, RESTARTED),
            (
                &[Some(S::Testing)],
                S::Pending,
                &["skip_the_line_interrupted"],
            ),
            (&[Some(S::PendingFailure)], S::Pending, UPSTREAM),
            (&[Some(S::PendingFailure)], S::Pending, RESTARTED),
            (&[Some(S::PendingFailure)], S::Pending, BISECTION),
            (
                &[Some(S::PendingFailure)],
                S::Pending,
                &["skip_the_line_interrupted"],
            ),
            (
                &[Some(S::PendingFailure)],
                S::Testing,
                &["failure_budget_retry"],
            ),
            (
                &[Some(S::PendingFailure)],
                S::TestsPassed,
                &["successor_test_run_passed"],
            ),
            (&[Some(S::PendingFailure)], S::Failed, RUN_FAILED),
            (&[Some(S::TestsPassed)], S::Merged, &["merge_graph_merge"]),
            (&[Some(S::TestsPassed)], S::Pending, UPSTREAM),
            (
                &[Some(S::TestsPassed)],
                S::Pending,
                &["skip_the_line_interrupted"],
            ),
            (&[Some(S::TestsPassed)], S::Failed, MERGE_FAILED),
            (
                &[Some(S::NotReady)],
                S::Cancelled,
                &["not_ready_timeout_expired"],
            ),
            (live, S::Cancelled, CANCELLED),
            (live, S::Merged, &["manual_merge"]),
        ]);
    }

    #[test]
    fn group_table() {
        use GroupState as S;
        let live = &[
            Some(S::Testing),
            Some(S::PendingFailure),
            Some(S::TestsPassed),
        ];
        assert_table::<GroupState>(&[
            (&[None], S::Testing, &["test_run_started"]),
            (
                &[Some(S::Testing)],
                S::TestsPassed,
                &["test_run_passed", "passed_by_optimization"],
            ),
            (
                &[Some(S::Testing)],
                S::PendingFailure,
                &["test_run_required_status_failed", "test_run_timeout"],
            ),
            (
                &[Some(S::Testing)],
                S::Failed,
                &["start_test_run_*", "test_run_failed_by_optimization"],
            ),
            (
                &[Some(S::PendingFailure)],
                S::Testing,
                &["failure_budget_retry"],
            ),
            (
                &[Some(S::PendingFailure)],
                S::TestsPassed,
                &["successor_test_run_passed"],
            ),
            (&[Some(S::PendingFailure)], S::Failed, RUN_FAILED),
            (
                &[Some(S::PendingFailure)],
                S::Bisected,
                &["bisection_required"],
            ),
            (&[Some(S::TestsPassed)], S::Merged, &["merge_graph_merge"]),
            (&[Some(S::TestsPassed)], S::Failed, MERGE_FAILED),
            (live, S::Dissolved, UPSTREAM),
            (live, S::Dissolved, RESTARTED),
            (live, S::Dissolved, BISECTION),
            (live, S::Dissolved, &["skip_the_line_interrupted"]),
            (live, S::Dissolved, CANCELLED),
            (
                live,
                S::Dissolved,
                &["manual_merge", "start_test_run_merge_conflict"],
            ),
            (live, S::Dissolved, MERGE_FAILED),
        ]);
    }

    #[test]
    fn run_table() {
        use RunStatus as S;
        let live = &[Some(S::Assembling), Some(S::InProgress)];
        let given_up: &[&[&str]] = &[
            UPSTREAM,
            RESTARTED,
            BISECTION,
            &["skip_the_line_interrupted"],
            &["manual_merge", "start_test_run_merge_conflict"],
            MERGE_FAILED,
        ];
        let mut rows: Vec<Row<RunStatus>> = vec![
            (
                &[None],
                S::Assembling,
                &["test_run_started", "failure_budget_retry"],
            ),
            (
                &[Some(S::Assembling)],
                S::InProgress,
                &["test_branch_assembled"],
            ),
            (&[Some(S::Assembling)], S::Failed, &["start_test_run_*"]),
            (&[Some(S::InProgress)], S::Succeeded, &["test_run_passed"]),
            (
                &[Some(S::InProgress)],
                S::Failed,
                &["test_run_required_status_failed"],
            ),
            (&[Some(S::InProgress)], S::TimedOut, &["test_run_timeout"]),
            (live, S::Interrupted, &["passed_by_optimization"]),
            (live, S::Cancelled, CANCELLED),
        ];
        rows.extend(
            given_up
                .iter()
                .map(|names| (live as &[_], S::Interrupted, *names)),
        );
        assert_table(&rows);
    }

    #[test]
    fn queue_table() {
        use QueueState as S;
        assert_table::<QueueState>(&[
            (
                &[Some(S::Running), Some(S::Draining), Some(S::SwitchingModes)],
                S::Paused,
                &["admin_requested", "access_lost"],
            ),
            (
                &[Some(S::Paused)],
                S::Running,
                &["admin_requested", "access_restored"],
            ),
            (&[Some(S::Running)], S::Draining, &["admin_requested"]),
            (&[Some(S::Draining)], S::Running, &["admin_requested"]),
            (&[Some(S::Running)], S::SwitchingModes, &["mode_changed"]),
            (
                &[Some(S::SwitchingModes)],
                S::Running,
                &["mode_switch_completed"],
            ),
        ]);
    }

    #[test]
    fn error_names_the_transition() {
        let err = check_transition(Some(ItemState::Merged), ItemState::Pending, R::Submitted)
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "transition merged -> pending (submitted) is not allowed"
        );
        let err = check_transition(None, GroupState::Merged, R::Submitted).unwrap_err();
        assert_eq!(
            err.to_string(),
            "creation in merged (submitted) is not allowed"
        );
    }
}

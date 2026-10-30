use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{GroupId, OrgId, QueueId, Reason, RunId};

/// One attempt to test a group; a group has more than one after retries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestRun {
    /// The run; its test branch carries it.
    pub id: RunId,
    /// The organization of the queue.
    pub org_id: OrgId,
    /// The queue of the group.
    pub queue_id: QueueId,
    /// The group that the run tests.
    pub group_id: GroupId,
    /// Starts at 1; the failure budget counts attempts.
    pub attempt: u32,
    /// The branch that holds the test commit (`crate::test_branch`).
    pub test_branch: String,
    /// `None` until assembly creates the test commit.
    pub test_sha: Option<String>,
    /// The draft PR from the test branch that triggers the CI; `None` before
    /// the assembly, or when the trigger is not `draft_pr`.
    pub draft_pr_number: Option<u64>,
    /// Where the run is in its life.
    pub status: RunStatus,
    /// The names of the checks that must pass on the test commit; empty
    /// until the assembly resolves them.
    pub required_statuses: Vec<String>,
    /// `None` until assembly resolves the required statuses.
    pub required_statuses_source: Option<RequiredStatusesSource>,
    /// When the queue created the run.
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
    /// When the run reached a terminal status; `None` before.
    #[serde(with = "time::serde::rfc3339::option")]
    pub ended_at: Option<OffsetDateTime>,
    /// Why the run failed; `None` for a run that did not fail.
    pub failure_reason: Option<Reason>,
    /// Required statuses that failed the run.
    pub failure_causing_statuses: Vec<String>,
    /// What a member conflicts with, when the assembly ended on a merge
    /// conflict; `None` otherwise.
    pub conflict_culprit: Option<ConflictCulprit>,
}

closed_enum! {
    /// Where a run is in its life.
    pub enum RunStatus {
        /// The assembly job builds the test branch.
        Assembling,
        /// The CI tests the test commit.
        InProgress,
        /// Every required status passed on the test commit.
        Succeeded,
        /// A required status failed, or the assembly failed.
        Failed,
        /// The queue replaced the run: the group was given up or proved by a
        /// successor.
        Interrupted,
        /// A member of the group was cancelled.
        Cancelled,
        /// The run lasted longer than `testing_timeout_minutes`.
        TimedOut,
    }
}

impl RunStatus {
    /// Whether the run can change status no more.
    pub const fn is_terminal(self) -> bool {
        !matches!(self, Self::Assembling | Self::InProgress)
    }
}

/// What the member of a group conflicts with when its merge into the test
/// branch returns 409 (`start_test_run_merge_conflict`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConflictCulprit {
    /// A PR ahead in the queue, merged in the test branch before the member.
    QueuePr {
        /// The number of the PR ahead.
        pr_number: u64,
    },
    /// The target branch itself: the member conflicts with it alone.
    BaseBranch,
}

closed_enum! {
    /// Where the required statuses of a run come from, from the strongest.
    pub enum RequiredStatusesSource {
        /// `.cihub.yaml` on the target branch at `base_sha`.
        RepoConfig,
        /// The rules of the SCM on the target branch: GitHub protections and
        /// rulesets, GitLab protected branches.
        BranchProtection,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflict_culprit_is_tagged_by_kind() {
        let json = serde_json::to_string(&ConflictCulprit::QueuePr { pr_number: 7 }).unwrap();
        assert_eq!(json, r#"{"kind":"queue_pr","pr_number":7}"#);
    }
}

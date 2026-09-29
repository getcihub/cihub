use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{
    CheckConclusion, CheckKind, GroupId, GroupKind, GroupState, ItemId, OrgId, QueueId, Reason,
    RunId, RunStatus,
};

/// The unit that is tested and held in `pending_failure`: one PR or a batch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestGroup {
    pub id: GroupId,
    pub org_id: OrgId,
    pub queue_id: QueueId,
    pub kind: GroupKind,
    pub state: GroupState,
    /// Reason of the last transition; the members receive the same one.
    pub state_reason: Reason,
    /// Members in queue order; each one merges as its own commit.
    pub item_ids: Vec<ItemId>,
    /// The failed batch that a bisection half comes from.
    pub parent_group_id: Option<GroupId>,
    /// Items the group is tested on top of, in queue order. A run stays valid
    /// only while this set and `base_sha` do not change.
    pub predecessor_ids: Vec<ItemId>,
    pub base_sha: String,
}

/// One attempt to test a group; a group has more than one after retries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestRun {
    pub id: RunId,
    pub org_id: OrgId,
    pub queue_id: QueueId,
    pub group_id: GroupId,
    /// Starts at 1; the failure budget counts attempts.
    pub attempt: u32,
    pub test_branch: String,
    /// `None` until assembly creates the test commit.
    pub test_sha: Option<String>,
    pub draft_pr_number: Option<u64>,
    pub status: RunStatus,
    pub required_statuses: Vec<String>,
    /// `None` until assembly resolves the required statuses.
    pub required_statuses_source: Option<RequiredStatusesSource>,
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub ended_at: Option<OffsetDateTime>,
    pub failure_reason: Option<Reason>,
    /// Required statuses that failed the run.
    pub failure_causing_statuses: Vec<String>,
    pub conflict_culprit: Option<ConflictCulprit>,
}

/// What a `start_test_run_merge_conflict` conflicts with, from the merge call
/// that returned 409.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConflictCulprit {
    QueuePr { pr_number: u64 },
    BaseBranch,
}

closed_enum! {
    /// Where the required statuses of a run come from, from the strongest.
    pub enum RequiredStatusesSource {
        /// The override in the dashboard.
        UiOverride,
        /// `.cihub.yaml` on the target branch at `base_sha`.
        RepoConfig,
        /// The rules of the SCM on the target branch: GitHub protections and
        /// rulesets, GitLab protected branches.
        BranchProtection,
    }
}

/// Last report of a check run or commit status on the test SHA of a run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckObservation {
    pub org_id: OrgId,
    pub run_id: RunId,
    pub name: String,
    pub kind: CheckKind,
    /// The name is in the required statuses of the run.
    pub required: bool,
    /// `None` while the check has not concluded.
    pub conclusion: Option<CheckConclusion>,
    pub details_url: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
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

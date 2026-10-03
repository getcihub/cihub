use std::collections::BTreeSet;

use cihub_domain::{
    CheckConclusion, CheckReport, ConnectionId, MergeMethod, Mergeability, PrState, ReviewDecision,
};
use time::OffsetDateTime;

/// The connection that a call uses: its quota, its budget and its credentials.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionRef {
    pub id: ConnectionId,
    /// The identifier of the connection on the SCM (`DECISIONS.md` D49).
    pub external_id: String,
}

/// A repository, as a call names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoRef {
    pub connection: ConnectionRef,
    pub external_id: String,
    pub namespace: String,
    pub name: String,
}

/// A user account on the SCM (`DECISIONS.md` D51).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScmUser {
    pub external_id: String,
    pub login: String,
}

/// The state of a PR as the SCM reports it, in one read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrSnapshot {
    pub number: u64,
    pub state: PrState,
    pub head_sha: String,
    pub base_branch: String,
    pub head_branch: String,
    /// The head branch is in another repository.
    pub from_fork: bool,
    pub created_at: OffsetDateTime,
    pub draft: bool,
    /// The title and the description, for the title of the merge commit
    /// (`DECISIONS.md` D126).
    pub title: String,
    pub body: String,
    pub labels: BTreeSet<String>,
    pub mergeability: Mergeability,
    pub review_decision: ReviewDecision,
    /// The logins whose last review approves the PR, or asks for changes.
    pub approved_by: BTreeSet<String>,
    pub changes_requested_by: BTreeSet<String>,
    /// The users and teams whose review the PR asks for.
    pub review_requested: BTreeSet<String>,
    /// The checks and statuses of the head.
    pub checks: Vec<CheckReport>,
    pub author: ScmUser,
    pub additions: u64,
    pub deletions: u64,
    pub changed_files: u64,
}

/// The check `name` has a passing conclusion on the head of `pr`.
pub fn check_passed(pr: &PrSnapshot, name: &str) -> bool {
    pr.checks.iter().any(|check| {
        check.name == name && check.conclusion.is_some_and(CheckConclusion::is_passing)
    })
}

/// The checks that the rules of a branch require before a merge.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RequiredChecks {
    pub names: BTreeSet<String>,
    /// The rules require the head of a PR to be up to date with its base.
    pub up_to_date: bool,
}

/// The state of a commit status that CIHub publishes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitStatus {
    Pending,
    Success,
    Failure,
    Error,
}

/// A commit status to publish on the commit `sha`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusUpdate {
    pub sha: String,
    pub context: String,
    pub status: CommitStatus,
    pub description: String,
    pub details_url: Option<String>,
}

/// A merge of the PR `number`, guarded by its head: the SCM refuses it when
/// the head is no longer `head_sha`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrMerge {
    pub number: u64,
    pub head_sha: String,
    /// The test commit that proved the PR: `fast_forward` moves the base
    /// there (`DECISIONS.md` D124).
    pub tested_sha: String,
    pub method: MergeMethod,
    pub title: String,
    pub message: String,
}

/// The result of a merge of a commit into a ref.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MergeIntoRef {
    /// The ref moved to this new merge commit.
    Merged { sha: String },
    /// The ref already contains the commit: nothing changed.
    AlreadyContained,
    /// The commit conflicts with the ref.
    Conflict,
}

/// An attempt of the SCM to send a webhook delivery to CIHub
/// (`DECISIONS.md` D88).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentDelivery {
    /// The identifier that the delivery carries; its new attempts share it.
    pub delivery_id: String,
    /// The identifier of this attempt, to ask for the delivery again.
    pub attempt_id: String,
}

/// The account of the SCM that a connection serves: an organization or a
/// user (`DECISIONS.md` D48).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScmAccount {
    pub external_id: String,
    /// The name of the account in the paths of the SCM.
    pub path: String,
}

/// A repository that a connection reaches, as the SCM lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScmRepo {
    pub external_id: String,
    pub namespace: String,
    pub name: String,
    pub default_branch: String,
}

/// A point of the configuration of the SCM that the queue needs, and that a
/// repository misses (`DECISIONS.md` D162, `docs/GITHUB.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SetupFinding {
    /// No rule restricts the updates of the target branch: a merge can skip
    /// the queue.
    UpdateRestrictionMissing,
    /// A rule restricts the updates of the target branch, but does not let
    /// CIHub through: the queue cannot merge.
    AppNotExempt,
    /// A rule covers the test branches too: the queue cannot write them.
    TestBranchesNotExcluded,
    /// The rules ask that a PR be up to date with its base: each merge makes
    /// the next PRs wait.
    UpToDateRequired,
}

impl SetupFinding {
    pub fn code(self) -> &'static str {
        match self {
            Self::UpdateRestrictionMissing => "update_restriction_missing",
            Self::AppNotExempt => "app_not_exempt",
            Self::TestBranchesNotExcluded => "test_branches_not_excluded",
            Self::UpToDateRequired => "up_to_date_required",
        }
    }
}

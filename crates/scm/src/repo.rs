use std::collections::BTreeSet;

use cihub_domain::{
    CheckConclusion, CheckReport, JobClass, MergeMethod, Mergeability, Permission, PrState,
    ReviewDecision,
};
use time::OffsetDateTime;

use crate::{RepoRef, ScmError, ScmFuture, ScmUser};

/// A read of PRs asks for this many PRs at most.
pub const MAX_PRS_PER_READ: usize = 50;

/// A list of the open PRs of a branch gives this many PRs at most.
pub const MAX_OPEN_PRS: usize = 1_000;

/// The calls on a repository. Each one names its repository, and so its
/// connection, whose budget applies.
pub trait RepoCalls: Send + Sync {
    /// The PRs of `numbers`, at most [`MAX_PRS_PER_READ`], in one read, in
    /// the order of `numbers`. A PR that does not exist is absent from the
    /// answer.
    fn pull_requests<'a>(
        &'a self,
        repo: &'a RepoRef,
        numbers: &'a [u64],
        class: JobClass,
    ) -> ScmFuture<'a, Vec<PrSnapshot>>;

    /// The numbers of the open PRs whose base is `base_branch`, oldest first,
    /// at most [`MAX_OPEN_PRS`]: the PRs that a lost delivery hid from CIHub.
    fn open_pull_requests<'a>(
        &'a self,
        repo: &'a RepoRef,
        base_branch: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<u64>>;

    /// The PR `number`, by a read of one PR; `NotFound` when it does not
    /// exist.
    fn pull_request<'a>(
        &'a self,
        repo: &'a RepoRef,
        number: u64,
        class: JobClass,
    ) -> ScmFuture<'a, PrSnapshot> {
        Box::pin(async move {
            self.pull_requests(repo, std::slice::from_ref(&number), class)
                .await?
                .pop()
                .ok_or_else(|| ScmError::NotFound(format!("PR {number}")))
        })
    }

    /// The commit of the branch `branch`, or `None` when it does not exist.
    fn branch_head<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, Option<String>>;

    /// The checks and statuses of the commit `sha`.
    fn checks<'a>(
        &'a self,
        repo: &'a RepoRef,
        sha: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<CheckReport>>;

    /// What the rules of the branch `branch` require before a merge.
    fn required_checks<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, RequiredChecks>;

    /// The content of the file `path` at `git_ref`, or `None` when it does
    /// not exist.
    fn file<'a>(
        &'a self,
        repo: &'a RepoRef,
        git_ref: &'a str,
        path: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, Option<String>>;

    /// The permission of `user` on `repo`; `Permission::None` when they have
    /// no access.
    fn permission<'a>(
        &'a self,
        repo: &'a RepoRef,
        user: &'a ScmUser,
        class: JobClass,
    ) -> ScmFuture<'a, Permission>;

    /// Whether the commit `ancestor` is in the history of the commit
    /// `descendant`, or is it: a PR whose head descends from the head of its
    /// target is up to date.
    fn is_ancestor<'a>(
        &'a self,
        repo: &'a RepoRef,
        ancestor: &'a str,
        descendant: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, bool>;

    /// The paths of the files that the PR `number` changes, read only when a
    /// condition uses them.
    fn pr_files<'a>(
        &'a self,
        repo: &'a RepoRef,
        number: u64,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<String>>;

    /// The path of each unresolved review thread of the PR `number`, read
    /// only when a condition uses them.
    fn unresolved_threads<'a>(
        &'a self,
        repo: &'a RepoRef,
        number: u64,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<String>>;

    /// Creates the branch `branch` at `sha`, or moves it there as `how`
    /// says; `NotFastForward` when a move that is not forced does not go
    /// forward.
    fn set_branch<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        sha: &'a str,
        how: BranchMove,
        class: JobClass,
    ) -> ScmFuture<'a, ()>;

    /// Deletes the branch `branch`; a branch that does not exist is a success.
    fn delete_branch<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, ()>;

    /// Merges the commit `sha` into the branch `branch`.
    fn merge_into_branch<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        sha: &'a str,
        message: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, MergeIntoRef>;

    /// Merges the PR if its head is still the expected one, and returns the
    /// commit that lands on the base. A PR already merged returns its commit.
    fn merge_pr<'a>(
        &'a self,
        repo: &'a RepoRef,
        merge: &'a PrMerge,
        class: JobClass,
    ) -> ScmFuture<'a, String>;

    /// Publishes a commit status.
    fn set_status<'a>(
        &'a self,
        repo: &'a RepoRef,
        status: &'a StatusUpdate,
        class: JobClass,
    ) -> ScmFuture<'a, ()>;

    /// Creates a comment on the PR and returns its identifier on the SCM.
    fn create_comment<'a>(
        &'a self,
        repo: &'a RepoRef,
        number: u64,
        body: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, String>;

    /// Replaces the body of the comment `comment_id`.
    fn update_comment<'a>(
        &'a self,
        repo: &'a RepoRef,
        comment_id: &'a str,
        body: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, ()>;

    /// Shows on the comment `comment_id` that CIHub accepted its command.
    /// Acknowledging twice has no more effect.
    fn acknowledge_comment<'a>(
        &'a self,
        repo: &'a RepoRef,
        comment_id: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, ()>;

    /// Adds the label; a label that the PR carries already is a success.
    fn add_label<'a>(
        &'a self,
        repo: &'a RepoRef,
        number: u64,
        label: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, ()>;

    /// Removes the label; a label that the PR does not carry is a success.
    fn remove_label<'a>(
        &'a self,
        repo: &'a RepoRef,
        number: u64,
        label: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, ()>;

    /// Opens `draft` and returns its number. An open PR from its head into
    /// its base returns its number.
    fn open_draft_pr<'a>(
        &'a self,
        repo: &'a RepoRef,
        draft: &'a DraftPr,
        class: JobClass,
    ) -> ScmFuture<'a, u64>;

    /// Closes the PR; a closed PR is a success.
    fn close_pr<'a>(&'a self, repo: &'a RepoRef, number: u64, class: JobClass)
    -> ScmFuture<'a, ()>;

    /// What the configuration of `branch` misses for the queue, in the order
    /// of [`SetupFinding`].
    fn check_setup<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<SetupFinding>>;

    /// The page of the SCM where an owner changes the rules of the branches
    /// of `repo`, or `None` when the SCM has none. It makes no call.
    fn branch_rules_url(&self, repo: &RepoRef) -> Option<String>;

    /// The page of PR `number` of `repo` on the SCM. It makes no call.
    fn pr_url(&self, repo: &RepoRef, number: u64) -> String;
}

/// The state of a PR as the SCM reports it, in one read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrSnapshot {
    /// The number of the PR in its repository.
    pub number: u64,
    /// The page of the PR on the SCM.
    pub url: String,
    /// Open, closed or merged.
    pub state: PrState,
    /// The commit at the head of the PR now.
    pub head_sha: String,
    /// The branch that the PR targets.
    pub base_branch: String,
    /// The branch that holds the changes of the PR.
    pub head_branch: String,
    /// The head branch is in another repository.
    pub is_from_fork: bool,
    /// When the PR was opened.
    pub created_at: OffsetDateTime,
    /// The PR is a draft: the SCM does not let it merge.
    pub is_draft: bool,
    /// The title of the PR, for the title of the merge commit.
    pub title: String,
    /// The description of the PR, in Markdown; empty when it has none.
    pub body: String,
    /// The names of the labels of the PR.
    pub labels: BTreeSet<String>,
    /// Whether the SCM would merge the PR now, and why not.
    pub mergeability: Mergeability,
    /// What the reviews decide, as the rules of the base branch count them.
    pub review_decision: ReviewDecision,
    /// The logins whose last review approves the PR.
    pub approved_by: BTreeSet<String>,
    /// The logins whose last review asks for changes.
    pub changes_requested_by: BTreeSet<String>,
    /// The users and teams whose review the PR asks for: a login, or the
    /// name of a team.
    pub review_requested: BTreeSet<String>,
    /// The checks and statuses of the head.
    pub checks: Vec<CheckReport>,
    /// Who opened the PR.
    pub author: ScmUser,
    /// The lines that the PR adds, as the SCM counts them.
    pub additions: u64,
    /// The lines that the PR removes, as the SCM counts them.
    pub deletions: u64,
    /// The number of files that the PR changes.
    pub changed_files: u64,
}

impl PrSnapshot {
    /// The check `name` has a passing conclusion on the head.
    pub fn has_passed(&self, name: &str) -> bool {
        has_passed(&self.checks, name)
    }
}

/// The check `name` has a passing conclusion among `checks`.
pub(crate) fn has_passed(checks: &[CheckReport], name: &str) -> bool {
    checks.iter().any(|check| {
        check.name == name && check.conclusion.is_some_and(CheckConclusion::is_passing)
    })
}

/// The checks that the rules of a branch require before a merge.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RequiredChecks {
    /// The names of the checks and statuses that must pass on the head.
    pub names: BTreeSet<String>,
    /// The rules require the head of a PR to be up to date with its base.
    pub requires_up_to_date: bool,
}

/// How [`RepoCalls::set_branch`] moves a branch that exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BranchMove {
    /// Only to a commit that descends from the head of the branch.
    FastForwardOnly,
    /// To any commit, even one that drops commits of the branch.
    Force,
}

/// A draft PR to open from the branch `head` into the branch `base`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DraftPr {
    /// The branch that holds the changes.
    pub head: String,
    /// The branch that the PR targets.
    pub base: String,
    /// The title of the PR.
    pub title: String,
    /// The description of the PR, in Markdown.
    pub body: String,
}

/// The state of a commit status that CIHub publishes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitStatus {
    /// The work runs: the status has no conclusion yet.
    Pending,
    /// The work passed.
    Success,
    /// The work failed.
    Failure,
    /// The work could not run to its end.
    Error,
}

/// A commit status to publish on the commit `sha`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusUpdate {
    /// The commit that the status reports on.
    pub sha: String,
    /// The name of the status; a new status with the same name replaces it.
    pub context: String,
    /// The state of the status.
    pub status: CommitStatus,
    /// The short text that the SCM shows next to the status.
    pub description: String,
    /// The page that the status links to; `None` for no link.
    pub details_url: Option<String>,
}

/// A merge of the PR `number`, guarded by its head: the SCM refuses it when
/// the head is no longer `head_sha`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrMerge {
    /// The number of the PR in its repository.
    pub number: u64,
    /// The head that the queue tested; the merge fails when the head moved.
    pub head_sha: String,
    /// The test commit that proved the PR: `fast_forward` moves the base
    /// there.
    pub tested_sha: String,
    /// How the PR lands on its base.
    pub method: MergeMethod,
    /// The title of the commit that lands; ignored by a fast forward.
    pub title: String,
    /// The body of the commit that lands; ignored by a fast forward.
    pub message: String,
}

/// The result of a merge of a commit into a ref.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MergeIntoRef {
    /// The ref moved to this new merge commit.
    Merged {
        /// The new merge commit, now the head of the ref.
        sha: String,
    },
    /// The ref already contains the commit: nothing changed.
    AlreadyContained,
    /// The commit conflicts with the ref.
    Conflict,
}

/// A point of the configuration of the SCM that the queue needs, and that a
/// repository misses (`specs/GITHUB.md`).
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
    /// The stable code of the finding, which the API and the stored setup of
    /// a repository carry.
    pub fn code(self) -> &'static str {
        match self {
            Self::UpdateRestrictionMissing => "update_restriction_missing",
            Self::AppNotExempt => "app_not_exempt",
            Self::TestBranchesNotExcluded => "test_branches_not_excluded",
            Self::UpToDateRequired => "up_to_date_required",
        }
    }
}

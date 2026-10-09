use std::future::Future;
use std::pin::Pin;

use cihub_domain::{CheckReport, JobClass, Permission};
use time::OffsetDateTime;

use crate::{
    ConnectionRef, MergeIntoRef, PrMerge, PrSnapshot, RecentDelivery, RepoRef, RequiredChecks,
    ScmAccount, ScmError, ScmRepo, ScmUser, SetupFinding, StatusUpdate,
};

/// The answer of a call. A future is boxed because an executor holds an
/// `Arc<dyn Scm>` (`DECISIONS.md` D71), and a trait with `async fn` cannot be
/// used as `dyn`.
pub type ScmFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, ScmError>> + Send + 'a>>;

/// A read of PRs asks for this many PRs at most.
pub const MAX_PRS_PER_READ: usize = 50;

/// The calls of CIHub to its SCM (`DECISIONS.md` D76). Each call names
/// its repository, and so its connection, and its class, which decides what
/// the budget of the connection lets through. A write whose result already
/// exists succeeds without a change, so that a job can run twice (principle
/// III).
pub trait Scm: Send + Sync {
    /// The PRs of `numbers`, at most [`MAX_PRS_PER_READ`], in one read. A PR
    /// that does not exist is absent from the answer.
    fn pull_requests<'a>(
        &'a self,
        repo: &'a RepoRef,
        numbers: &'a [u64],
        class: JobClass,
    ) -> ScmFuture<'a, Vec<PrSnapshot>>;

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

    fn permission<'a>(
        &'a self,
        repo: &'a RepoRef,
        user: &'a ScmUser,
        class: JobClass,
    ) -> ScmFuture<'a, Permission>;

    /// Whether the commit `ancestor` is in the history of the commit
    /// `descendant`, or is it: a PR whose head descends from the head of its
    /// target is up to date (`DECISIONS.md` D153).
    fn is_ancestor<'a>(
        &'a self,
        repo: &'a RepoRef,
        ancestor: &'a str,
        descendant: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, bool>;

    /// The paths of the files that the PR `number` changes, read only when a
    /// condition uses them (`DECISIONS.md` D143).
    fn pr_files<'a>(
        &'a self,
        repo: &'a RepoRef,
        number: u64,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<String>>;

    /// The path of each unresolved review thread of the PR `number`, read
    /// only when a condition uses them (`DECISIONS.md` D143).
    fn unresolved_threads<'a>(
        &'a self,
        repo: &'a RepoRef,
        number: u64,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<String>>;

    /// Creates the branch `branch` at `sha`, or moves it there, forced when
    /// `force`.
    fn set_branch<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        sha: &'a str,
        force: bool,
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

    fn update_comment<'a>(
        &'a self,
        repo: &'a RepoRef,
        comment_id: &'a str,
        body: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, ()>;

    /// Shows on the comment `comment_id` that CIHub accepted its command
    /// (`DECISIONS.md` D166). Acknowledging twice has no more effect.
    fn acknowledge_comment<'a>(
        &'a self,
        repo: &'a RepoRef,
        comment_id: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, ()>;

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

    /// Opens a draft PR from `head` into `base` and returns its number. An
    /// open PR from `head` into `base` returns its number.
    fn open_draft_pr<'a>(
        &'a self,
        repo: &'a RepoRef,
        head: &'a str,
        base: &'a str,
        title: &'a str,
        body: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, u64>;

    /// Closes the PR; a closed PR is a success.
    fn close_pr<'a>(&'a self, repo: &'a RepoRef, number: u64, class: JobClass)
    -> ScmFuture<'a, ()>;

    /// The webhook deliveries that the SCM sent to CIHub since `since`,
    /// each attempt once, for every connection (`DECISIONS.md` D88). No
    /// connection makes the call, so no budget of a connection applies.
    fn recent_deliveries(
        &self,
        since: OffsetDateTime,
        class: JobClass,
    ) -> ScmFuture<'_, Vec<RecentDelivery>>;

    /// Asks the SCM to send the attempt `attempt_id` of a delivery again.
    fn redeliver<'a>(&'a self, attempt_id: &'a str, class: JobClass) -> ScmFuture<'a, ()>;

    /// The account that the connection `external_id` serves
    /// (`DECISIONS.md` D155). No connection of CIHub exists yet, so no budget of
    /// a connection applies.
    fn installation<'a>(
        &'a self,
        external_id: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, ScmAccount>;

    /// What the configuration of `branch` misses for the queue, in the order
    /// of [`SetupFinding`] (`DECISIONS.md` D162).
    fn check_setup<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<SetupFinding>>;

    /// Every repository that `connection` reaches.
    fn list_repos<'a>(
        &'a self,
        connection: &'a ConnectionRef,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<ScmRepo>>;
}

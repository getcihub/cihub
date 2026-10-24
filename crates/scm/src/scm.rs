use std::future::Future;
use std::pin::Pin;

use cihub_domain::{CheckReport, JobClass, MembershipRole, Permission};
use secrecy::SecretString;
use time::OffsetDateTime;

use crate::{
    ConnectionRef, MergeIntoRef, PrMerge, PrSnapshot, RecentDelivery, RepoRef, RequiredChecks,
    ScmAccount, ScmError, ScmInstallation, ScmMember, ScmPersonReach, ScmPersonToken, ScmRepo,
    ScmUser, SetupFinding, SignInCode, SignInRequest, SignedIn, StatusUpdate,
};

/// The answer of a call. A future is boxed because an executor holds an
/// `Arc<dyn Scm>`, and a trait with `async fn` cannot be used as `dyn`.
pub type ScmFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, ScmError>> + Send + 'a>>;

/// A read of PRs asks for this many PRs at most.
pub const MAX_PRS_PER_READ: usize = 50;

/// A list of the open PRs of a branch gives this many PRs at most.
pub const MAX_OPEN_PRS: usize = 1_000;

/// The calls of CIHub to its SCM. Each call names its repository, and so its
/// connection, and its class, which decides what the budget of the connection
/// lets through. A write whose result already exists succeeds without a change,
/// so that a job can run twice (principle III).
pub trait Scm: Send + Sync {
    /// The PRs of `numbers`, at most [`MAX_PRS_PER_READ`], in one read. A PR
    /// that does not exist is absent from the answer.
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

    /// Shows on the comment `comment_id` that CIHub accepted its command.
    /// Acknowledging twice has no more effect.
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

    /// Opens a PR from `head` into `base`, a draft when `draft`, and
    /// returns its number. An open PR from `head` into `base` returns its
    /// number.
    #[expect(
        clippy::too_many_arguments,
        reason = "the fields of a PR, as the SCM takes them"
    )]
    fn open_pr<'a>(
        &'a self,
        repo: &'a RepoRef,
        head: &'a str,
        base: &'a str,
        title: &'a str,
        body: &'a str,
        draft: bool,
        class: JobClass,
    ) -> ScmFuture<'a, u64>;

    /// Writes `text` at `path` on the branch `branch`, in one commit of
    /// `message` on its head; a file that already holds `text` makes no
    /// commit. A rule of the repository that refuses the commit is
    /// `Forbidden`.
    fn commit_file<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        path: &'a str,
        text: &'a str,
        message: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, ()>;

    /// Closes the PR; a closed PR is a success.
    fn close_pr<'a>(&'a self, repo: &'a RepoRef, number: u64, class: JobClass)
    -> ScmFuture<'a, ()>;

    /// The webhook deliveries that the SCM sent to CIHub since `since`, each
    /// attempt once with its outcome, for every connection. No connection makes
    /// the call, so no budget of a connection applies.
    fn recent_deliveries(
        &self,
        since: OffsetDateTime,
        class: JobClass,
    ) -> ScmFuture<'_, Vec<RecentDelivery>>;

    /// Asks the SCM to send the attempt `attempt_id` of a delivery again.
    fn redeliver<'a>(&'a self, attempt_id: &'a str, class: JobClass) -> ScmFuture<'a, ()>;

    /// The installation `external_id`, or `None` when the SCM does not know it.
    /// No connection of CIHub may exist yet, so no budget of a connection
    /// applies.
    fn installation<'a>(
        &'a self,
        external_id: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, Option<ScmInstallation>>;

    /// Every installation of the App, suspended ones included.
    fn installations(&self, class: JobClass) -> ScmFuture<'_, Vec<ScmInstallation>>;

    /// What the configuration of `branch` misses for the queue, in the order
    /// of [`SetupFinding`].
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

    /// The page of the SCM where an owner installs the App on an account, or
    /// chooses the repositories of its installation. No connection makes the
    /// call, so no budget of a connection applies.
    fn app_install_url(&self, class: JobClass) -> ScmFuture<'_, String>;

    /// The page of the SCM where an owner changes the rules of the branches
    /// of `repo`, or `None` when the SCM has none. It makes no call.
    fn branch_rules_url(&self, repo: &RepoRef) -> Option<String>;

    /// The page of PR `number` of `repo` on the SCM. It makes no call.
    fn pr_url(&self, repo: &RepoRef, number: u64) -> String;

    /// The address of the SCM where a person signs in to CIHub, or `None`
    /// when the operator configured no sign-in.
    fn sign_in_url(&self, request: &SignInRequest) -> Option<String>;

    /// Exchanges the code of a sign-in, reads the person and the
    /// installations of the App that they reach, with their role in each, and
    /// gives the token of the person. An installation where the person has no
    /// role has no membership. No connection makes the calls, so no budget of
    /// a connection applies: the calls with the token of a person spend the
    /// rate limit of the person.
    fn sign_in<'a>(
        &'a self,
        code: &'a SignInCode,
        class: JobClass,
    ) -> ScmFuture<'a, (SignedIn, ScmPersonToken)>;

    /// A new token for the person of `refresh`, which then works no more.
    /// `AccessLost` when the SCM refuses it: the person signs in again.
    fn refresh_person_token<'a>(
        &'a self,
        refresh: &'a SecretString,
        class: JobClass,
    ) -> ScmFuture<'a, ScmPersonToken>;

    /// The installations of the App that the person of `token` reaches, and
    /// the repositories of each one with their permission. `AccessLost` when
    /// the SCM refuses the token.
    fn person_reach<'a>(
        &'a self,
        token: &'a SecretString,
        class: JobClass,
    ) -> ScmFuture<'a, ScmPersonReach>;

    /// The permission of the person of `token` on `repo`; `None` when they do
    /// not see it. `AccessLost` when the SCM refuses the token.
    fn person_permission<'a>(
        &'a self,
        token: &'a SecretString,
        repo: &'a RepoRef,
        class: JobClass,
    ) -> ScmFuture<'a, Permission>;

    /// The role of `person`, whose token is `token`, in `account`; `None`
    /// without one. The owner of the account of a person is its `admin`.
    /// `AccessLost` when the SCM refuses the token.
    fn person_role<'a>(
        &'a self,
        token: &'a SecretString,
        person: &'a ScmUser,
        account: &'a ScmAccount,
        class: JobClass,
    ) -> ScmFuture<'a, Option<MembershipRole>>;

    /// The members of the account of `connection`, with their role. The
    /// owner of the account of a person is its only member, as `admin`.
    fn members<'a>(
        &'a self,
        connection: &'a ConnectionRef,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<ScmMember>>;
}

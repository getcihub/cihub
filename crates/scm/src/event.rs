use cihub_domain::CheckReport;

use crate::ScmUser;

/// The mark of the submission box in a comment of CIHub: `- [ ] <mark>` to
/// check, `- [x] <mark>` once checked.
pub const SUBMIT_BOX: &str = "<!-- cihub:submit -->";

/// A webhook delivery, as the adapter reads it: the connection that received
/// it and what it tells.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScmDelivery {
    /// The identifier of the connection on the SCM; `None` when the delivery
    /// does not name one.
    pub connection_external_id: Option<String>,
    /// What the delivery tells.
    pub event: ScmEvent,
}

/// What a delivery tells CIHub. An event says what changed, not the new
/// state: the scheduler reads the state again when it needs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScmEvent {
    /// A PR, its reviews or its labels changed.
    PrChanged {
        /// The repository of the PR.
        repo_external_id: String,
        /// The number of the PR in its repository.
        number: u64,
        /// The branch that the PR targets after the change.
        base_branch: String,
        /// The label that the change added, when it added one: it can submit
        /// the PR.
        label_added: Option<String>,
        /// The PR is open and out of draft after the change.
        is_ready_for_review: bool,
    },
    /// A branch moved.
    Pushed {
        /// The repository of the branch.
        repo_external_id: String,
        /// The short name of the branch.
        branch: String,
        /// `None` for a new branch.
        before: Option<String>,
        /// `None` for a deleted branch.
        after: Option<String>,
        /// Who pushed.
        sender: ScmUser,
    },
    /// A check or a commit status of a commit changed.
    CheckChanged {
        /// The repository of the commit.
        repo_external_id: String,
        /// The commit that the check reports on.
        sha: String,
        /// The branches whose head is `sha`, when the SCM gives them: they
        /// route the check of a test branch to its queue.
        branches: Vec<String>,
        /// The check or the commit status, as it is now.
        check: CheckReport,
    },
    /// Someone else than CIHub checked the submission box of a comment of
    /// CIHub on a PR.
    SubmitBoxChecked {
        /// The repository of the PR.
        repo_external_id: String,
        /// The number of the PR in its repository.
        number: u64,
        /// The comment of CIHub that holds the box.
        comment_external_id: String,
        /// Who checked the box.
        sender: ScmUser,
    },
    /// A comment on a PR, which can hold a command.
    Commented {
        /// The repository of the PR.
        repo_external_id: String,
        /// The number of the PR in its repository.
        number: u64,
        /// The new comment.
        comment_external_id: String,
        /// Who wrote the comment.
        author: ScmUser,
        /// The text of the comment, in Markdown.
        body: String,
    },
    /// The rules of the repository changed: the required checks can differ.
    RulesChanged {
        /// The repository whose rules changed.
        repo_external_id: String,
    },
    /// The repository was renamed or deleted.
    RepoChanged {
        /// The repository that changed.
        repo_external_id: String,
    },
    /// The installation of the connection changed on the SCM: created,
    /// deleted, suspended, active again or with new permissions. CIHub reads
    /// the installation again.
    ConnectionChanged,
    /// The connection gained or lost repositories.
    ReposChanged {
        /// The external identifiers of the repositories that it gained.
        added: Vec<String>,
        /// The external identifiers of the repositories that it lost.
        removed: Vec<String>,
    },
    /// A delivery that CIHub does not use.
    Ignored,
}

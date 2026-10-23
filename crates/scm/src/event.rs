use cihub_domain::{CheckReport, PrState};

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
    pub event: ScmEvent,
}

/// What a delivery tells CIHub. An event says what changed, not the new
/// state: the scheduler reads the state again when it needs it (lot 5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScmEvent {
    /// A PR, its reviews or its labels changed.
    PrChanged {
        repo_external_id: String,
        number: u64,
        base_branch: String,
        /// The label that the change added, when it added one: it can submit
        /// the PR.
        label_added: Option<String>,
        /// The PR is open and out of draft after the change.
        is_ready_for_review: bool,
        /// Open, closed or merged, after the change.
        state: PrState,
    },
    /// A branch moved.
    Pushed {
        repo_external_id: String,
        branch: String,
        /// `None` for a new branch.
        before: Option<String>,
        /// `None` for a deleted branch.
        after: Option<String>,
        sender: ScmUser,
    },
    /// A check or a commit status of a commit changed.
    CheckChanged {
        repo_external_id: String,
        sha: String,
        /// The branches whose head is `sha`, when the SCM gives them: they
        /// route the check of a test branch to its queue.
        branches: Vec<String>,
        check: CheckReport,
    },
    /// Someone else than CIHub checked the submission box of a comment of
    /// CIHub on a PR.
    SubmitBoxChecked {
        repo_external_id: String,
        number: u64,
        comment_external_id: String,
        sender: ScmUser,
    },
    /// A comment on a PR, which can hold a command.
    Commented {
        repo_external_id: String,
        number: u64,
        comment_external_id: String,
        author: ScmUser,
        body: String,
    },
    /// The rules of the repository changed: the required checks can differ.
    RulesChanged { repo_external_id: String },
    /// The repository was renamed or deleted.
    RepoChanged { repo_external_id: String },
    /// The installation of the connection changed on the SCM: created,
    /// deleted, suspended, active again or with new permissions. CIHub reads
    /// the installation again.
    ConnectionChanged,
    /// The connection gained or lost repositories.
    ReposChanged {
        added: Vec<String>,
        removed: Vec<String>,
    },
    /// A delivery that CIHub does not use.
    Ignored,
}

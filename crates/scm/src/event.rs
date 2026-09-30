use cihub_domain::CheckReport;

use crate::ScmUser;

/// A webhook delivery, as the adapter reads it: the connection that received
/// it and what it tells (`AGENTS.md` D75).
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
    /// A comment on a PR, which can hold a command.
    Commented {
        repo_external_id: String,
        number: u64,
        author: ScmUser,
        body: String,
    },
    /// The rules of the repository changed: the required checks can differ.
    RulesChanged { repo_external_id: String },
    /// The repository was renamed or deleted.
    RepoChanged { repo_external_id: String },
    /// The connection was created on the SCM. `claim` is the value that
    /// links it to an organization, when the installation flow gives one.
    ConnectionCreated {
        account_external_id: String,
        account_path: String,
        claim: Option<String>,
    },
    /// The connection gained or lost repositories.
    ReposChanged {
        added: Vec<String>,
        removed: Vec<String>,
    },
    /// The connection is suspended or deleted.
    AccessLost,
    /// The connection is active again.
    AccessRestored,
    /// A delivery that CIHub does not use.
    Ignored,
}

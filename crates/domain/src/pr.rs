//! What CIHub reads of a PR on the SCM. A PR is any change request: a
//! GitHub pull request, a GitLab merge request. Each SCM adapter translates
//! its own values into these ones.

use serde::{Deserialize, Serialize};

use crate::{CheckConclusion, CheckKind};

closed_enum! {
    pub enum PrState {
        Open,
        Closed,
        Merged,
    }
}

closed_enum! {
    /// Whether the SCM accepts to merge the PR.
    pub enum Mergeability {
        /// Nothing on the SCM prevents the merge.
        Mergeable,
        /// The SCM requires the head to be up to date with the base.
        Behind,
        /// The PR conflicts with its base.
        Conflict,
        /// The SCM refuses the merge for another reason: a review, a
        /// required check, a draft.
        Blocked,
        /// The SCM has not computed it yet.
        Unknown,
    }
}

closed_enum! {
    /// The review decision of the PR.
    #[derive(Default)]
    pub enum ReviewDecision {
        Approved,
        ChangesRequested,
        ReviewRequired,
        /// No review rule applies.
        #[default]
        None,
    }
}

closed_enum! {
    /// What a user may do on a repository, from the lowest to the highest
    /// (`DECISIONS.md` D145).
    pub enum Permission {
        /// No access to the repository.
        None,
        /// Reads the repository.
        Read,
        /// Pushes to the repository.
        Write,
        /// Administers the repository.
        Admin,
    }
}

/// One check or commit status, as the SCM reports it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckReport {
    pub name: String,
    pub kind: CheckKind,
    /// `None` while the check has not concluded.
    pub conclusion: Option<CheckConclusion>,
    pub details_url: Option<String>,
}

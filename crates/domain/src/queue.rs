use serde::{Deserialize, Serialize};

use crate::{OrgId, QueueId, QueueMode, RepoId};

/// One queue per target branch of a repository. Settings are validated and
/// held by `cihub-config`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Queue {
    /// The queue.
    pub id: QueueId,
    /// The organization that owns the repository.
    pub org_id: OrgId,
    /// The repository of the target branch.
    pub repo_id: RepoId,
    /// The branch where the queue merges PRs; a queue is unique by its
    /// repository and this branch.
    pub target_branch: String,
    /// Whether the queue accepts and merges PRs.
    pub state: QueueState,
    /// Whether the queue orders every PR in one line or by impacted targets.
    pub mode: QueueMode,
}

closed_enum! {
    /// Whether a queue accepts and merges PRs. An operator changes it; a
    /// change of mode does too.
    pub enum QueueState {
        /// Accepts and merges.
        Running,
        /// Accepts nothing, merges nothing, keeps the order.
        Paused,
        /// Accepts nothing, lets in-flight items merge.
        Draining,
        /// Entered when the mode changes with items in flight; behaves as
        /// `Draining` until the queue is empty.
        SwitchingModes,
    }
}

closed_enum! {
    /// What an operator asks of a queue.
    pub enum QueueControl {
        /// Stop the queue: [`QueueState::Paused`].
        Pause,
        /// Start the queue again: [`QueueState::Running`].
        Resume,
        /// Let the items in flight merge and accept no new one:
        /// [`QueueState::Draining`].
        Drain,
    }
}

impl QueueControl {
    /// The queue state that the control asks for.
    pub const fn target(self) -> QueueState {
        match self {
            Self::Pause => QueueState::Paused,
            Self::Resume => QueueState::Running,
            Self::Drain => QueueState::Draining,
        }
    }
}

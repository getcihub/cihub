use serde::{Deserialize, Serialize};

use crate::{OrgId, QueueId, QueueMode, QueueState, RepoId};

/// One queue per target branch of a repository. Settings are validated and
/// held by `cihub-config`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Queue {
    pub id: QueueId,
    pub org_id: OrgId,
    pub repo_id: RepoId,
    pub target_branch: String,
    pub state: QueueState,
    pub mode: QueueMode,
}

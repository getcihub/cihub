use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{EventId, GroupId, ItemId, OrgId, QueueId, RunId, Target};

/// Immutable, append-only record of something that happened in a queue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub id: EventId,
    pub org_id: OrgId,
    pub queue_id: QueueId,
    pub item_id: Option<ItemId>,
    pub group_id: Option<GroupId>,
    pub run_id: Option<RunId>,
    pub actor: Actor,
    #[serde(with = "time::serde::rfc3339")]
    pub occurred_at: OffsetDateTime,
    pub kind: EventKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventKind {
    ItemStateChanged { to: Target },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Actor {
    /// CIHub itself: the reconciler, a job or the repair loop.
    System,
    User {
        login: String,
    },
}

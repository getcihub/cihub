//! The requests, the responses and the errors of the public API v1
//! (`specs/API.md`), shared by the server and the
//! clients. Pure: no I/O and no server code. The feature `openapi` derives
//! the `OpenAPI` schemas, for the server only.

use std::fmt;

use cihub_domain::{
    BlockReason, CheckConclusion, CheckKind, ConflictCulprit, ItemState, Priority, QueueMode,
    QueueState, Reason, RequiredStatusesSource, RunStatus,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;

/// The prefix of every route of the API.
pub const API_PREFIX: &str = "/api/v1";

/// The media type of an error.
pub const PROBLEM_JSON: &str = "application/problem+json";

/// The most items that a page of items gives.
pub const MAX_TAKE: u32 = 100;

/// The most PRs that a `prs:batchGet` reads.
pub const MAX_BATCH_GET: usize = 50;

/// The machine code of an error.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// No token, or a token that is unknown or expired.
    Unauthorized,
    /// The token lacks the scope of the route.
    Forbidden,
    NotFound,
    /// The request is not valid: its body, its path or its query.
    InvalidRequest,
    /// The token made too many requests: retry after `Retry-After`.
    RateLimited,
    /// The item is not in a state that allows the request.
    InvalidState,
    Internal,
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match serde_json::to_value(self) {
            Ok(Value::String(name)) => f.write_str(&name),
            _ => Err(fmt::Error),
        }
    }
}

/// An error, as `application/problem+json` (RFC 9457).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Problem {
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    pub status: u16,
    pub detail: String,
    pub code: ErrorCode,
}

/// A repository that the token sees.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Repo {
    pub id: String,
    pub namespace: String,
    pub name: String,
    pub default_branch: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RepoList {
    pub repos: Vec<Repo>,
}

/// A queue of a target branch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Queue {
    pub id: String,
    pub repo_id: String,
    pub name: String,
    pub target_branch: String,
    #[cfg_attr(feature = "openapi", schema(value_type = String))]
    pub state: QueueState,
    #[cfg_attr(feature = "openapi", schema(value_type = String))]
    pub mode: QueueMode,
    /// The head of the target whose configuration file the queue read last.
    pub config_sha: Option<String>,
    /// Why the last file read is not valid; the queue keeps the last valid
    /// one.
    pub config_error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct QueueList {
    pub queues: Vec<Queue>,
}

/// The current items of a queue in each state that is not terminal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ItemCounts {
    pub not_ready: u32,
    pub pending: u32,
    pub testing: u32,
    pub pending_failure: u32,
    pub tests_passed: u32,
}

/// A queue of the organization, with its counters.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct OrgQueue {
    /// The name of the repository of the queue, as the routes name it.
    pub repo: String,
    pub queue: Queue,
    pub counts: ItemCounts,
    /// The current items that reached `merged` in the 24 hours before the
    /// request.
    pub merged_last_24h: u32,
    /// The current items that reached `failed` in the 24 hours before the
    /// request.
    pub failed_last_24h: u32,
    /// When the last current item reached `merged`; `None` if no item merged.
    #[serde(with = "time::serde::rfc3339::option")]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<String>, format = DateTime))]
    pub last_merged_at: Option<OffsetDateTime>,
}

/// `GET /orgs/{org}/queues`: by repository name, then queue name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct OrgQueueList {
    pub queues: Vec<OrgQueue>,
}

/// The state that `PATCH /orgs/{org}/repos/{repo}/queues/{queue}` asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum RequestedQueueState {
    Running,
    Paused,
    Draining,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct UpdateQueue {
    pub state: RequestedQueueState,
}

/// A submission of a PR.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Item {
    pub id: String,
    pub queue_id: String,
    pub pr_number: u64,
    pub head_sha: String,
    #[cfg_attr(feature = "openapi", schema(value_type = String))]
    pub state: ItemState,
    #[cfg_attr(feature = "openapi", schema(value_type = String))]
    pub state_reason: Reason,
    /// The sentence of `state_reason`, in English, for a person; it can
    /// change without notice, unlike `state_reason`.
    pub state_description: String,
    #[serde(with = "time::serde::rfc3339")]
    #[cfg_attr(feature = "openapi", schema(value_type = String, format = DateTime))]
    pub state_changed_at: OffsetDateTime,
    /// Lower first.
    #[cfg_attr(feature = "openapi", schema(value_type = u8))]
    pub priority: Priority,
    /// The order in the queue; `None` before the admission.
    pub position: Option<i64>,
    pub is_current: bool,
    #[cfg_attr(feature = "openapi", schema(value_type = Vec<Object>))]
    pub block_reasons: Vec<BlockReason>,
    pub group_id: Option<String>,
    pub merged_sha: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    #[cfg_attr(feature = "openapi", schema(value_type = String, format = DateTime))]
    pub submitted_at: OffsetDateTime,
}

/// A page of items, newest submission first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ItemPage {
    pub items: Vec<Item>,
    /// The cursor of the next page; `None` on the last page.
    pub next_cursor: Option<String>,
}

/// The priority of a submission: a name or a value from 0 to 255.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(untagged)]
pub enum PriorityInput {
    Value(u8),
    Name(String),
}

impl PriorityInput {
    /// The priority, or `None` for an unknown name.
    pub fn priority(&self) -> Option<Priority> {
        match self {
            Self::Value(value) => Some(Priority(*value)),
            Self::Name(name) => Priority::from_name(name),
        }
    }
}

/// `POST /orgs/{org}/repos/{repo}/queues/{queue}/prs`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct SubmitPr {
    pub pr_number: u64,
    /// The `priority_rules` of the file decide without it.
    pub priority: Option<PriorityInput>,
    #[serde(default)]
    pub no_batch: bool,
    #[serde(default)]
    pub direct: bool,
}

/// A request that the queue does later: a submission reads the PR first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Accepted {
    pub accepted: bool,
}

/// An event of the log of a queue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct TimelineEvent {
    pub id: String,
    pub seq: i64,
    #[serde(rename = "type")]
    pub kind: String,
    pub actor_kind: String,
    #[serde(with = "time::serde::rfc3339")]
    #[cfg_attr(feature = "openapi", schema(value_type = String, format = DateTime))]
    pub occurred_at: OffsetDateTime,
    #[cfg_attr(feature = "openapi", schema(value_type = Object))]
    pub payload: Value,
}

/// `GET /orgs/{org}/repos/{repo}/queues/{queue}/prs/{number}`: the last item of the PR.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct PrItem {
    pub item: Item,
    /// With `?include=timeline`: the events of the item, in their order.
    pub timeline: Option<Vec<TimelineEvent>>,
}

/// `POST /orgs/{org}/repos/{repo}/queues/{queue}/prs:batchGet`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct BatchGet {
    pub pr_numbers: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct BatchGetResponse {
    pub items: Vec<Item>,
    pub not_found: Vec<u64>,
}

/// The merge graph: the live items, and the predecessors that their tests
/// merge before them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Graph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct GraphNode {
    pub item_id: String,
    pub pr_number: u64,
    #[cfg_attr(feature = "openapi", schema(value_type = String))]
    pub state: ItemState,
    pub position: Option<i64>,
}

/// The test of `to` merges `from` before it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
}

/// A check or a status observed on the test SHA of a run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RunCheck {
    pub name: String,
    #[cfg_attr(feature = "openapi", schema(value_type = String))]
    pub kind: CheckKind,
    pub required: bool,
    #[cfg_attr(feature = "openapi", schema(value_type = Option<String>))]
    pub conclusion: Option<CheckConclusion>,
    pub details_url: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    #[cfg_attr(feature = "openapi", schema(value_type = String, format = DateTime))]
    pub observed_at: OffsetDateTime,
}

/// A live run whose predecessors hold a PR of this run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Dependent {
    pub run_id: String,
    pub pr_numbers: Vec<u64>,
}

/// `GET /orgs/{org}/runs/{run_id}`, as `specs/API.md` fixes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct TestRun {
    pub id: String,
    pub queue_id: String,
    pub group_id: String,
    pub attempt: u32,
    #[cfg_attr(feature = "openapi", schema(value_type = String))]
    pub status: RunStatus,
    #[cfg_attr(feature = "openapi", schema(value_type = String))]
    pub status_reason: Reason,
    pub test_branch: String,
    pub test_sha: Option<String>,
    pub base_sha: String,
    pub draft_pr_number: Option<u64>,
    pub tested_pr_numbers: Vec<u64>,
    /// The heads merged in order: the predecessors, then the members.
    pub head_shas: Vec<String>,
    pub predecessor_pr_numbers: Vec<u64>,
    pub required_statuses: Vec<String>,
    #[cfg_attr(feature = "openapi", schema(value_type = Option<String>))]
    pub required_statuses_source: Option<RequiredStatusesSource>,
    #[serde(with = "time::serde::rfc3339")]
    #[cfg_attr(feature = "openapi", schema(value_type = String, format = DateTime))]
    pub started_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<String>, format = DateTime))]
    pub ended_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<String>, format = DateTime))]
    pub last_report_at: Option<OffsetDateTime>,
    #[cfg_attr(feature = "openapi", schema(value_type = Option<String>))]
    pub failure_reason: Option<Reason>,
    pub failure_causing_statuses: Vec<String>,
    #[cfg_attr(feature = "openapi", schema(value_type = Option<Object>))]
    pub conflict_culprit: Option<ConflictCulprit>,
    pub checks: Vec<RunCheck>,
    #[cfg_attr(feature = "openapi", schema(value_type = Option<Object>))]
    pub impacted_targets: Option<Value>,
    pub dependents: Vec<Dependent>,
}

/// An action of an admin, or a change of a queue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct AuditEntry {
    pub id: String,
    pub action: String,
    pub target: String,
    pub actor_user_id: Option<String>,
    #[cfg_attr(feature = "openapi", schema(value_type = Option<Object>))]
    pub before: Option<Value>,
    #[cfg_attr(feature = "openapi", schema(value_type = Option<Object>))]
    pub after: Option<Value>,
    #[serde(with = "time::serde::rfc3339")]
    #[cfg_attr(feature = "openapi", schema(value_type = String, format = DateTime))]
    pub occurred_at: OffsetDateTime,
}

/// A page of the audit log, newest first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct AuditPage {
    pub entries: Vec<AuditEntry>,
    pub next_cursor: Option<String>,
}

/// A point of the configuration of the SCM that the queue needs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Finding {
    pub code: String,
    pub message: String,
}

/// `GET /orgs/{org}/repos/{repo}/queues/{queue}/config-check`: no finding means a sound
/// configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ConfigCheck {
    pub findings: Vec<Finding>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_priority_is_a_name_or_a_value() {
        let read = |text: &str| serde_json::from_str::<PriorityInput>(text).unwrap();
        assert_eq!(read("42").priority(), Some(Priority(42)));
        assert_eq!(read("\"urgent\"").priority(), Some(Priority::URGENT));
        assert_eq!(read("\"soon\"").priority(), None);
        assert!(serde_json::from_str::<PriorityInput>("256").is_err());
    }

    #[test]
    fn a_problem_has_a_type_and_a_code() {
        let problem = Problem {
            kind: "about:blank".into(),
            title: "Not Found".into(),
            status: 404,
            detail: "no queue".into(),
            code: ErrorCode::NotFound,
        };
        let json = serde_json::to_value(&problem).unwrap();
        assert_eq!(json["type"], "about:blank");
        assert_eq!(json["code"], "not_found");
        assert_eq!(ErrorCode::InvalidState.to_string(), "invalid_state");
    }
}

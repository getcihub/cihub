//! The requests, the responses and the errors of the public API v1
//! (`specs/API.md`), shared by the server and the
//! clients. Pure: no I/O and no server code. The feature `openapi` derives
//! the `OpenAPI` schemas, for the server only.

use cihub_domain::{
    BlockReason, CheckConclusion, CheckKind, ConflictCulprit, ItemState, MembershipRole,
    MergeMethod, Priority, QueueMode, QueueState, Reason, RequiredStatusesSource, RunStatus,
    Strategy, SubmittedVia,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;

/// The prefix of every route of the API.
pub const API_PREFIX: &str = "/api/v1";

/// The media type of an error.
pub const PROBLEM_JSON: &str = "application/problem+json";

/// The size of a page when a request does not give its `limit`.
pub const DEFAULT_LIMIT: u32 = 50;

/// The largest `limit` of a page.
pub const MAX_LIMIT: u32 = 100;

/// A page of a list: its entries, then the fields that every list shares.
macro_rules! page {
    (
        $(#[$doc:meta])*
        pub struct $name:ident {
            $(#[$entries_doc:meta])*
            pub $entries:ident: Vec<$entry:ty>,
        }
    ) => {
        $(#[$doc])*
        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
        #[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
        pub struct $name {
            $(#[$entries_doc])*
            pub $entries: Vec<$entry>,
            /// More entries follow this page.
            pub has_more: bool,
            /// How many entries the list has, every page together.
            #[cfg_attr(feature = "openapi", schema(example = 137))]
            pub count: u64,
            /// The `from` of the next page; `null` on the last page.
            #[cfg_attr(
                feature = "openapi",
                schema(example = "eyJsaXN0IjoicHJzIiwiYWZ0ZXIiOlsiMjAyNi0xMC0xMVQwOTozMDowMFoiXX0")
            )]
            pub cursor: Option<String>,
        }
    };
}

/// The `OpenAPI` schemas of the closed enums of the domain, which does not
/// derive them: a component each, whose values are those of the enum.
#[cfg(feature = "openapi")]
mod closed {
    use std::borrow::Cow;

    use utoipa::openapi::RefOr;
    use utoipa::openapi::schema::{ObjectBuilder, Schema, Type};

    macro_rules! closed_schemas {
        ($($schema:ident),+ $(,)?) => {$(
            pub(crate) struct $schema;

            impl utoipa::PartialSchema for $schema {
                fn schema() -> RefOr<Schema> {
                    let values = <cihub_domain::$schema as strum::IntoEnumIterator>::iter()
                        .map(cihub_domain::$schema::as_str);
                    ObjectBuilder::new()
                        .schema_type(Type::String)
                        .enum_values(Some(values))
                        .build()
                        .into()
                }
            }

            impl utoipa::ToSchema for $schema {
                fn name() -> Cow<'static, str> {
                    Cow::Borrowed(stringify!($schema))
                }
            }
        )+};
    }

    closed_schemas!(
        ActorKind,
        CheckConclusion,
        CheckKind,
        EventType,
        ItemState,
        MembershipRole,
        MergeMethod,
        QueueMode,
        QueueState,
        Reason,
        RequiredStatusesSource,
        RunStatus,
        Strategy,
        SubmittedVia,
    );
}

/// The machine code of an error: a client decides on it, never on
/// `detail`. Each code has one HTTP status.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, strum::EnumIter, strum::Display,
)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum ErrorCode {
    InvalidRequest,
    LimitOutOfRange,
    InvalidCursor,
    UnknownPriority,
    MeNeedsSession,
    QueueNeedsRepo,
    Unauthorized,
    ScopeMissing,
    CsrfRefused,
    RouteNotFound,
    OrgNotFound,
    RepoNotFound,
    QueueNotFound,
    PrNotFound,
    RunNotFound,
    QueueNotAccepting,
    PrAlreadyQueued,
    ItemEnded,
    ItemNotInTest,
    RateLimited,
    TooManyStreams,
    Internal,
}

impl ErrorCode {
    pub fn all() -> impl Iterator<Item = Self> {
        <Self as strum::IntoEnumIterator>::iter()
    }

    /// The HTTP status of an error of this code.
    pub const fn status(self) -> u16 {
        match self {
            Self::InvalidRequest
            | Self::LimitOutOfRange
            | Self::InvalidCursor
            | Self::UnknownPriority
            | Self::MeNeedsSession
            | Self::QueueNeedsRepo => 400,
            Self::Unauthorized => 401,
            Self::ScopeMissing | Self::CsrfRefused => 403,
            Self::RouteNotFound
            | Self::OrgNotFound
            | Self::RepoNotFound
            | Self::QueueNotFound
            | Self::PrNotFound
            | Self::RunNotFound => 404,
            Self::QueueNotAccepting
            | Self::PrAlreadyQueued
            | Self::ItemEnded
            | Self::ItemNotInTest => 409,
            Self::RateLimited | Self::TooManyStreams => 429,
            Self::Internal => 500,
        }
    }

    /// What the code means, in English, for a person.
    pub const fn sentence(self) -> &'static str {
        match self {
            Self::InvalidRequest => "The body, the path or the query of the request is not valid.",
            Self::LimitOutOfRange => "`limit` is from 1 to 100.",
            Self::InvalidCursor => "`from` is not a `cursor` that this list gave.",
            Self::UnknownPriority => {
                "The name of the priority is not `urgent`, `high`, `medium` or `low`."
            }
            Self::MeNeedsSession => {
                "`@me` names the person of a session; an API token stands for no person."
            }
            Self::QueueNeedsRepo => "`queue` names a queue of the repository that `repo` names.",
            Self::Unauthorized => "No API token or session, or one that is unknown or expired.",
            Self::ScopeMissing => {
                "The token, or the role of the user of the session, lacks the scope of the route."
            }
            Self::CsrfRefused => {
                "A request of a session that can change a state lacks the header `X-CSRF-Token`, \
                 or carries another token than the cookie `__Host-cihub_csrf`."
            }
            Self::RouteNotFound => "No route has this path.",
            Self::OrgNotFound => {
                "No organization has this slug, or the token or the user does not belong to it."
            }
            Self::RepoNotFound => "The organization has no repository with this name.",
            Self::QueueNotFound => "The repository has no queue with this name.",
            Self::PrNotFound => "The PR was never submitted to a queue of the repository.",
            Self::RunNotFound => "The organization has no run with this identifier.",
            Self::QueueNotAccepting => {
                "The queue is `draining` or `switching_modes`: it accepts no new PR."
            }
            Self::PrAlreadyQueued => {
                "The PR is already in a queue of the repository, and its item has not ended."
            }
            Self::ItemEnded => "The item of the PR ended: `merged`, `failed` or `cancelled`.",
            Self::ItemNotInTest => {
                "Only an item in `testing` or `pending_failure` can restart its tests."
            }
            Self::RateLimited => {
                "The token or the session made 600 requests in the minute: retry after \
                 `Retry-After` seconds."
            }
            Self::TooManyStreams => "The token or the person already keeps 8 streams open.",
            Self::Internal => "The server failed; the logs of the instance say why.",
        }
    }
}

/// An error, as `application/problem+json` (RFC 9457).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Problem {
    /// Always `about:blank`: `code` tells the errors apart.
    #[serde(rename = "type")]
    #[cfg_attr(feature = "openapi", schema(example = "about:blank"))]
    pub kind: String,
    /// The reason phrase of the HTTP status.
    #[cfg_attr(feature = "openapi", schema(example = "Not Found"))]
    pub title: String,
    /// The HTTP status of the answer.
    #[cfg_attr(feature = "openapi", schema(example = 404))]
    pub status: u16,
    /// What went wrong with this request, in English, for a person; it can
    /// change without notice, unlike `code`.
    #[cfg_attr(feature = "openapi", schema(example = "queue `main` not found"))]
    pub detail: String,
    /// The machine code of the error: decide on it.
    pub code: ErrorCode,
}

/// `GET /user`: the person of a session of the dashboard.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct User {
    /// The login of the person on the SCM.
    #[cfg_attr(feature = "openapi", schema(example = "ada"))]
    pub login: String,
    /// The name that the person gives on the SCM, `null` when they give none.
    #[cfg_attr(feature = "openapi", schema(example = "Ada Lovelace"))]
    pub name: Option<String>,
    /// The address of the image of the person on the SCM, `null` when the SCM
    /// gives none.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "https://avatars.example.com/u/1")
    )]
    pub avatar_url: Option<String>,
}

/// `GET /instance`: the instance of CIHub.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Instance {
    /// The version of the instance.
    #[cfg_attr(feature = "openapi", schema(example = "0.1.0"))]
    pub version: String,
    /// The page of the SCM that installs the App on an account, or chooses
    /// the repositories of its installation; `null` when the SCM did not
    /// answer.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "https://scm.example.com/apps/cihub/installations/new")
    )]
    pub app_install_url: Option<String>,
}

/// An organization: the account of an installation of the App.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Org {
    /// The slug of the organization, as the routes name it.
    #[cfg_attr(feature = "openapi", schema(example = "acme"))]
    pub slug: String,
    /// The role of the person of the session: `admin` has every scope,
    /// `member` has `queues:read` and `prs:write`; `null` for an API token.
    #[cfg_attr(feature = "openapi", schema(value_type = Option<closed::MembershipRole>))]
    pub role: Option<MembershipRole>,
}

page! {
    /// A page of the organizations of the person, by slug.
    pub struct OrgList {
        /// The organizations of the page.
        pub orgs: Vec<Org>,
    }
}

/// A repository of the organization.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Repo {
    /// The identifier of the repository.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "repo_06cx8fn2w6gkyc9x5csgnm5r1dmqz3s0")
    )]
    pub id: String,
    /// The account of the repository on the SCM.
    #[cfg_attr(feature = "openapi", schema(example = "acme"))]
    pub namespace: String,
    /// The name of the repository, as the routes name it.
    #[cfg_attr(feature = "openapi", schema(example = "web"))]
    pub name: String,
    /// The default branch of the repository.
    #[cfg_attr(feature = "openapi", schema(example = "main"))]
    pub default_branch: String,
}

page! {
    /// A page of repositories, by namespace then name.
    pub struct RepoList {
        /// The repositories of the page.
        pub repos: Vec<Repo>,
    }
}

/// A queue of a target branch, with its counters and its settings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Queue {
    /// The identifier of the queue.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "queue_06cx8fn2wagkyc9x5csgnm5r1gz34p81")
    )]
    pub id: String,
    /// The identifier of the repository of the queue.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "repo_06cx8fn2w6gkyc9x5csgnm5r1dmqz3s0")
    )]
    pub repo_id: String,
    /// The name of the repository of the queue, as the routes name it.
    #[cfg_attr(feature = "openapi", schema(example = "web"))]
    pub repo: String,
    /// The name of the queue in its repository, as the routes name it.
    #[cfg_attr(feature = "openapi", schema(example = "main"))]
    pub name: String,
    /// The branch that the queue merges into.
    #[cfg_attr(feature = "openapi", schema(example = "main"))]
    pub target_branch: String,
    /// `running` accepts and merges; `paused` tests and merges nothing and
    /// keeps the order, and a submission waits for the resume; `draining`
    /// accepts nothing and lets the items in flight merge; `switching_modes`
    /// behaves as `draining` until the queue is empty.
    #[cfg_attr(feature = "openapi", schema(value_type = closed::QueueState))]
    pub state: QueueState,
    /// `single` tests every PR on top of all those ahead of it; `parallel`
    /// only on top of those that change the same targets.
    #[cfg_attr(feature = "openapi", schema(value_type = closed::QueueMode))]
    pub mode: QueueMode,
    /// When the queue reached its state; its creation before a first change.
    #[serde(with = "time::serde::rfc3339")]
    #[cfg_attr(
        feature = "openapi",
        schema(value_type = String, format = DateTime, example = "2026-10-11T09:42:00Z")
    )]
    pub state_changed_at: OffsetDateTime,
    /// The login of the person who asked for the state; `null` when CIHub
    /// decided it, or when an API token asked for it.
    #[cfg_attr(feature = "openapi", schema(example = "ada"))]
    pub state_changed_by: Option<String>,
    /// The strategy of the last valid configuration file.
    #[cfg_attr(feature = "openapi", schema(value_type = closed::Strategy))]
    pub strategy: Strategy,
    /// The head of the target branch that the queue saw last; `null` before
    /// the first.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "9f2c4e1a7b3d5f60812a4c6e8b0d2f4a6c8e0b1d")
    )]
    pub target_head_sha: Option<String>,
    /// Whether CIHub reaches the repository of the queue on the SCM.
    pub access: QueueAccess,
    /// The target branch has a configuration file, valid or not; without
    /// it, the queue takes the defaults.
    pub has_config_file: bool,
    /// The head of the target whose configuration file the queue read last.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "9f2c4e1a7b3d5f60812a4c6e8b0d2f4a6c8e0b1d")
    )]
    pub config_sha: Option<String>,
    /// Why the last file read is not valid; the queue keeps the last valid
    /// one.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "line 4: `batching.size` is from 1 to 100")
    )]
    pub config_error: Option<String>,
    /// The current items of the queue in each state that is not terminal.
    pub counts: ItemCounts,
    /// The current items that reached `merged` in the 24 hours before the
    /// request.
    #[cfg_attr(feature = "openapi", schema(example = 17))]
    pub merged_last_24h: u32,
    /// The current items that reached `failed` in the 24 hours before the
    /// request.
    #[cfg_attr(feature = "openapi", schema(example = 1))]
    pub failed_last_24h: u32,
    /// The current items that reached `failed` or `cancelled` in the 24 hours
    /// before the request, as the list of the PRs gives them.
    #[cfg_attr(feature = "openapi", schema(example = 2))]
    pub failed_or_cancelled_last_24h: u32,
    /// When the last current item reached `merged`; `null` if no item merged.
    #[serde(with = "time::serde::rfc3339::option")]
    #[cfg_attr(
        feature = "openapi",
        schema(value_type = Option<String>, format = DateTime, example = "2026-10-11T09:42:00Z")
    )]
    pub last_merged_at: Option<OffsetDateTime>,
    /// The runs in progress.
    #[cfg_attr(feature = "openapi", schema(example = 2))]
    pub live_runs: u32,
    /// The median duration of the runs that succeeded in the last 24 hours,
    /// in seconds; `null` without such a run.
    #[cfg_attr(feature = "openapi", schema(example = 540))]
    pub median_run_seconds: Option<u32>,
    /// How many runs the queue keeps in progress.
    pub window: Window,
    /// The settings of the last valid configuration file.
    pub settings: SettingsSummary,
    /// The last check of the configuration of the SCM that the queue needs.
    pub config_check: ConfigCheck,
}

/// Whether CIHub reaches the repository of a queue on the SCM: `ok`, or
/// `suspended` when the App is suspended or uninstalled, and the queue is
/// paused until the access comes back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum QueueAccess {
    Ok,
    Suspended,
}

page! {
    /// A page of queues, by repository name then queue name.
    pub struct QueueList {
        /// The queues of the page.
        pub queues: Vec<Queue>,
    }
}

/// The current items of a queue in each state that is not terminal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ItemCounts {
    /// Submitted, and blocked by a condition of the PR.
    #[cfg_attr(feature = "openapi", schema(example = 2))]
    pub not_ready: u32,
    /// Admitted, and waiting for a run.
    #[cfg_attr(feature = "openapi", schema(example = 3))]
    pub pending: u32,
    /// In a run in progress.
    #[cfg_attr(feature = "openapi", schema(example = 4))]
    pub testing: u32,
    /// In a run that failed, while the queue learns who is at fault.
    #[cfg_attr(feature = "openapi", schema(example = 0))]
    pub pending_failure: u32,
    /// Proved by a run, and waiting for the merge.
    #[cfg_attr(feature = "openapi", schema(example = 1))]
    pub tests_passed: u32,
}

/// The speculation window: how many runs the queue keeps in progress.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Window {
    /// The size of the window now, from `floor` to `ceiling`.
    #[cfg_attr(feature = "openapi", schema(example = 4))]
    pub current: u32,
    /// The smallest size of the window.
    #[cfg_attr(feature = "openapi", schema(example = 1))]
    pub floor: u32,
    /// The largest size of the window.
    #[cfg_attr(feature = "openapi", schema(example = 8))]
    pub ceiling: u32,
}

/// The settings of a queue that its page shows, from its last valid
/// configuration file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct SettingsSummary {
    /// The queue tests several PRs in one run.
    pub batching_enabled: bool,
    /// The most PRs in one batch.
    #[cfg_attr(feature = "openapi", schema(example = 4))]
    pub batching_size: u32,
    /// How long a batch that is not full waits for more PRs, in minutes.
    #[cfg_attr(feature = "openapi", schema(example = 5))]
    pub batching_max_wait_minutes: u32,
    /// How many times a run that fails is tested again before its PRs fail.
    #[cfg_attr(feature = "openapi", schema(example = 1))]
    pub failure_budget_retries: u32,
    /// How the queue merges a PR into the target branch.
    #[cfg_attr(feature = "openapi", schema(value_type = closed::MergeMethod))]
    pub merge_method: MergeMethod,
    /// The label that submits a PR when someone adds it; `null` when the
    /// labels do not submit (`label_commands.enabled: false`).
    #[cfg_attr(feature = "openapi", schema(example = "cihub-merge-queue-submit"))]
    pub enqueue_label: Option<String>,
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

/// `PATCH /orgs/{org}/repos/{repo}/queues/{queue}`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct UpdateQueue {
    /// `running` resumes the queue; `paused` stops its tests and its merges
    /// and keeps its order; `draining` lets the items in flight merge and
    /// refuses new ones.
    pub state: RequestedQueueState,
}

/// A PR in a queue: its last submission. A PR submitted again gets a new
/// submission, whose history its events keep.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Pr {
    /// The identifier of the submission.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "item_06cx8fn2wegkyc9x5csgnm5r1ngcahzx")
    )]
    pub id: String,
    /// The identifier of the queue of the item.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "queue_06cx8fn2wagkyc9x5csgnm5r1gz34p81")
    )]
    pub queue_id: String,
    /// The name of the repository of the queue, as the routes name it.
    #[cfg_attr(feature = "openapi", schema(example = "web"))]
    pub repo: String,
    /// The name of the queue, as the routes name it.
    #[cfg_attr(feature = "openapi", schema(example = "main"))]
    pub queue: String,
    /// The number of the PR in its repository.
    #[cfg_attr(feature = "openapi", schema(example = 118))]
    pub number: u64,
    /// The title at the last read of the PR; `null` before the first.
    #[cfg_attr(feature = "openapi", schema(example = "Export invoices as CSV"))]
    pub title: Option<String>,
    /// The login of the author of the PR.
    #[cfg_attr(feature = "openapi", schema(example = "ada"))]
    pub author: String,
    /// The head of the PR that the item tests.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "3b5d7f91a2c4e6081b3d5f7a9c1e3b5d7f9a1c3e")
    )]
    pub head_sha: String,
    /// `not_ready` waits for a condition of the PR; `pending` waits for a run;
    /// `testing` is in a run; `pending_failure` is in a run that failed, while
    /// the queue learns who is at fault; `tests_passed` waits for the merge;
    /// `merged`, `failed` and `cancelled` end the item.
    #[cfg_attr(feature = "openapi", schema(value_type = closed::ItemState))]
    pub state: ItemState,
    /// Why the item reached its state: a code of the closed vocabulary
    /// (`specs/DOMAIN.md`). A client decides on it.
    #[cfg_attr(feature = "openapi", schema(value_type = closed::Reason))]
    pub state_reason: Reason,
    /// The sentence of `state_reason`, in English, for a person; it can
    /// change without notice, unlike `state_reason`.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "all the required checks passed")
    )]
    pub state_description: String,
    /// When the item reached its state.
    #[serde(with = "time::serde::rfc3339")]
    #[cfg_attr(
        feature = "openapi",
        schema(value_type = String, format = DateTime, example = "2026-10-11T09:42:00Z")
    )]
    pub state_changed_at: OffsetDateTime,
    /// From 0 to 255, lower first: `urgent` is 0, `high` 10, `medium` 100,
    /// `low` 200.
    #[cfg_attr(feature = "openapi", schema(value_type = u8, example = 100))]
    pub priority: Priority,
    /// The order in the queue, lower first; `null` before the admission.
    #[cfg_attr(feature = "openapi", schema(example = 1000))]
    pub position: Option<i64>,
    /// What keeps a `not_ready` item out of the queue. Each one has a `kind`:
    /// `review_required` (`have`, `need`), `changes_requested`,
    /// `failing_required_check` (`check`), `awaiting_required_check`
    /// (`check`, `status`), `behind_base`, `merge_conflict`,
    /// `stack_needs_rebase` (`onto_pr_number`), `undetermined`, or
    /// `queue_conditions_unmet` (`conditions`).
    #[cfg_attr(
        feature = "openapi",
        schema(value_type = Vec<Object>, example = json!([{ "kind": "review_required", "have": 0, "need": 1 }]))
    )]
    pub block_reasons: Vec<BlockReason>,
    /// The identifier of the test group of the item; `null` outside a group.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "group_06cx8fn2wpgkyc9x5csgnm5r1ygtt5k0")
    )]
    pub group_id: Option<String>,
    /// The last run of the group of the item: the run that tests it, or
    /// that tested it last; `null` outside a group.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "run_06cx8fn2wtgkyc9x5csgnm5r22ca6g0w")
    )]
    pub run_id: Option<String>,
    /// The commit that the merge of the PR created on the target branch;
    /// `null` before the merge.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "c4e6081b3d5f7a9c1e3b5d7f9a1c3e5b7d9f1a3c")
    )]
    pub merged_sha: Option<String>,
    /// The PR is tested alone, never in a batch.
    pub no_batch: bool,
    /// The login of the person who submitted the PR; `null` when CIHub or an
    /// API token did.
    #[cfg_attr(feature = "openapi", schema(example = "ada"))]
    pub submitted_by: Option<String>,
    /// How the PR came into the queue: a `/cihub merge` comment, a label,
    /// or the API.
    #[cfg_attr(feature = "openapi", schema(value_type = closed::SubmittedVia))]
    pub submitted_via: SubmittedVia,
    /// When the PR was submitted.
    #[serde(with = "time::serde::rfc3339")]
    #[cfg_attr(
        feature = "openapi",
        schema(value_type = String, format = DateTime, example = "2026-10-11T09:30:00Z")
    )]
    pub submitted_at: OffsetDateTime,
    /// The admitted PRs that merge before it, the head of the queue first;
    /// empty for a merged item. For an item not admitted yet, every admitted
    /// PR: a PR of a higher priority goes before some of them.
    pub ahead: Vec<PrAhead>,
    /// About how many seconds until it merges, from the median duration of
    /// the runs of its queue; `null` when it does not move toward the merge,
    /// or when its queue has no run that succeeded in the last 24 hours.
    #[cfg_attr(feature = "openapi", schema(example = 1080))]
    pub merges_in_seconds: Option<u32>,
}

/// The event `queue_changed` of `GET /orgs/{org}/stream`: what the pages of
/// the queue show changed, and they read it again.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct QueueChanged {
    /// The identifier of the queue that changed.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "queue_06cx8fn2wagkyc9x5csgnm5r1gz34p81")
    )]
    pub queue_id: String,
    /// The sequence number of the last event of the log of the queue; 0
    /// before the first.
    #[cfg_attr(feature = "openapi", schema(example = 4182))]
    pub seq: i64,
}

/// A PR ahead of another one in its queue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct PrAhead {
    /// The number of the PR in its repository.
    #[cfg_attr(feature = "openapi", schema(example = 117))]
    pub pr_number: u64,
    /// The state of its item.
    #[cfg_attr(feature = "openapi", schema(value_type = closed::ItemState))]
    pub state: ItemState,
}

page! {
    /// A page of PRs.
    pub struct PrList {
        /// The PRs of the page.
        pub prs: Vec<Pr>,
    }
}

/// The priority of a submission: a name (`urgent`, `high`, `medium`, `low`)
/// or a value from 0 to 255, lower first.
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

/// `POST /orgs/{org}/repos/{repo}/prs/{number}/submit`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct SubmitPr {
    /// The queue of the repository; without it, the base branch of the PR
    /// chooses, as for a command in a comment.
    #[cfg_attr(feature = "openapi", schema(example = "main"))]
    #[serde(default)]
    pub queue: Option<String>,
    /// Without it, the `priority_rules` of the configuration file decide.
    #[cfg_attr(feature = "openapi", schema(example = "high"))]
    #[serde(default)]
    pub priority: Option<PriorityInput>,
    /// Test the PR alone, never in a batch.
    #[serde(default)]
    pub no_batch: bool,
    /// Merge the PR without a test of the queue; the person needs the
    /// permission `admin` on the repository.
    #[serde(default)]
    pub direct: bool,
}

/// An event of the log of a queue about a PR.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Event {
    /// The identifier of the event.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "event_06cx8fn2x2gkyc9x5csgnm5r2b89wbcv")
    )]
    pub id: String,
    /// The sequence number of the event in the log of the queue.
    #[cfg_attr(feature = "openapi", schema(example = 4182))]
    pub seq: i64,
    /// The submission of the PR that the event concerns.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "item_06cx8fn2wegkyc9x5csgnm5r1ngcahzx")
    )]
    pub item_id: String,
    /// The type of the event, `<subject>.<verb>`.
    #[serde(rename = "type")]
    #[cfg_attr(feature = "openapi", schema(value_type = closed::EventType))]
    pub kind: String,
    /// `system` for CIHub, `bot` for the account that CIHub acts with on the
    /// SCM, `user` for a person.
    #[cfg_attr(feature = "openapi", schema(value_type = closed::ActorKind))]
    pub actor_kind: String,
    /// The login of the person who acted, or whose request caused the
    /// decision; `null` when CIHub or an API token did.
    #[cfg_attr(feature = "openapi", schema(example = "ada"))]
    pub actor: Option<String>,
    /// When the event happened.
    #[serde(with = "time::serde::rfc3339")]
    #[cfg_attr(
        feature = "openapi",
        schema(value_type = String, format = DateTime, example = "2026-10-11T09:42:00Z")
    )]
    pub occurred_at: OffsetDateTime,
    /// What happened, in English, for a person, where a check or a condition
    /// is a Markdown code span; it can change without notice, unlike `type`
    /// and `payload`.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "all the required checks passed")
    )]
    pub description: String,
    /// The facts of the event, whose fields depend on `type`.
    #[cfg_attr(feature = "openapi", schema(value_type = Object))]
    pub payload: Value,
}

page! {
    /// A page of the events of a PR, oldest first.
    pub struct EventList {
        /// The events of the page.
        pub events: Vec<Event>,
    }
}

/// The merge graph: the head of the target branch, the live items, the
/// predecessors that their tests merge before them, and the live runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Graph {
    /// The head of the target branch that the queue saw last; `null` before
    /// the first.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "9f2c4e1a7b3d5f60812a4c6e8b0d2f4a6c8e0b1d")
    )]
    pub target_head_sha: Option<String>,
    /// The live items, in the order of the queue.
    pub nodes: Vec<GraphNode>,
    /// An edge from each predecessor of a live item, which its test merges
    /// before it.
    pub edges: Vec<GraphEdge>,
    /// The last run of each live group, the oldest first.
    pub runs: Vec<GraphRun>,
}

/// A live item of the graph.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct GraphNode {
    /// The identifier of the item.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "item_06cx8fn2wegkyc9x5csgnm5r1ngcahzx")
    )]
    pub item_id: String,
    /// The number of the PR in its repository.
    #[cfg_attr(feature = "openapi", schema(example = 118))]
    pub pr_number: u64,
    /// The state of the item.
    #[cfg_attr(feature = "openapi", schema(value_type = closed::ItemState))]
    pub state: ItemState,
    /// The order in the queue, lower first; `null` before the admission.
    #[cfg_attr(feature = "openapi", schema(example = 1000))]
    pub position: Option<i64>,
}

/// A run of the graph: the items that it tests, on top of their
/// predecessors.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct GraphRun {
    /// The identifier of the run.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "run_06cx8fn2wtgkyc9x5csgnm5r22ca6g0w")
    )]
    pub run_id: String,
    /// The items that the run tests.
    #[cfg_attr(
        feature = "openapi",
        schema(example = json!(["item_06cx8fn2wegkyc9x5csgnm5r1ngcahzx"]))
    )]
    pub item_ids: Vec<String>,
    /// The items that the test branch merges before them.
    #[cfg_attr(
        feature = "openapi",
        schema(example = json!(["item_06cx8fn2wjgkyc9x5csgnm5r1v8fe0y4"]))
    )]
    pub predecessor_ids: Vec<String>,
    /// From 1; a run tested again within the failure budget has a higher
    /// attempt.
    #[cfg_attr(feature = "openapi", schema(example = 1))]
    pub attempt: u32,
    /// The status of the run.
    #[cfg_attr(feature = "openapi", schema(value_type = closed::RunStatus))]
    pub status: RunStatus,
    /// When the run started.
    #[serde(with = "time::serde::rfc3339")]
    #[cfg_attr(
        feature = "openapi",
        schema(value_type = String, format = DateTime, example = "2026-10-11T09:33:00Z")
    )]
    pub started_at: OffsetDateTime,
}

/// The test of `to` merges `from` before it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct GraphEdge {
    /// The identifier of the predecessor.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "item_06cx8fn2wjgkyc9x5csgnm5r1v8fe0y4")
    )]
    pub from: String,
    /// The identifier of the item whose test merges `from` first.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "item_06cx8fn2wegkyc9x5csgnm5r1ngcahzx")
    )]
    pub to: String,
}

/// A check or a status observed on the test SHA of a run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RunCheck {
    /// The name of the check or of the status.
    #[cfg_attr(feature = "openapi", schema(example = "ci / test"))]
    pub name: String,
    /// `check` for a check with a run, `status` for a commit status.
    #[cfg_attr(feature = "openapi", schema(value_type = closed::CheckKind))]
    pub kind: CheckKind,
    /// The run needs it to pass.
    pub required: bool,
    /// The outcome; `null` while the check runs.
    #[cfg_attr(feature = "openapi", schema(value_type = Option<closed::CheckConclusion>))]
    pub conclusion: Option<CheckConclusion>,
    /// The page of the check on the CI.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "https://ci.example.com/acme/web/runs/1234")
    )]
    pub details_url: Option<String>,
    /// When CIHub saw this report last.
    #[serde(with = "time::serde::rfc3339")]
    #[cfg_attr(
        feature = "openapi",
        schema(value_type = String, format = DateTime, example = "2026-10-11T09:40:00Z")
    )]
    pub observed_at: OffsetDateTime,
}

/// A live run whose predecessors hold a PR of this run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Dependent {
    /// The identifier of the live run.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "run_06cx8fn2wygkyc9x5csgnm5r24q2nbta")
    )]
    pub run_id: String,
    /// The PRs that it tests.
    #[cfg_attr(feature = "openapi", schema(example = json!([119])))]
    pub pr_numbers: Vec<u64>,
}

/// A run: one test, on the CI, of a test branch that CIHub assembles from
/// the head of the target branch, the PRs ahead (the predecessors), then the
/// PRs that it tests. The name of the test branch ends with the identifier of
/// the run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Run {
    /// The identifier of the run.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "run_06cx8fn2wtgkyc9x5csgnm5r22ca6g0w")
    )]
    pub id: String,
    /// The identifier of the queue of the run.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "queue_06cx8fn2wagkyc9x5csgnm5r1gz34p81")
    )]
    pub queue_id: String,
    /// The identifier of the test group that the run tests.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "group_06cx8fn2wpgkyc9x5csgnm5r1ygtt5k0")
    )]
    pub group_id: String,
    /// From 1; a run tested again within the failure budget has a higher
    /// attempt.
    #[cfg_attr(feature = "openapi", schema(example = 1))]
    pub attempt: u32,
    /// `assembling` while CIHub builds the test branch, `in_progress` while the
    /// CI runs; then `succeeded`, `failed`, `timed_out`, `interrupted` when the
    /// queue replaced the run, or `cancelled` when a member was cancelled.
    #[cfg_attr(feature = "openapi", schema(value_type = closed::RunStatus))]
    pub status: RunStatus,
    /// Why the run reached its status: a code of the closed vocabulary
    /// (`specs/DOMAIN.md`).
    #[cfg_attr(feature = "openapi", schema(value_type = closed::Reason))]
    pub status_reason: Reason,
    /// The branch that the run tests, which CIHub assembles.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "cihub-merge/pr-118/run_06cx8fn2wtgkyc9x5csgnm5r22ca6g0w")
    )]
    pub test_branch: String,
    /// The commit that the run tests; `null` while CIHub assembles it.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "e6081b3d5f7a9c1e3b5d7f9a1c3e5b7d9f1a3c5e")
    )]
    pub test_sha: Option<String>,
    /// The head of the target branch that the test branch starts from.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "9f2c4e1a7b3d5f60812a4c6e8b0d2f4a6c8e0b1d")
    )]
    pub base_sha: String,
    /// The draft PR that runs the CI of the test branch, when the
    /// configuration asks for one.
    #[cfg_attr(feature = "openapi", schema(example = 412))]
    pub draft_pr_number: Option<u64>,
    /// The PRs that the run tests.
    #[cfg_attr(feature = "openapi", schema(example = json!([118])))]
    pub tested_pr_numbers: Vec<u64>,
    /// The heads merged in order: the predecessors, then the members.
    #[cfg_attr(
        feature = "openapi",
        schema(example = json!(["3b5d7f91a2c4e6081b3d5f7a9c1e3b5d7f9a1c3e"]))
    )]
    pub head_shas: Vec<String>,
    /// The PRs that the test branch merges before the tested ones.
    #[cfg_attr(feature = "openapi", schema(example = json!([117])))]
    pub predecessor_pr_numbers: Vec<u64>,
    /// The checks and statuses that the run needs to pass.
    #[cfg_attr(feature = "openapi", schema(example = json!(["ci / test"])))]
    pub required_statuses: Vec<String>,
    /// Where `required_statuses` come from: the configuration file, or the
    /// protection of the target branch on the SCM.
    #[cfg_attr(
        feature = "openapi",
        schema(value_type = Option<closed::RequiredStatusesSource>)
    )]
    pub required_statuses_source: Option<RequiredStatusesSource>,
    /// When the run started.
    #[serde(with = "time::serde::rfc3339")]
    #[cfg_attr(
        feature = "openapi",
        schema(value_type = String, format = DateTime, example = "2026-10-11T09:33:00Z")
    )]
    pub started_at: OffsetDateTime,
    /// When the run ended; `null` while it runs.
    #[serde(with = "time::serde::rfc3339::option")]
    #[cfg_attr(
        feature = "openapi",
        schema(value_type = Option<String>, format = DateTime, example = "2026-10-11T09:42:00Z")
    )]
    pub ended_at: Option<OffsetDateTime>,
    /// When CIHub saw the last report of a check of the run.
    #[serde(with = "time::serde::rfc3339::option")]
    #[cfg_attr(
        feature = "openapi",
        schema(value_type = Option<String>, format = DateTime, example = "2026-10-11T09:40:00Z")
    )]
    pub last_report_at: Option<OffsetDateTime>,
    /// `status_reason` when the run `failed` or `timed_out`; `null` otherwise.
    #[cfg_attr(feature = "openapi", schema(value_type = Option<closed::Reason>))]
    pub failure_reason: Option<Reason>,
    /// The required checks that failed the run.
    #[cfg_attr(feature = "openapi", schema(example = json!([])))]
    pub failure_causing_statuses: Vec<String>,
    /// What a member conflicts with, when the assembly of the test branch
    /// failed: `{"kind": "queue_pr", "pr_number": 117}` or
    /// `{"kind": "base_branch"}`.
    #[cfg_attr(feature = "openapi", schema(value_type = Option<Object>))]
    pub conflict_culprit: Option<ConflictCulprit>,
    /// Every check and status observed on `test_sha`.
    pub checks: Vec<RunCheck>,
    /// The targets that the tested PRs change (lot 13); `null` until then.
    #[cfg_attr(feature = "openapi", schema(value_type = Option<Object>))]
    pub impacted_targets: Option<Value>,
    /// The live runs whose predecessors hold a PR of this run.
    pub dependents: Vec<Dependent>,
}

/// An action of an admin, or a change of a queue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct AuditEntry {
    /// The identifier of the entry.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "audit_06cx8fn2xagkyc9x5csgnm5r2hmqf441")
    )]
    pub id: String,
    /// What was done: `queue.state_requested`, `queue.resync`,
    /// `item.cancel_requested` or `item.restart_requested`.
    #[cfg_attr(feature = "openapi", schema(example = "queue.state_requested"))]
    pub action: String,
    /// The identifier of what the action changed: a queue or an item.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "queue_06cx8fn2wagkyc9x5csgnm5r1gz34p81")
    )]
    pub target: String,
    /// The user who acted; `null` for an API token.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "user_06cx8fn2x6gkyc9x5csgnm5r2cyh4ryr")
    )]
    pub actor_user_id: Option<String>,
    /// What the target was before the action, when it matters.
    #[cfg_attr(feature = "openapi", schema(value_type = Option<Object>))]
    pub before: Option<Value>,
    /// What the action asked for.
    #[cfg_attr(
        feature = "openapi",
        schema(value_type = Option<Object>, example = json!({ "state": "paused" }))
    )]
    pub after: Option<Value>,
    /// When the action happened.
    #[serde(with = "time::serde::rfc3339")]
    #[cfg_attr(
        feature = "openapi",
        schema(value_type = String, format = DateTime, example = "2026-10-11T09:42:00Z")
    )]
    pub occurred_at: OffsetDateTime,
}

page! {
    /// A page of the audit log, newest first.
    pub struct AuditList {
        /// The entries of the page.
        pub entries: Vec<AuditEntry>,
    }
}

/// A point of the configuration of the SCM that the queue needs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Finding {
    /// `update_restriction_missing`, `app_not_exempt`,
    /// `test_branches_not_excluded`, `up_to_date_required` or
    /// `target_branch_missing`.
    #[cfg_attr(feature = "openapi", schema(example = "app_not_exempt"))]
    pub code: String,
    /// What to change, in English, for a person.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "add the App to the bypass list of the ruleset, in exempt mode")
    )]
    pub message: String,
}

/// `GET /orgs/{org}/repos/{repo}/queues/{queue}/config-check`: the last
/// check of the configuration of the SCM; no finding means a sound
/// configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ConfigCheck {
    /// What the configuration misses; empty for a sound configuration.
    pub findings: Vec<Finding>,
    /// When CIHub checked the configuration; `null` before the first check,
    /// which comes within minutes of the creation of the queue.
    #[serde(with = "time::serde::rfc3339::option")]
    #[cfg_attr(
        feature = "openapi",
        schema(value_type = Option<String>, format = DateTime, example = "2026-10-11T09:00:00Z")
    )]
    pub checked_at: Option<OffsetDateTime>,
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
            code: ErrorCode::QueueNotFound,
        };
        let json = serde_json::to_value(&problem).unwrap();
        assert_eq!(json["type"], "about:blank");
        assert_eq!(json["code"], "queue_not_found");
        assert_eq!(ErrorCode::PrAlreadyQueued.to_string(), "pr_already_queued");
    }

    #[test]
    fn each_code_has_a_status_of_an_error_and_a_sentence() {
        for code in ErrorCode::all() {
            assert!((400..600).contains(&code.status()), "{code}");
            assert!(code.sentence().ends_with('.'), "{code}");
        }
    }
}

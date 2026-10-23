//! The requests, the responses and the errors of the public API v1
//! (`specs/API.md`), shared by the server and the
//! clients. Pure: no I/O and no server code. The feature `openapi` derives
//! the `OpenAPI` schemas, for the server only.

use cihub_domain::{
    BlockReason, CheckConclusion, CheckKind, CheckStatus, ConflictCulprit, ItemState,
    MembershipRole, MergeMethod, Priority, QueueMode, QueueState, Reason, RequiredStatusesSource,
    RunStatus, Strategy, SubmittedVia,
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
        CheckStatus,
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
    DeviceCodePending,
    DeviceCodeExpired,
    Unauthorized,
    ScopeMissing,
    CsrfRefused,
    DeviceCodeDenied,
    RouteNotFound,
    OrgNotFound,
    RepoNotFound,
    QueueNotFound,
    PrNotFound,
    RunNotFound,
    DeviceCodeNotFound,
    PrAlreadyQueued,
    ItemEnded,
    ItemNotInTest,
    RateLimited,
    TooManyStreams,
    PollingTooFast,
    Internal,
    SignInUnavailable,
    ScmUnavailable,
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
            | Self::DeviceCodePending
            | Self::DeviceCodeExpired => 400,
            Self::Unauthorized => 401,
            Self::ScopeMissing | Self::CsrfRefused | Self::DeviceCodeDenied => 403,
            Self::RouteNotFound
            | Self::OrgNotFound
            | Self::RepoNotFound
            | Self::QueueNotFound
            | Self::PrNotFound
            | Self::RunNotFound
            | Self::DeviceCodeNotFound => 404,
            Self::PrAlreadyQueued | Self::ItemEnded | Self::ItemNotInTest => 409,
            Self::RateLimited | Self::TooManyStreams | Self::PollingTooFast => 429,
            Self::Internal => 500,
            Self::SignInUnavailable | Self::ScmUnavailable => 503,
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
            Self::DeviceCodePending => {
                "The person has not approved or denied the code yet: ask again after `interval` \
                 seconds."
            }
            Self::DeviceCodeExpired => {
                "The device code is unknown, already used, or older than 15 minutes: start again."
            }
            Self::Unauthorized => "No API token or session, or one that is unknown or expired.",
            Self::ScopeMissing => {
                "The token, or the role of the user of the session or their permission on the \
                 repository, lacks the scope of the route."
            }
            Self::CsrfRefused => {
                "A request of a session that can change a state lacks the header `X-CSRF-Token`, \
                 or carries another token than the cookie `__Host-cihub_csrf`."
            }
            Self::DeviceCodeDenied => {
                "The person denied the code: the command line is not signed in."
            }
            Self::RouteNotFound => "No route has this path.",
            Self::OrgNotFound => {
                "No organization has this slug, or the token or the user does not belong to it."
            }
            Self::RepoNotFound => "The organization has no repository with this name.",
            Self::QueueNotFound => "The repository has no queue of this branch.",
            Self::PrNotFound => "The PR was never submitted to a queue of the repository.",
            Self::RunNotFound => "The organization has no run with this identifier.",
            Self::DeviceCodeNotFound => {
                "No sign-in of the command line waits for this code: it is wrong, used, or older \
                 than 15 minutes."
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
            Self::PollingTooFast => {
                "The command line asked again sooner than `interval`: wait 5 seconds more each time."
            }
            Self::Internal => "The server failed; the logs of the instance say why.",
            Self::SignInUnavailable => {
                "The instance has no public address or no sign-in client: nobody signs in to it."
            }
            Self::ScmUnavailable => {
                "The SCM did not answer the check of the access of the person of the session: \
                 try again in a moment."
            }
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
    #[cfg_attr(feature = "openapi", schema(example = "no queue of `main`"))]
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
    /// When CIHub last read from the SCM the repositories that the person
    /// reaches; `null` until the first read ends, which their lists wait for.
    #[serde(with = "time::serde::rfc3339::option")]
    #[cfg_attr(
        feature = "openapi",
        schema(value_type = Option<String>, format = DateTime, example = "2026-10-11T09:00:00Z")
    )]
    pub synced_at: Option<OffsetDateTime>,
}

/// `POST /device-codes`: a sign-in of the command line, which waits for its
/// person.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct DeviceCode {
    /// The secret of the command line, which it sends to
    /// `POST /device-codes/token`; never shown to the person.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "chd_0123456789abcdefghjkmnpqrstvwx")
    )]
    pub device_code: String,
    /// The code that the person checks on the page of `verification_url`.
    #[cfg_attr(feature = "openapi", schema(example = "WDJB-MJHT"))]
    pub user_code: String,
    /// The page of the dashboard where the person approves the code.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "https://cihub.example.com/device")
    )]
    pub verification_url: String,
    /// The seconds before the code expires.
    #[cfg_attr(feature = "openapi", schema(example = 900))]
    pub expires_in: u64,
    /// The seconds to wait between two requests of the session.
    #[cfg_attr(feature = "openapi", schema(example = 5))]
    pub interval: u64,
}

/// `POST /device-codes/token`: the device code of the sign-in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct DeviceCodeToken {
    /// The `device_code` that `POST /device-codes` gave.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "chd_0123456789abcdefghjkmnpqrstvwx")
    )]
    pub device_code: String,
}

/// The session of the command line, once its person approved the code.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct CliSession {
    /// The secret of the session, for `Authorization: Bearer`; given once.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "chc_0123456789abcdefghjkmnpqrstvwx")
    )]
    pub token: String,
    /// The login of the person on the SCM.
    #[cfg_attr(feature = "openapi", schema(example = "ada"))]
    pub login: String,
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
    /// Every connection of the organization to the SCM is suspended: CIHub
    /// reaches none of its accounts, and its queues are paused.
    pub is_suspended: bool,
}

page! {
    /// A page of the organizations of the person, by slug.
    pub struct OrgList {
        /// The organizations of the page.
        pub orgs: Vec<Org>,
    }
}

/// A repository of the organization, with what the caller may do on it and
/// the verdict of its setup; `RepoDetail` gives its whole setup.
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
    /// What the caller may do on the repository.
    pub permission: Permission,
    /// The verdict of the setup of the repository.
    pub setup: SetupSummary,
}

/// A repository of the organization, with what the caller may do on it and
/// its whole setup.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RepoDetail {
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
    /// What the caller may do on the repository.
    pub permission: Permission,
    /// What CIHub found about the rules and the configuration file of the
    /// default branch.
    pub setup: RepoSetup,
}

/// What the caller may do on a repository: `read` its queues, its PRs and
/// its setup; `write`: submit, cancel and restart a PR; `admin`: change the
/// state of a queue and read a setup or a queue again from the SCM.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Permission {
    /// The caller reads the repository.
    pub read: bool,
    /// The caller acts on the PRs of the repository.
    pub write: bool,
    /// The caller acts on the queues and the setup of the repository.
    pub admin: bool,
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
    /// The branch that the queue merges into, which names the queue in the
    /// routes: a repository has one queue per branch.
    #[cfg_attr(feature = "openapi", schema(example = "main"))]
    pub target_branch: String,
    /// The branch of the queue is the default branch of its repository.
    pub is_default_branch: bool,
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
    /// CIHub no longer reaches the repository of the queue on the SCM: the
    /// App is suspended or uninstalled, or no longer reaches the repository,
    /// and the queue is paused until the access comes back.
    pub is_suspended: bool,
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
    /// What the last setup read found about the rules of the SCM that the
    /// queue needs.
    pub setup: Setup,
}

page! {
    /// A page of queues, by repository name then target branch.
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
    /// The target branch of the queue, as the routes name it.
    #[cfg_attr(feature = "openapi", schema(example = "main"))]
    pub queue: String,
    /// The number of the PR in its repository.
    #[cfg_attr(feature = "openapi", schema(example = 118))]
    pub number: u64,
    /// The title at the last read of the PR; `null` before the first.
    #[cfg_attr(feature = "openapi", schema(example = "Export invoices as CSV"))]
    pub title: Option<String>,
    /// The page of the PR on the SCM, at the last read of the PR; `null`
    /// before the first.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "https://github.com/acme/web/pull/118")
    )]
    pub url: Option<String>,
    /// The author of the PR.
    pub author: Person,
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
    /// The profile of the queue that gives the settings of the PR (its merge
    /// method, its batches, its failure budget); `null` for those of the
    /// queue.
    #[cfg_attr(feature = "openapi", schema(example = "dependencies"))]
    pub profile: Option<String>,
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

/// A person on the SCM.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Person {
    /// The login of the person on the SCM.
    #[cfg_attr(feature = "openapi", schema(example = "ada"))]
    pub login: String,
    /// The address of the image of the person on the SCM, `null` until CIHub
    /// reads it at their sign-in or in the members of an account.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "https://avatars.example.com/u/1")
    )]
    pub avatar_url: Option<String>,
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
    /// Without it, the `priority_rules` of the configuration file decide.
    #[cfg_attr(feature = "openapi", schema(example = "high"))]
    #[serde(default)]
    pub priority: Option<PriorityInput>,
    /// Test the PR alone, never in a batch.
    #[serde(default)]
    pub no_batch: bool,
    /// A profile of the queue of the base branch of the PR; without it, the
    /// `conditions` of the profiles decide.
    #[cfg_attr(feature = "openapi", schema(example = "dependencies"))]
    #[serde(default)]
    pub profile: Option<String>,
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

/// What a queue did over a period, against the period of the same length
/// before it: the PRs that ended, how long they took to merge, why the others
/// left, and the checks of its runs. A period ends with the hour, or the UTC
/// day, that holds now.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct QueueHealth {
    /// The start of the period.
    #[serde(with = "time::serde::rfc3339")]
    #[cfg_attr(
        feature = "openapi",
        schema(value_type = String, format = DateTime, example = "2026-10-13T10:00:00Z")
    )]
    pub from: OffsetDateTime,
    /// The end of the period, excluded.
    #[serde(with = "time::serde::rfc3339")]
    #[cfg_attr(
        feature = "openapi",
        schema(value_type = String, format = DateTime, example = "2026-10-14T10:00:00Z")
    )]
    pub to: OffsetDateTime,
    /// The first thing to know about the queue over the period.
    pub verdict: HealthVerdict,
    /// The PRs that ended in the period.
    pub current: HealthTotals,
    /// The PRs that ended in the period before.
    pub previous: HealthTotals,
    /// The period cut in hours (`24h`) or in UTC days (`7d`, `30d`), the oldest first.
    pub buckets: Vec<HealthBucket>,
    /// The median time that the PRs merged in the period spent in each
    /// state before their merge, in the order of the states.
    pub time_in_states: Vec<StateTime>,
    /// Why the other PRs ended without a merge, the most frequent reason first.
    pub left_without_merge: Vec<ReasonCount>,
    /// The checks of the runs that ended in the period, by name.
    pub checks: Vec<CheckHealth>,
}

/// The first thing to know about a queue over the period.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HealthVerdict {
    /// No PR merged or failed.
    Quiet,
    /// The PRs merge, and few fail.
    Healthy,
    /// More than 15 % of the PRs that merged or failed failed.
    ManyFailures,
    /// A check failed, then passed on the next attempt of its run, at least 3
    /// times: each retry costs a run and makes the PRs behind wait.
    CheckPassesOnRetry {
        /// The name of the check.
        #[cfg_attr(feature = "openapi", schema(example = "ci / test"))]
        check: String,
        /// How many times it passed on a retry.
        #[cfg_attr(feature = "openapi", schema(example = 4))]
        times: u64,
    },
}

/// The PRs that ended in a period.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct HealthTotals {
    /// The PRs that merged.
    #[cfg_attr(feature = "openapi", schema(example = 42))]
    pub merged: u64,
    /// The PRs that failed.
    #[cfg_attr(feature = "openapi", schema(example = 3))]
    pub failed: u64,
    /// The PRs that a person cancelled, or that left the queue on a change of the PR.
    #[cfg_attr(feature = "openapi", schema(example = 2))]
    pub cancelled: u64,
    /// `null` when no PR merged.
    pub time_to_merge: Option<TimeToMerge>,
}

/// The time from the submission of a PR to its merge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct TimeToMerge {
    /// Half of the PRs merged within this time, in seconds.
    #[cfg_attr(feature = "openapi", schema(example = 1380))]
    pub median_seconds: u64,
    /// 90 % of the PRs merged within this time, in seconds.
    #[cfg_attr(feature = "openapi", schema(example = 2940))]
    pub p90_seconds: u64,
}

/// The PRs that ended in an hour or a day of the period.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct HealthBucket {
    /// The start of the hour or of the day.
    #[serde(with = "time::serde::rfc3339")]
    #[cfg_attr(
        feature = "openapi",
        schema(value_type = String, format = DateTime, example = "2026-10-14T09:00:00Z")
    )]
    pub start: OffsetDateTime,
    /// The PRs that merged in the bucket.
    #[cfg_attr(feature = "openapi", schema(example = 4))]
    pub merged: u64,
    /// The PRs that failed in the bucket.
    #[cfg_attr(feature = "openapi", schema(example = 1))]
    pub failed: u64,
    /// `null` when no PR merged.
    pub time_to_merge: Option<TimeToMerge>,
}

/// The median time in a state of the PRs merged in a period; a PR that did
/// not pass through the state counts as 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct StateTime {
    /// A state before the merge.
    #[cfg_attr(feature = "openapi", schema(value_type = closed::ItemState))]
    pub state: ItemState,
    /// The median time in the state, in seconds.
    #[cfg_attr(feature = "openapi", schema(example = 420))]
    pub median_seconds: u64,
}

/// How many PRs ended for a reason.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ReasonCount {
    /// `failed` or `cancelled`.
    #[cfg_attr(feature = "openapi", schema(value_type = closed::ItemState, example = "failed"))]
    pub conclusion: ItemState,
    /// Why the PRs ended.
    #[cfg_attr(feature = "openapi", schema(value_type = closed::Reason))]
    pub reason: Reason,
    /// The reason for a person, as the comment of a PR gives it.
    #[cfg_attr(feature = "openapi", schema(example = "a required check failed"))]
    pub description: String,
    /// How many PRs ended for this reason.
    #[cfg_attr(feature = "openapi", schema(example = 3))]
    pub count: u64,
}

/// A check over the runs that ended in a period.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct CheckHealth {
    /// The name of the check or of the status.
    #[cfg_attr(feature = "openapi", schema(example = "ci / test"))]
    pub name: String,
    /// `check` for a check with a run, `status` for a commit status.
    #[cfg_attr(feature = "openapi", schema(value_type = closed::CheckKind))]
    pub kind: CheckKind,
    /// A run needs it to pass.
    pub required: bool,
    /// The runs that reported it.
    #[cfg_attr(feature = "openapi", schema(example = 48))]
    pub runs: u64,
    /// The runs where it failed.
    #[cfg_attr(feature = "openapi", schema(example = 6))]
    pub failed: u64,
    /// The runs where it passed after it failed on the attempt before.
    #[cfg_attr(feature = "openapi", schema(example = 4))]
    pub passed_on_retry: u64,
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
    /// `queued` waits for a runner, `in_progress` runs, `completed` has an
    /// outcome.
    #[cfg_attr(feature = "openapi", schema(value_type = closed::CheckStatus))]
    pub status: CheckStatus,
    /// The outcome; `null` while the check runs.
    #[cfg_attr(feature = "openapi", schema(value_type = Option<closed::CheckConclusion>))]
    pub conclusion: Option<CheckConclusion>,
    /// When the check started, as the CI reports it; `null` before. A commit
    /// status gives the time of its state only.
    #[serde(with = "time::serde::rfc3339::option")]
    #[cfg_attr(
        feature = "openapi",
        schema(value_type = Option<String>, format = DateTime, example = "2026-10-11T09:31:00Z")
    )]
    pub started_at: Option<OffsetDateTime>,
    /// When the check concluded, as the CI reports it; `null` before.
    #[serde(with = "time::serde::rfc3339::option")]
    #[cfg_attr(
        feature = "openapi",
        schema(value_type = Option<String>, format = DateTime, example = "2026-10-11T09:39:00Z")
    )]
    pub completed_at: Option<OffsetDateTime>,
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
    /// The page of the draft PR on the SCM; `null` without a draft PR.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "https://github.com/acme/web/pull/412")
    )]
    pub draft_pr_url: Option<String>,
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
    /// What was done: `queue.state_requested`, `queue.sync`, `repo.sync`,
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
    /// `test_branches_not_excluded`, `up_to_date_required`,
    /// `conditions_status_required` or `target_branch_missing`.
    #[cfg_attr(feature = "openapi", schema(example = "app_not_exempt"))]
    pub code: String,
    /// What to change, in English, for a person.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "add the App to the bypass list of the ruleset, in exempt mode")
    )]
    pub message: String,
}

/// The setup of a queue: what its last setup read found about the rules of
/// the SCM; no finding means sound rules.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Setup {
    /// What the rules miss; empty for sound rules.
    pub findings: Vec<Finding>,
    /// When CIHub read the setup; `null` before the first read, which comes
    /// within minutes of the creation of the queue.
    #[serde(with = "time::serde::rfc3339::option")]
    #[cfg_attr(
        feature = "openapi",
        schema(value_type = Option<String>, format = DateTime, example = "2026-10-11T09:00:00Z")
    )]
    pub checked_at: Option<OffsetDateTime>,
}

/// The verdict of the setup of a repository, and when CIHub read it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct SetupSummary {
    /// The verdict of the setup.
    pub state: SetupState,
    /// When CIHub read the setup; `null` before the first read.
    #[serde(with = "time::serde::rfc3339::option")]
    #[cfg_attr(
        feature = "openapi",
        schema(value_type = Option<String>, format = DateTime, example = "2026-10-16T09:00:00Z")
    )]
    pub checked_at: Option<OffsetDateTime>,
    /// When an admin asked to read the setup again, while that read waits;
    /// `null` otherwise.
    #[serde(with = "time::serde::rfc3339::option")]
    #[cfg_attr(
        feature = "openapi",
        schema(value_type = Option<String>, format = DateTime, example = "2026-10-16T09:05:00Z")
    )]
    pub requested_at: Option<OffsetDateTime>,
}

/// The setup of a repository: what CIHub found about the rules of its
/// default branch and its configuration file, with a verdict. Every field
/// comes from Postgres: showing a setup calls the SCM zero times.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RepoSetup {
    /// The verdict of the setup.
    pub state: SetupState,
    /// The target branch of the queue of the default branch, with `started`;
    /// `null` otherwise.
    #[cfg_attr(feature = "openapi", schema(example = "main"))]
    pub queue: Option<String>,
    /// The four rules that a queue needs, always in this order:
    /// `update_restriction_missing`, `app_not_exempt`,
    /// `test_branches_not_excluded`, `up_to_date_required`; empty before the
    /// first read.
    pub rules: Vec<RuleCheck>,
    /// The default branch did not exist at the last read: no queue can merge
    /// into it.
    pub target_branch_missing: bool,
    /// The page of the SCM that changes the rules of the repository; `null`
    /// when the SCM has none.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "https://scm.example.com/acme/web/settings/rules")
    )]
    pub rules_url: Option<String>,
    /// The configuration file at the head of the default branch; `null`
    /// before the first read.
    pub config: Option<ConfigFile>,
    /// When CIHub read the setup; `null` before the first read.
    #[serde(with = "time::serde::rfc3339::option")]
    #[cfg_attr(
        feature = "openapi",
        schema(value_type = Option<String>, format = DateTime, example = "2026-10-16T09:00:00Z")
    )]
    pub checked_at: Option<OffsetDateTime>,
    /// When an admin asked to read the setup again, while that read waits;
    /// `null` otherwise.
    #[serde(with = "time::serde::rfc3339::option")]
    #[cfg_attr(
        feature = "openapi",
        schema(value_type = Option<String>, format = DateTime, example = "2026-10-16T09:05:00Z")
    )]
    pub requested_at: Option<OffsetDateTime>,
}

/// The verdict of the setup of a repository, computed by the server in this
/// order: `started` (the default branch has its queue), `checking` (no read
/// yet, or a read asked after the last one), `invalid_config` (the file is
/// not valid), `cannot_merge` (a rule blocks the merges, or the default
/// branch is missing), `ready_with_warnings` (a rule fails without blocking
/// the merges), `ready`.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    strum::EnumString,
    strum::IntoStaticStr,
)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum SetupState {
    Started,
    Checking,
    Ready,
    ReadyWithWarnings,
    CannotMerge,
    InvalidConfig,
}

/// A rule of the SCM that a queue needs, and whether the default branch has
/// it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RuleCheck {
    /// `update_restriction_missing`, `app_not_exempt`,
    /// `test_branches_not_excluded` or `up_to_date_required`: the finding
    /// when the rule fails.
    #[cfg_attr(feature = "openapi", schema(example = "app_not_exempt"))]
    pub code: String,
    /// The default branch has the rule.
    pub passed: bool,
    /// The queue cannot merge while the rule fails (`app_not_exempt`,
    /// `test_branches_not_excluded`).
    pub blocks_merge: bool,
    /// The rule, in English, for a person: what passes, or what fails.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "The CIHub App is exempt from it")
    )]
    pub title: String,
    /// What to change, in English, for a person, when the rule fails; `null`
    /// when it passes.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "add the App to the bypass list of the ruleset, in exempt mode")
    )]
    pub message: Option<String>,
}

/// The configuration file at the head of the default branch of a
/// repository.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ConfigFile {
    /// `none` without a file, `valid`, or `invalid`.
    pub state: ConfigFileState,
    /// The path of the file: `.cihub.yaml`, `.cihub.yml`,
    /// `.cihub/config.yaml` or `.cihub/config.yml`; `null` without a file.
    #[cfg_attr(feature = "openapi", schema(example = ".cihub.yaml"))]
    pub path: Option<String>,
    /// The head of the default branch that was read; `null` when the branch
    /// is missing.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "9f2c4e1a7b3d5f60812a4c6e8b0d2f4a6c8e0b1d")
    )]
    pub sha: Option<String>,
    /// Why the file is not valid; `null` otherwise.
    #[cfg_attr(
        feature = "openapi",
        schema(example = "line 6: `batching.size` is from 1 to 100")
    )]
    pub error: Option<String>,
    /// The line of the error, from 1; `null` without one.
    #[cfg_attr(feature = "openapi", schema(example = 6))]
    pub line: Option<u32>,
    /// The column of the error, from 1; `null` without one.
    #[cfg_attr(feature = "openapi", schema(example = 3))]
    pub column: Option<u32>,
    /// The text of the file when it is not valid, at most 64 KiB; `null`
    /// otherwise.
    #[cfg_attr(feature = "openapi", schema(example = "queues:\n  - branch: main\n"))]
    pub text: Option<String>,
    /// The `branch` of each entry of the file, globs as written.
    #[cfg_attr(feature = "openapi", schema(example = json!(["main", "release/*"])))]
    pub entries: Vec<String>,
    /// The strategy of the default branch: from the file, or the default.
    #[cfg_attr(feature = "openapi", schema(value_type = closed::Strategy))]
    pub strategy: Strategy,
    /// The settings of the default branch: from the file, or the defaults.
    pub settings: SettingsSummary,
}

/// Whether a repository has a configuration file on its default branch, and
/// whether it is valid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum ConfigFileState {
    None,
    Valid,
    Invalid,
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
    fn the_states_of_a_setup_are_snake_case() {
        let states = [
            (SetupState::Started, "started"),
            (SetupState::Checking, "checking"),
            (SetupState::Ready, "ready"),
            (SetupState::ReadyWithWarnings, "ready_with_warnings"),
            (SetupState::CannotMerge, "cannot_merge"),
            (SetupState::InvalidConfig, "invalid_config"),
        ];

        for (state, text) in states {
            let json = serde_json::to_value(state).unwrap();
            assert_eq!(json, text);
            assert_eq!(serde_json::from_value::<SetupState>(json).unwrap(), state);
        }
    }

    #[test]
    fn the_states_of_a_config_file_are_snake_case() {
        let files = [
            (ConfigFileState::None, "none"),
            (ConfigFileState::Valid, "valid"),
            (ConfigFileState::Invalid, "invalid"),
        ];

        for (state, text) in files {
            let json = serde_json::to_value(state).unwrap();
            assert_eq!(json, text);
            assert_eq!(
                serde_json::from_value::<ConfigFileState>(json).unwrap(),
                state
            );
        }
    }

    #[test]
    fn each_code_has_a_status_of_an_error_and_a_sentence() {
        for code in ErrorCode::all() {
            assert!((400..600).contains(&code.status()), "{code}");
            assert!(code.sentence().ends_with('.'), "{code}");
        }
    }
}

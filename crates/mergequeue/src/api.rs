//! The routes of the merge queue under `/api/v1` (`specs/API.md`). They name
//! the organization and the repository by their names and the queue by its
//! target branch, read the
//! projections in a transaction of the organization, and change a queue only
//! through its inbox (`queue_inbox`) or a job: the scheduler stays the only
//! writer of the log.

mod branch_config;
mod health;
mod prs;
mod queues;
mod repos;
mod runs;

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post};
use cihub_api_types::{
    BranchConfig, CheckHealth, ConditionBlock, ConditionDocument, ConfigDocument, ConfigFile,
    ConfigFileState, ConfigPreview, ConfigProposal, ConfigSettings, DefaultsDocument, Dependent,
    DiffLine, ErrorCode, Event, EventList, Finding, Graph, GraphEdge, GraphNode, GraphRun,
    HealthBucket, HealthTotals, HealthVerdict, ItemCounts, LineChange, Permission, Person, Pr,
    PrAhead, PrList, PreviewConfig, PriorityRuleDocument, ProfileDocument, ProposeConfig, Queue,
    QueueDocument, QueueHealth, QueueList, ReasonCount, Repo, RepoDetail, RepoList, RepoSetup,
    RequestedQueueState, RequiredStatusesDocument, ResolvedConfig, ResolvedQueue, RuleCheck, Run,
    RunCheck, SettingSource, SettingSources, SettingsDocument, SettingsSummary, Setup, SetupState,
    SetupSummary, StateTime, SubmitPr, TimeToMerge, UpdateQueue, Window,
};
use cihub_domain::{Actor, RepoId};
use cihub_platform::{ApiError, ApiGrant, GrantActor, ModuleApi, PAGE_ERRORS, error_responses};
use cihub_scm::{Scm, ScmUser};
use cihub_store::{PgPool, Postgres, Transaction};
use serde::Deserialize;
use serde_json::Value;
use sqlx::PgConnection;
use utoipa::openapi::{RefOr, Response as DocumentResponse};
use utoipa::{IntoResponses, OpenApi};

use crate::Clock;
use crate::jobs::Account;

#[derive(Clone)]
struct ApiState {
    pool: PgPool,
    clock: Clock,
    /// Gives the page of the rules of a repository; the routes make no call.
    scm: Arc<dyn Scm>,
}

#[derive(OpenApi)]
#[openapi(
    tags(
        (
            name = "Queues",
            description = "A queue merges the PRs of one target branch, in order, once each one \
                           passed its tests on top of those ahead of it."
        ),
        (
            name = "Pull requests",
            description = "A PR in a queue, as its last submission gives it; a PR submitted again \
                           gets a new submission, whose history its events keep."
        ),
        (
            name = "Runs",
            description = "A run tests, on the CI, a test branch that holds a group of PRs on top \
                           of the PRs ahead of them."
        ),
        (
            name = "Repositories",
            description = "A repository of the organization: what the caller may do on it, and \
                           its setup, what CIHub found about the rules of its default branch and \
                           its configuration file, with a verdict."
        )
    ),
    paths(
        queues::list_org_queues,
        queues::list_repo_queues,
        queues::get_queue,
        queues::update_queue,
        queues::sync_queue,
        queues::graph,
        health::get_health,
        prs::list_org_prs,
        prs::list_repo_prs,
        prs::get_pr,
        prs::list_events,
        prs::submit,
        prs::cancel,
        prs::restart,
        runs::get_run,
        repos::list_repos,
        repos::get_repo,
        repos::sync_repo,
        branch_config::get_branch_config,
        branch_config::preview_config,
        branch_config::propose_config
    ),
    components(schemas(
        BranchConfig,
        CheckHealth,
        ConditionBlock,
        ConditionDocument,
        ConfigDocument,
        ConfigPreview,
        ConfigProposal,
        DefaultsDocument,
        DiffLine,
        LineChange,
        PreviewConfig,
        PriorityRuleDocument,
        ProfileDocument,
        ProposeConfig,
        QueueDocument,
        RequiredStatusesDocument,
        ResolvedConfig,
        ResolvedQueue,
        SettingsDocument,
        ConfigFile,
        ConfigFileState,
        ConfigSettings,
        Dependent,
        Event,
        EventList,
        Finding,
        Graph,
        GraphEdge,
        GraphNode,
        GraphRun,
        HealthBucket,
        HealthTotals,
        HealthVerdict,
        ItemCounts,
        Permission,
        Person,
        Pr,
        PrAhead,
        PrList,
        Queue,
        QueueHealth,
        QueueList,
        ReasonCount,
        Repo,
        RepoDetail,
        RepoList,
        RepoSetup,
        RequestedQueueState,
        RuleCheck,
        Run,
        RunCheck,
        SettingSource,
        SettingSources,
        SettingsSummary,
        Setup,
        SetupState,
        SetupSummary,
        StateTime,
        SubmitPr,
        TimeToMerge,
        UpdateQueue,
        Window
    ))
)]
struct MergeQueueApi;

/// The `OpenAPI` document of the routes of the queues.
pub(crate) fn openapi() -> utoipa::openapi::OpenApi {
    MergeQueueApi::openapi()
}

/// The routes of the module and their `OpenAPI` document.
pub(crate) fn api(pool: PgPool, clock: Clock, scm: Arc<dyn Scm>) -> ModuleApi {
    let routes = Router::new()
        .route("/orgs/{org}/queues", get(queues::list_org_queues))
        .route(
            "/orgs/{org}/repos/{repo}/queues",
            get(queues::list_repo_queues),
        )
        .route(
            "/orgs/{org}/repos/{repo}/queues/{queue}",
            get(queues::get_queue).patch(queues::update_queue),
        )
        .route(
            "/orgs/{org}/repos/{repo}/queues/{queue}/sync",
            post(queues::sync_queue),
        )
        .route(
            "/orgs/{org}/repos/{repo}/queues/{queue}/graph",
            get(queues::graph),
        )
        .route(
            "/orgs/{org}/repos/{repo}/queues/{queue}/health",
            get(health::get_health),
        )
        .route("/orgs/{org}/prs", get(prs::list_org_prs))
        .route("/orgs/{org}/repos", get(repos::list_repos))
        .route("/orgs/{org}/repos/{repo}", get(repos::get_repo))
        .route("/orgs/{org}/repos/{repo}/sync", post(repos::sync_repo))
        .route(
            "/orgs/{org}/repos/{repo}/config",
            get(branch_config::get_branch_config),
        )
        .route(
            "/orgs/{org}/repos/{repo}/config/preview",
            post(branch_config::preview_config),
        )
        .route(
            "/orgs/{org}/repos/{repo}/config/proposals",
            post(branch_config::propose_config),
        )
        .route("/orgs/{org}/repos/{repo}/prs", get(prs::list_repo_prs))
        .route("/orgs/{org}/repos/{repo}/prs/{number}", get(prs::get_pr))
        .route(
            "/orgs/{org}/repos/{repo}/prs/{number}/events",
            get(prs::list_events),
        )
        .route(
            "/orgs/{org}/repos/{repo}/prs/{number}/submit",
            post(prs::submit),
        )
        .route(
            "/orgs/{org}/repos/{repo}/prs/{number}/cancel",
            post(prs::cancel),
        )
        .route(
            "/orgs/{org}/repos/{repo}/prs/{number}/restart",
            post(prs::restart),
        )
        .route("/orgs/{org}/repos/{repo}/runs/{run_id}", get(runs::get_run))
        .with_state(ApiState { pool, clock, scm });
    ModuleApi {
        routes,
        openapi: openapi(),
    }
}

/// The repository that the URL names.
#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Path)]
struct RepoPath {
    /// The slug of the organization.
    #[param(example = "acme")]
    org: String,
    /// The name of the repository in the organization.
    #[param(example = "web")]
    repo: String,
}

/// The queue that the URL names.
#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Path)]
struct QueuePath {
    /// The slug of the organization.
    #[param(example = "acme")]
    org: String,
    /// The name of the repository in the organization.
    #[param(example = "web")]
    repo: String,
    /// The target branch of the queue, percent-encoded: `release%2F2.0`.
    #[param(example = "main")]
    queue: String,
}

/// The PR that the URL names.
#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Path)]
struct PrPath {
    /// The slug of the organization.
    #[param(example = "acme")]
    org: String,
    /// The name of the repository in the organization.
    #[param(example = "web")]
    repo: String,
    /// The number of the PR in the repository.
    #[param(example = 118)]
    number: u64,
}

/// The run that the URL names.
#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Path)]
struct RunPath {
    /// The slug of the organization.
    #[param(example = "acme")]
    org: String,
    /// The name of the repository in the organization.
    #[param(example = "web")]
    repo: String,
    /// The identifier of the run.
    #[param(example = "run_06cx8fn2wtgkyc9x5csgnm5r22ca6g0w")]
    run_id: String,
}

/// Declares the errors of a route in its `OpenAPI` document, besides those
/// that `cihub-web` adds to every route of its kind. One type per route:
/// two types of a route would overwrite each other's statuses.
macro_rules! route_errors {
    ($name:ident: $($codes:expr),+ $(,)?) => {
        struct $name;

        impl IntoResponses for $name {
            fn responses() -> BTreeMap<String, RefOr<DocumentResponse>> {
                let codes: &[&[ErrorCode]] = &[$($codes),+];
                error_responses(&codes.concat())
            }
        }
    };
}

route_errors!(QueueListErrors: PAGE_ERRORS, &[ErrorCode::RepoNotFound]);
route_errors!(QueueErrors: &[ErrorCode::RepoNotFound, ErrorCode::QueueNotFound]);
route_errors!(
    UpdateQueueErrors: &[
        ErrorCode::InvalidRequest,
        ErrorCode::RepoNotFound,
        ErrorCode::QueueNotFound,
    ]
);
route_errors!(OrgPrListErrors: PAGE_ERRORS);
route_errors!(
    RepoPrListErrors: PAGE_ERRORS,
    &[ErrorCode::RepoNotFound, ErrorCode::QueueNotFound]
);
route_errors!(
    PrErrors: &[
        ErrorCode::InvalidRequest,
        ErrorCode::RepoNotFound,
        ErrorCode::PrNotFound,
    ]
);
route_errors!(
    EventErrors: PAGE_ERRORS,
    &[ErrorCode::RepoNotFound, ErrorCode::PrNotFound]
);
route_errors!(
    SubmitErrors: &[
        ErrorCode::InvalidRequest,
        ErrorCode::UnknownPriority,
        ErrorCode::RepoNotFound,
        ErrorCode::PrAlreadyQueued,
    ]
);
route_errors!(
    CancelErrors: &[
        ErrorCode::InvalidRequest,
        ErrorCode::RepoNotFound,
        ErrorCode::PrNotFound,
        ErrorCode::ItemEnded,
    ]
);
route_errors!(
    RestartErrors: &[
        ErrorCode::InvalidRequest,
        ErrorCode::RepoNotFound,
        ErrorCode::PrNotFound,
        ErrorCode::ItemNotInTest,
    ]
);
route_errors!(RunErrors: &[ErrorCode::RepoNotFound, ErrorCode::RunNotFound]);
route_errors!(RepoListErrors: PAGE_ERRORS);
route_errors!(RepoErrors: &[ErrorCode::RepoNotFound]);
route_errors!(BranchConfigErrors: &[ErrorCode::RepoNotFound, ErrorCode::ConfigNotFound]);
route_errors!(
    PreviewErrors: &[
        ErrorCode::InvalidRequest,
        ErrorCode::RepoNotFound,
        ErrorCode::ConfigNotFound,
    ]
);
route_errors!(
    ProposeErrors: &[
        ErrorCode::InvalidRequest,
        ErrorCode::InvalidConfig,
        ErrorCode::RepoNotFound,
        ErrorCode::ConfigNotFound,
        ErrorCode::ConfigProposalOpen,
    ]
);

/// The text that a `q` filter looks for in a name: from 1 to 100 characters.
fn name_part(q: Option<String>) -> Result<Option<String>, ApiError> {
    match q {
        Some(text) if !(1..=100).contains(&text.chars().count()) => Err(ApiError::new(
            ErrorCode::InvalidRequest,
            "`q` is from 1 to 100 characters".to_owned(),
        )),
        q => Ok(q),
    }
}

/// A transaction of the organization of the URL, and the repository that the
/// URL names, read in it.
async fn begin_repo(
    state: &ApiState,
    grant: &ApiGrant,
    org: &str,
    repo: &str,
) -> Result<(Transaction<'static, Postgres>, RepoId), ApiError> {
    let mut tx = grant.begin_in_org(&state.pool, org).await?;
    let repo = repo_id(&mut tx, repo).await?;

    Ok((tx, repo))
}

/// The repository `name` of the organization of the transaction. Its
/// repositories come from one installation of the App, so from one account:
/// the name is enough.
async fn repo_id(conn: &mut PgConnection, name: &str) -> Result<RepoId, ApiError> {
    // The SCM does not tell names apart by their case.
    let id: Option<String> = sqlx::query_scalar(
        "SELECT id FROM repos WHERE removed_at IS NULL AND lower(name) = lower($1)
         ORDER BY id LIMIT 1",
    )
    .bind(name)
    .fetch_optional(conn)
    .await?;

    id.ok_or_else(|| {
        ApiError::new(
            ErrorCode::RepoNotFound,
            format!("repository `{name}` not found"),
        )
    })?
    .parse()
    .map_err(ApiError::from)
}

/// Who acts in the log: the user of a session, or CIHub for a token, which
/// stands for no person.
fn actor(grant: &ApiGrant) -> Actor {
    match grant.actor {
        GrantActor::User(user_id) => Actor::User { user_id },
        GrantActor::Token(_) => Actor::System,
    }
}

/// The account of the user of a session on the SCM: the submission job
/// checks its permission on the repository, `write`, or `admin` for a direct
/// merge, as for a command in a comment. A token has the scope `prs:write`
/// and no account.
async fn requester(conn: &mut PgConnection, grant: &ApiGrant) -> Result<Option<Account>, ApiError> {
    let GrantActor::User(user_id) = grant.actor else {
        return Ok(None);
    };

    let (external_id, login): (String, String) =
        sqlx::query_as("SELECT external_id, login FROM users WHERE id = $1")
            .bind(user_id.to_string())
            .fetch_one(conn)
            .await?;
    Ok(Some(Account::from(&ScmUser { external_id, login })))
}

/// Writes an action of the actor of `grant` in the audit log.
async fn audit(
    conn: &mut PgConnection,
    grant: &ApiGrant,
    action: &str,
    target: &str,
    after: Value,
) -> Result<(), ApiError> {
    #[expect(
        clippy::disallowed_methods,
        reason = "an entry of the audit log has a new identifier"
    )]
    let id = cihub_domain::AuditEntryId::new();
    let (token_id, user_id) = match grant.actor {
        GrantActor::Token(token_id) => (Some(token_id.to_string()), None),
        GrantActor::User(user_id) => (None, Some(user_id.to_string())),
    };
    sqlx::query(
        "INSERT INTO audit_log (id, org_id, actor_token_id, actor_user_id, action, target, after)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(id.to_string())
    .bind(grant.org_id.to_string())
    .bind(token_id)
    .bind(user_id)
    .bind(action)
    .bind(target)
    .bind(after)
    .execute(conn)
    .await?;
    Ok(())
}

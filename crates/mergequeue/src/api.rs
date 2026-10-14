//! The routes of the merge queue under `/api/v1` (`specs/API.md`). They name
//! the organization, the repository and the queue by their names, read the
//! projections in a transaction of the organization, and change a queue only
//! through its inbox (`queue_inbox`) or a job: the scheduler stays the only
//! writer of the log.

mod health;
mod prs;
mod queues;
mod runs;

use std::collections::BTreeMap;

use axum::Router;
use axum::routing::{get, post};
use cihub_api_types::{
    CheckHealth, ConfigCheck, Dependent, ErrorCode, Event, EventList, Finding, Graph, GraphEdge,
    GraphNode, GraphRun, HealthBucket, HealthTotals, HealthVerdict, ItemCounts, Person, Pr,
    PrAhead, PrList, Queue, QueueAccess, QueueHealth, QueueList, ReasonCount, RequestedQueueState,
    Run, RunCheck, SettingsSummary, StateTime, SubmitPr, TimeToMerge, UpdateQueue, Window,
};
use cihub_domain::{Actor, RepoId};
use cihub_platform::{ApiError, ApiGrant, GrantActor, ModuleApi, PAGE_ERRORS, error_responses};
use cihub_scm::ScmUser;
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
        )
    ),
    paths(
        queues::list_org_queues,
        queues::list_repo_queues,
        queues::get_queue,
        queues::update_queue,
        queues::resync_queue,
        queues::graph,
        health::get_health,
        prs::list_prs,
        prs::get_pr,
        prs::list_events,
        prs::submit,
        prs::cancel,
        prs::restart,
        runs::get_run
    ),
    components(schemas(
        CheckHealth,
        ConfigCheck,
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
        Person,
        Pr,
        PrAhead,
        PrList,
        Queue,
        QueueAccess,
        QueueHealth,
        QueueList,
        ReasonCount,
        RequestedQueueState,
        Run,
        RunCheck,
        SettingsSummary,
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
pub(crate) fn api(pool: PgPool, clock: Clock) -> ModuleApi {
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
            "/orgs/{org}/repos/{repo}/queues/{queue}/resync",
            post(queues::resync_queue),
        )
        .route(
            "/orgs/{org}/repos/{repo}/queues/{queue}/graph",
            get(queues::graph),
        )
        .route(
            "/orgs/{org}/repos/{repo}/queues/{queue}/health",
            get(health::get_health),
        )
        .route("/orgs/{org}/prs", get(prs::list_prs))
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
        .route("/orgs/{org}/runs/{run_id}", get(runs::get_run))
        .with_state(ApiState { pool, clock });
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
    /// The name of the queue in the repository.
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
route_errors!(
    PrListErrors: PAGE_ERRORS,
    &[
        ErrorCode::MeNeedsSession,
        ErrorCode::QueueNeedsRepo,
        ErrorCode::RepoNotFound,
        ErrorCode::QueueNotFound,
    ]
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
        ErrorCode::QueueNotFound,
        ErrorCode::QueueNotAccepting,
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
route_errors!(RunErrors: &[ErrorCode::RunNotFound]);

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
        "SELECT id FROM repos WHERE lower(name) = lower($1) ORDER BY id LIMIT 1",
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

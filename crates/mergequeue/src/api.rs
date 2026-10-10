//! The routes of the merge queue under `/api/v1` (`specs/API.md`). They name
//! the organization, the repository and the queue by their names, read the
//! projections in a transaction of the organization, and change a queue only
//! through its inbox (`queue_inbox`) or a job: the scheduler stays the only
//! writer of the log.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::Router;
use axum::http::StatusCode;
use axum::routing::{get, post};
use cihub_api_types::{
    Accepted, BatchGet, BatchGetResponse, ConfigCheck, Dependent, ErrorCode, Finding, Graph,
    GraphEdge, GraphNode, Item, ItemCounts, ItemPage, MAX_BATCH_GET, MAX_TAKE, OrgQueue,
    OrgQueueList, PrItem, Queue, QueueList, RequestedQueueState, RunCheck, SubmitPr, TestRun,
    TimelineEvent, UpdateQueue,
};
use cihub_domain::{
    Actor, ApiScope, EventKind, ItemId, ItemState, JobClass, Priority, QueueControl, QueueId,
    Reason, RepoId, RunId, SubmittedVia,
};
use cihub_platform::{
    ApiError, ApiGrant, ApiJson, ApiPath, ApiQuery, GrantActor, Job, ModuleApi, enqueue_job,
};
use cihub_scm::{Scm, ScmError, ScmUser, SetupFinding};
use cihub_store::{PgPool, Postgres, Transaction};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgConnection;
use time::OffsetDateTime;
use utoipa::OpenApi;

use crate::Clock;
use crate::jobs::{self, Account, give_back, new_event_id};
use crate::target::QueueTarget;

#[derive(Clone)]
struct ApiState {
    pool: PgPool,
    clock: Clock,
    scm: Arc<dyn Scm>,
}

#[derive(OpenApi)]
#[openapi(
    paths(
        list_org_queues,
        list_queues,
        get_queue,
        update_queue,
        queue_action,
        list_items,
        submit,
        get_pr,
        batch_get,
        graph,
        cancel_item,
        restart_item,
        test_run,
        config_check
    ),
    components(schemas(
        Accepted,
        ConfigCheck,
        Finding,
        BatchGet,
        BatchGetResponse,
        Graph,
        Item,
        ItemCounts,
        ItemPage,
        OrgQueue,
        OrgQueueList,
        PrItem,
        Queue,
        QueueList,
        SubmitPr,
        TestRun,
        UpdateQueue
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
        .route("/orgs/{org}/queues", get(list_org_queues))
        .route("/orgs/{org}/repos/{repo}/queues", get(list_queues))
        // `POST` on a queue or a PR takes the action after a colon in the last
        // segment: `{queue}:resync`, `{number}:cancel`, `{number}:restart`.
        .route(
            "/orgs/{org}/repos/{repo}/queues/{queue}",
            get(get_queue).patch(update_queue).post(queue_action),
        )
        .route(
            "/orgs/{org}/repos/{repo}/queues/{queue}/prs",
            get(list_items).post(submit),
        )
        .route(
            "/orgs/{org}/repos/{repo}/queues/{queue}/prs/{number}",
            get(get_pr).post(pr_action),
        )
        .route(
            "/orgs/{org}/repos/{repo}/queues/{queue}/prs:batchGet",
            post(batch_get),
        )
        .route("/orgs/{org}/repos/{repo}/queues/{queue}/graph", get(graph))
        .route(
            "/orgs/{org}/repos/{repo}/queues/{queue}/config-check",
            get(config_check),
        )
        .route("/orgs/{org}/runs/{run_id}", get(test_run))
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
    org: String,
    /// The name of the repository in the organization.
    repo: String,
}

/// The queue that the URL names.
#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Path)]
struct QueuePath {
    /// The slug of the organization.
    org: String,
    /// The name of the repository in the organization.
    repo: String,
    /// The name of the queue in the repository.
    queue: String,
}

/// The PR that the URL names in a queue.
#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Path)]
struct PrPath {
    /// The slug of the organization.
    org: String,
    /// The name of the repository in the organization.
    repo: String,
    /// The name of the queue in the repository.
    queue: String,
    /// The number of the PR.
    number: u64,
}

impl PrPath {
    fn into_queue_and_number(self) -> (QueuePath, u64) {
        let queue = QueuePath {
            org: self.org,
            repo: self.repo,
            queue: self.queue,
        };

        (queue, self.number)
    }
}

/// The path of an action on a PR: its last segment is `{number}:{action}`.
#[derive(Deserialize)]
struct PrActionPath {
    org: String,
    repo: String,
    queue: String,
    #[serde(rename = "number")]
    number_and_action: String,
}

/// The run that the URL names.
#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Path)]
struct RunPath {
    /// The slug of the organization.
    org: String,
    /// The identifier of the run.
    run_id: String,
}

/// A transaction of the organization of the URL, and the queue that the URL
/// names, read in it.
async fn begin_queue(
    state: &ApiState,
    grant: &ApiGrant,
    path: &QueuePath,
) -> Result<(Transaction<'static, Postgres>, QueueRow), ApiError> {
    let mut tx = grant.begin_in_org(&state.pool, &path.org).await?;
    let repo = repo_id(&mut tx, &path.repo).await?;
    let row = queue_row(&mut tx, &repo, &path.queue).await?;

    Ok((tx, row))
}

/// The answer of a request that the queue takes later.
fn accepted() -> (StatusCode, axum::Json<Accepted>) {
    (
        StatusCode::ACCEPTED,
        axum::Json(Accepted { accepted: true }),
    )
}

#[derive(sqlx::FromRow)]
struct QueueRow {
    id: String,
    repo_id: String,
    name: String,
    target_branch: String,
    state: String,
    mode: String,
    config_sha: Option<String>,
    config_error: Option<String>,
}

const QUEUE_COLUMNS: &str =
    "id, repo_id, name, target_branch, state::text, mode::text, config_sha, config_error";

impl QueueRow {
    fn into_queue(self) -> Result<Queue, ApiError> {
        Ok(Queue {
            state: self.state.parse()?,
            mode: self.mode.parse()?,
            id: self.id,
            repo_id: self.repo_id,
            name: self.name,
            target_branch: self.target_branch,
            config_sha: self.config_sha,
            config_error: self.config_error,
        })
    }
}

/// The queue `name` of the repository `repo`.
async fn queue_row(
    conn: &mut PgConnection,
    repo: &RepoId,
    name: &str,
) -> Result<QueueRow, ApiError> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {QUEUE_COLUMNS} FROM queues WHERE repo_id = $1 AND name = $2"
    )))
    .bind(repo.to_string())
    .bind(name)
    .fetch_optional(conn)
    .await?
    .ok_or_else(|| ApiError::not_found("queue"))
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

    id.ok_or_else(|| ApiError::not_found(&format!("repository {name}")))?
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

/// The queues of a repository.
#[utoipa::path(
    get,
    path = "/orgs/{org}/repos/{repo}/queues",
    params(RepoPath),
    responses((status = 200, body = QueueList)),
    security(("token" = ["queues:read"]))
)]
async fn list_queues(
    grant: ApiGrant,
    axum::extract::State(state): axum::extract::State<ApiState>,
    ApiPath(path): ApiPath<RepoPath>,
) -> Result<axum::Json<QueueList>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;

    let mut tx = grant.begin_in_org(&state.pool, &path.org).await?;
    let repo = repo_id(&mut tx, &path.repo).await?;
    let rows: Vec<QueueRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {QUEUE_COLUMNS} FROM queues WHERE repo_id = $1 ORDER BY name"
    )))
    .bind(repo.to_string())
    .fetch_all(&mut *tx)
    .await?;
    let queues = rows
        .into_iter()
        .map(QueueRow::into_queue)
        .collect::<Result<_, _>>()?;

    Ok(axum::Json(QueueList { queues }))
}

#[derive(sqlx::FromRow)]
struct OrgQueueRow {
    repo: String,
    #[sqlx(flatten)]
    queue: QueueRow,
    not_ready: i64,
    pending: i64,
    testing: i64,
    pending_failure: i64,
    tests_passed: i64,
    merged_last_24h: i64,
    failed_last_24h: i64,
    last_merged_at: Option<OffsetDateTime>,
}

impl OrgQueueRow {
    fn into_org_queue(self) -> Result<OrgQueue, ApiError> {
        let counts = ItemCounts {
            not_ready: u32::try_from(self.not_ready)?,
            pending: u32::try_from(self.pending)?,
            testing: u32::try_from(self.testing)?,
            pending_failure: u32::try_from(self.pending_failure)?,
            tests_passed: u32::try_from(self.tests_passed)?,
        };

        Ok(OrgQueue {
            repo: self.repo,
            queue: self.queue.into_queue()?,
            counts,
            merged_last_24h: u32::try_from(self.merged_last_24h)?,
            failed_last_24h: u32::try_from(self.failed_last_24h)?,
            last_merged_at: self.last_merged_at,
        })
    }
}

/// Every queue of the organization with its counters, for the overview of
/// the dashboard. Like the graph and the commands, the counters read only
/// the current item of each PR.
#[utoipa::path(
    get,
    path = "/orgs/{org}/queues",
    params(("org" = String, Path, description = "The slug of the organization")),
    responses((status = 200, body = OrgQueueList)),
    security(("token" = ["queues:read"]))
)]
async fn list_org_queues(
    grant: ApiGrant,
    axum::extract::State(state): axum::extract::State<ApiState>,
    ApiPath(org): ApiPath<String>,
) -> Result<axum::Json<OrgQueueList>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;

    let mut tx = grant.begin_in_org(&state.pool, &org).await?;
    let now = (state.clock)();
    let rows: Vec<OrgQueueRow> = sqlx::query_as(
        "SELECT r.name AS repo, q.id, q.repo_id, q.name, q.target_branch, q.state::text AS state,
                q.mode::text AS mode, q.config_sha, q.config_error,
                count(*) FILTER (WHERE i.is_current AND i.state = 'not_ready') AS not_ready,
                count(*) FILTER (WHERE i.is_current AND i.state = 'pending') AS pending,
                count(*) FILTER (WHERE i.is_current AND i.state = 'testing') AS testing,
                count(*) FILTER (WHERE i.is_current AND i.state = 'pending_failure')
                    AS pending_failure,
                count(*) FILTER (WHERE i.is_current AND i.state = 'tests_passed') AS tests_passed,
                count(*) FILTER (WHERE i.state = 'merged'
                                   AND i.state_changed_at > $1 - interval '24 hours')
                    AS merged_last_24h,
                count(*) FILTER (WHERE i.state = 'failed'
                                   AND i.state_changed_at > $1 - interval '24 hours')
                    AS failed_last_24h,
                max(i.state_changed_at) FILTER (WHERE i.state = 'merged') AS last_merged_at
         FROM queues q
         JOIN repos r ON r.id = q.repo_id
         LEFT JOIN queue_items i ON i.queue_id = q.id
         GROUP BY q.id, r.id
         ORDER BY r.name, q.name, q.id",
    )
    .bind(now)
    .fetch_all(&mut *tx)
    .await?;

    let queues = rows
        .into_iter()
        .map(OrgQueueRow::into_org_queue)
        .collect::<Result<_, _>>()?;

    Ok(axum::Json(OrgQueueList { queues }))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/repos/{repo}/queues/{queue}",
    params(QueuePath),
    responses((status = 200, body = Queue)),
    security(("token" = ["queues:read"]))
)]
async fn get_queue(
    grant: ApiGrant,
    axum::extract::State(state): axum::extract::State<ApiState>,
    ApiPath(path): ApiPath<QueuePath>,
) -> Result<axum::Json<Queue>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;

    let (_, row) = begin_queue(&state, &grant, &path).await?;

    Ok(axum::Json(row.into_queue()?))
}

/// Asks the queue for a new state; the queue changes it at its next
/// reconciliation.
#[utoipa::path(
    patch,
    path = "/orgs/{org}/repos/{repo}/queues/{queue}",
    params(QueuePath),
    request_body = UpdateQueue,
    responses((status = 202, body = Queue)),
    security(("token" = ["queues:write"]))
)]
async fn update_queue(
    grant: ApiGrant,
    axum::extract::State(state): axum::extract::State<ApiState>,
    ApiPath(path): ApiPath<QueuePath>,
    ApiJson(request): ApiJson<UpdateQueue>,
) -> Result<(StatusCode, axum::Json<Queue>), ApiError> {
    grant.require(ApiScope::QueuesWrite)?;

    let (mut tx, row) = begin_queue(&state, &grant, &path).await?;
    let control = match request.state {
        RequestedQueueState::Running => QueueControl::Resume,
        RequestedQueueState::Paused => QueueControl::Pause,
        RequestedQueueState::Draining => QueueControl::Drain,
    };
    let id: QueueId = row.id.parse()?;
    let now = (state.clock)();
    let facts = vec![(actor(&grant), EventKind::QueueControlRequested { control })];

    give_back(&mut tx, grant.org_id, id, facts, now).await?;

    let after = json!({ "state": request.state });
    audit(&mut tx, &grant, "queue.state_requested", &row.id, after).await?;

    let queue = row.into_queue()?;
    tx.commit().await?;

    Ok((StatusCode::ACCEPTED, axum::Json(queue)))
}

/// `POST …/queues/{queue}:resync`: reads again from the SCM each live PR,
/// the checks of each live run and the head of the target.
#[utoipa::path(
    post,
    path = "/orgs/{org}/repos/{repo}/queues/{queue}:resync",
    params(QueuePath),
    responses((status = 202, body = Accepted)),
    security(("token" = ["queues:write"]))
)]
async fn queue_action(
    grant: ApiGrant,
    axum::extract::State(state): axum::extract::State<ApiState>,
    ApiPath(path): ApiPath<QueuePath>,
) -> Result<(StatusCode, axum::Json<Accepted>), ApiError> {
    let Some(queue) = path.queue.strip_suffix(":resync").map(str::to_owned) else {
        return Err(ApiError::not_found("action"));
    };

    grant.require(ApiScope::QueuesWrite)?;

    let path = QueuePath { queue, ..path };
    let (mut tx, row) = begin_queue(&state, &grant, &path).await?;

    audit(&mut tx, &grant, "queue.resync", &row.id, json!({})).await?;
    tx.commit().await?;

    let id: QueueId = row.id.parse()?;
    crate::resync(&state.pool, id, (state.clock)()).await?;

    Ok(accepted())
}

#[derive(sqlx::FromRow)]
struct ItemRow {
    id: String,
    queue_id: String,
    pr_number: i64,
    pr_head_sha: String,
    state: String,
    state_reason: String,
    state_changed_at: OffsetDateTime,
    priority_value: i16,
    position: Option<i64>,
    is_current: bool,
    block_reasons: Value,
    group_id: Option<String>,
    merged_sha: Option<String>,
    submitted_at: OffsetDateTime,
}

const ITEM_COLUMNS: &str = "id, queue_id, pr_number, pr_head_sha, state::text, state_reason::text,
    state_changed_at, priority_value, position, is_current, block_reasons, group_id, merged_sha,
    submitted_at";

impl ItemRow {
    fn into_item(self) -> Result<Item, ApiError> {
        let state_reason: Reason = self.state_reason.parse()?;
        Ok(Item {
            pr_number: u64::try_from(self.pr_number)?,
            state: self.state.parse()?,
            state_reason,
            state_description: state_reason.describe().to_owned(),
            priority: Priority(u8::try_from(self.priority_value)?),
            block_reasons: serde_json::from_value(self.block_reasons)?,
            id: self.id,
            queue_id: self.queue_id,
            head_sha: self.pr_head_sha,
            state_changed_at: self.state_changed_at,
            position: self.position,
            is_current: self.is_current,
            group_id: self.group_id,
            merged_sha: self.merged_sha,
            submitted_at: self.submitted_at,
        })
    }
}

fn items(rows: Vec<ItemRow>) -> Result<Vec<Item>, ApiError> {
    rows.into_iter().map(ItemRow::into_item).collect()
}

#[derive(Deserialize, utoipa::IntoParams)]
struct ItemFilter {
    /// Only the items in this state.
    #[param(value_type = Option<String>)]
    state: Option<ItemState>,
    /// Submitted at or after this time (RFC 3339).
    #[serde(default, with = "time::serde::rfc3339::option")]
    #[param(value_type = Option<String>)]
    since: Option<OffsetDateTime>,
    /// Submitted before this time (RFC 3339).
    #[serde(default, with = "time::serde::rfc3339::option")]
    #[param(value_type = Option<String>)]
    until: Option<OffsetDateTime>,
    /// The `next_cursor` of the previous page.
    cursor: Option<String>,
    /// At most 100; 50 by default.
    take: Option<u32>,
}

/// The items of a queue, live and past, newest submission first.
#[utoipa::path(
    get,
    path = "/orgs/{org}/repos/{repo}/queues/{queue}/prs",
    params(QueuePath, ItemFilter),
    responses((status = 200, body = ItemPage)),
    security(("token" = ["queues:read"]))
)]
async fn list_items(
    grant: ApiGrant,
    axum::extract::State(state): axum::extract::State<ApiState>,
    ApiPath(path): ApiPath<QueuePath>,
    ApiQuery(filter): ApiQuery<ItemFilter>,
) -> Result<axum::Json<ItemPage>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;

    let take = filter.take.unwrap_or(50);
    if take == 0 || take > MAX_TAKE {
        return Err(ApiError::invalid(format!("take is from 1 to {MAX_TAKE}")));
    }

    let (mut tx, queue) = begin_queue(&state, &grant, &path).await?;
    let rows: Vec<ItemRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {ITEM_COLUMNS} FROM queue_items
         WHERE queue_id = $1
           AND ($2::item_state IS NULL OR state = $2::item_state)
           AND ($3::timestamptz IS NULL OR submitted_at >= $3)
           AND ($4::timestamptz IS NULL OR submitted_at < $4)
           AND ($5::text IS NULL OR (submitted_at, id) <
                (SELECT submitted_at, id FROM queue_items WHERE id = $5))
         ORDER BY submitted_at DESC, id DESC
         LIMIT $6"
    )))
    .bind(&queue.id)
    .bind(filter.state.map(ItemState::as_str))
    .bind(filter.since)
    .bind(filter.until)
    .bind(&filter.cursor)
    .bind(i64::from(take) + 1)
    .fetch_all(&mut *tx)
    .await?;
    let mut page = items(rows)?;
    let next_cursor = if page.len() > take as usize {
        page.truncate(take as usize);
        page.last().map(|item| item.id.clone())
    } else {
        None
    };

    Ok(axum::Json(ItemPage {
        items: page,
        next_cursor,
    }))
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/repos/{repo}/queues/{queue}/prs",
    params(QueuePath),
    request_body = SubmitPr,
    responses((status = 202, body = Accepted)),
    security(("token" = ["prs:write"]))
)]
async fn submit(
    grant: ApiGrant,
    axum::extract::State(state): axum::extract::State<ApiState>,
    ApiPath(path): ApiPath<QueuePath>,
    ApiJson(request): ApiJson<SubmitPr>,
) -> Result<(StatusCode, axum::Json<Accepted>), ApiError> {
    grant.require(ApiScope::PrsWrite)?;

    let priority = request
        .priority
        .as_ref()
        .map(|input| {
            input
                .priority()
                .ok_or_else(|| ApiError::invalid("the priority is a name or a value from 0 to 255"))
        })
        .transpose()?;
    let (mut tx, row) = begin_queue(&state, &grant, &path).await?;
    let id: QueueId = row.id.parse()?;
    let target = QueueTarget::load(&mut tx, id).await?;
    let requested_by = requester(&mut tx, &grant).await?;
    let payload = jobs::SubmitPr {
        repo_id: target.repo_id,
        pr_number: request.pr_number,
        via: SubmittedVia::Api,
        requested_by,
        priority,
        no_batch: request.no_batch,
        direct: request.direct,
        queue: Some(target.name.clone()),
        label: None,
        comment_id: None,
    };
    let cause = format!("api-{}", new_event_id());
    let job: Job = jobs::submit(grant.org_id, target.repo.connection.id, &cause, &payload);
    let now = (state.clock)();

    enqueue_job(&mut tx, &job, now, now).await?;
    tx.commit().await?;

    Ok(accepted())
}

#[derive(Deserialize, utoipa::IntoParams)]
struct PrQuery {
    /// `timeline` adds the events of the item.
    include: Option<String>,
}

/// The last item of a PR in the queue.
#[utoipa::path(
    get,
    path = "/orgs/{org}/repos/{repo}/queues/{queue}/prs/{number}",
    params(PrPath, PrQuery),
    responses((status = 200, body = PrItem)),
    security(("token" = ["queues:read"]))
)]
async fn get_pr(
    grant: ApiGrant,
    axum::extract::State(state): axum::extract::State<ApiState>,
    ApiPath(path): ApiPath<PrPath>,
    ApiQuery(query): ApiQuery<PrQuery>,
) -> Result<axum::Json<PrItem>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;

    let (path, pr_number) = path.into_queue_and_number();
    let (mut tx, queue) = begin_queue(&state, &grant, &path).await?;
    let number = i64::try_from(pr_number).map_err(|_| ApiError::not_found("PR"))?;
    let row: Option<ItemRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {ITEM_COLUMNS} FROM queue_items WHERE queue_id = $1 AND pr_number = $2
         ORDER BY submitted_at DESC, id DESC LIMIT 1"
    )))
    .bind(&queue.id)
    .bind(number)
    .fetch_optional(&mut *tx)
    .await?;
    let item = row.ok_or_else(|| ApiError::not_found("PR"))?.into_item()?;
    let timeline = match query.include.as_deref() {
        Some("timeline") => Some(timeline(&mut tx, &queue.id, &item.id).await?),
        Some(other) => return Err(ApiError::invalid(format!("unknown include `{other}`"))),
        None => None,
    };

    Ok(axum::Json(PrItem { item, timeline }))
}

async fn timeline(
    conn: &mut PgConnection,
    queue_id: &str,
    item_id: &str,
) -> Result<Vec<TimelineEvent>, ApiError> {
    type Row = (String, i64, String, String, OffsetDateTime, Value);
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, seq, type::text, actor_kind::text, occurred_at, payload FROM events
         WHERE queue_id = $1 AND item_id = $2 ORDER BY seq",
    )
    .bind(queue_id)
    .bind(item_id)
    .fetch_all(conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(id, seq, kind, actor_kind, occurred_at, payload)| TimelineEvent {
                id,
                seq,
                kind,
                actor_kind,
                occurred_at,
                payload,
            },
        )
        .collect())
}

/// The last item of each PR, at most 50 PRs.
#[utoipa::path(
    post,
    path = "/orgs/{org}/repos/{repo}/queues/{queue}/prs:batchGet",
    params(QueuePath),
    request_body = BatchGet,
    responses((status = 200, body = BatchGetResponse)),
    security(("token" = ["queues:read"]))
)]
async fn batch_get(
    grant: ApiGrant,
    axum::extract::State(state): axum::extract::State<ApiState>,
    ApiPath(path): ApiPath<QueuePath>,
    ApiJson(request): ApiJson<BatchGet>,
) -> Result<axum::Json<BatchGetResponse>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;

    if request.pr_numbers.len() > MAX_BATCH_GET {
        return Err(ApiError::invalid(format!(
            "at most {MAX_BATCH_GET} PRs in one read"
        )));
    }

    let (mut tx, queue) = begin_queue(&state, &grant, &path).await?;
    let numbers: Vec<i64> = request
        .pr_numbers
        .iter()
        .filter_map(|n| i64::try_from(*n).ok())
        .collect();
    let rows: Vec<ItemRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT DISTINCT ON (pr_number) {ITEM_COLUMNS} FROM queue_items
         WHERE queue_id = $1 AND pr_number = ANY($2)
         ORDER BY pr_number, submitted_at DESC, id DESC"
    )))
    .bind(&queue.id)
    .bind(&numbers)
    .fetch_all(&mut *tx)
    .await?;
    let found = items(rows)?;
    let not_found = request
        .pr_numbers
        .iter()
        .copied()
        .filter(|n| !found.iter().any(|item| item.pr_number == *n))
        .collect();

    Ok(axum::Json(BatchGetResponse {
        items: found,
        not_found,
    }))
}

/// The merge graph of the queue: its live items in order, and the
/// predecessors that the test of each one merges before it.
#[utoipa::path(
    get,
    path = "/orgs/{org}/repos/{repo}/queues/{queue}/graph",
    params(QueuePath),
    responses((status = 200, body = Graph)),
    security(("token" = ["queues:read"]))
)]
async fn graph(
    grant: ApiGrant,
    axum::extract::State(state): axum::extract::State<ApiState>,
    ApiPath(path): ApiPath<QueuePath>,
) -> Result<axum::Json<Graph>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;

    let (mut tx, queue) = begin_queue(&state, &grant, &path).await?;
    let rows: Vec<(String, i64, String, Option<i64>)> = sqlx::query_as(
        "SELECT id, pr_number, state::text, position FROM queue_items
         WHERE queue_id = $1 AND is_current
           AND state NOT IN ('merged', 'failed', 'cancelled')
         ORDER BY position NULLS LAST, submitted_at",
    )
    .bind(&queue.id)
    .fetch_all(&mut *tx)
    .await?;
    let nodes = rows
        .into_iter()
        .map(|(item_id, pr_number, state, position)| {
            Ok(GraphNode {
                item_id,
                pr_number: u64::try_from(pr_number)?,
                state: state.parse()?,
                position,
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    let groups: Vec<(Vec<String>, Vec<String>)> = sqlx::query_as(
        "SELECT item_ids, predecessor_ids FROM test_groups
         WHERE queue_id = $1 AND state NOT IN ('merged', 'failed', 'bisected', 'dissolved')",
    )
    .bind(&queue.id)
    .fetch_all(&mut *tx)
    .await?;
    let mut edges = Vec::new();

    for (members, predecessors) in groups {
        for to in &members {
            for from in &predecessors {
                edges.push(GraphEdge {
                    from: from.clone(),
                    to: to.clone(),
                });
            }
        }
    }

    Ok(axum::Json(Graph { nodes, edges }))
}

/// The current item of the PR `number` in the queue `queue_id`, and its
/// state.
async fn current_item(
    conn: &mut PgConnection,
    queue_id: &str,
    number: u64,
) -> Result<(ItemId, ItemState), ApiError> {
    let number = i64::try_from(number).map_err(|_| ApiError::not_found("item"))?;
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT id, state::text FROM queue_items
         WHERE queue_id = $1 AND pr_number = $2 AND is_current",
    )
    .bind(queue_id)
    .bind(number)
    .fetch_optional(conn)
    .await?;
    let (id, state) = row.ok_or_else(|| ApiError::not_found("item"))?;

    Ok((id.parse()?, state.parse()?))
}

/// Gives the queue the request `kind` on the current item of the PR `number`.
async fn give(
    state: &ApiState,
    grant: &ApiGrant,
    path: &QueuePath,
    number: u64,
    kind: impl FnOnce(ItemId, ItemState) -> Result<EventKind, ApiError>,
) -> Result<(StatusCode, axum::Json<Accepted>), ApiError> {
    grant.require(ApiScope::PrsWrite)?;

    let (mut tx, queue) = begin_queue(state, grant, path).await?;
    let (item_id, item_state) = current_item(&mut tx, &queue.id, number).await?;
    let fact = kind(item_id, item_state)?;
    let queue_id: QueueId = queue.id.parse()?;

    give_back(
        &mut tx,
        grant.org_id,
        queue_id,
        vec![(actor(grant), fact)],
        (state.clock)(),
    )
    .await?;

    tx.commit().await?;

    Ok(accepted())
}

/// `POST …/prs/{number}:cancel` and `POST …/prs/{number}:restart`.
async fn pr_action(
    grant: ApiGrant,
    axum::extract::State(state): axum::extract::State<ApiState>,
    ApiPath(path): ApiPath<PrActionPath>,
) -> Result<(StatusCode, axum::Json<Accepted>), ApiError> {
    let Some((number, action)) = path.number_and_action.split_once(':') else {
        return Err(ApiError::not_found("action"));
    };

    let number: u64 = number
        .parse()
        .map_err(|_| ApiError::invalid("the PR number is a positive integer"))?;
    let queue = QueuePath {
        org: path.org,
        repo: path.repo,
        queue: path.queue,
    };

    match action {
        "cancel" => cancel_item(&state, &grant, &queue, number).await,
        "restart" => restart_item(&state, &grant, &queue, number).await,
        _ => Err(ApiError::not_found("action")),
    }
}

/// Cancels the current item of the PR (`admin_requested`).
#[utoipa::path(
    post,
    path = "/orgs/{org}/repos/{repo}/queues/{queue}/prs/{number}:cancel",
    params(PrPath),
    responses((status = 202, body = Accepted)),
    security(("token" = ["prs:write"]))
)]
async fn cancel_item(
    state: &ApiState,
    grant: &ApiGrant,
    path: &QueuePath,
    number: u64,
) -> Result<(StatusCode, axum::Json<Accepted>), ApiError> {
    give(state, grant, path, number, |item_id, item_state| {
        if item_state.is_terminal() {
            return Err(ApiError::new(
                ErrorCode::InvalidState,
                format!("the item is `{item_state}`"),
            ));
        }

        Ok(EventKind::ItemCancelRequested {
            item_id,
            by_admin: true,
        })
    })
    .await
}

/// Restarts the tests of the current item of the PR, in test
/// (`tests_restarted_by_api_request`).
#[utoipa::path(
    post,
    path = "/orgs/{org}/repos/{repo}/queues/{queue}/prs/{number}:restart",
    params(PrPath),
    responses((status = 202, body = Accepted)),
    security(("token" = ["prs:write"]))
)]
async fn restart_item(
    state: &ApiState,
    grant: &ApiGrant,
    path: &QueuePath,
    number: u64,
) -> Result<(StatusCode, axum::Json<Accepted>), ApiError> {
    give(state, grant, path, number, |item_id, item_state| {
        if !matches!(item_state, ItemState::Testing | ItemState::PendingFailure) {
            return Err(ApiError::new(
                ErrorCode::InvalidState,
                format!("only an item in test can restart; this one is `{item_state}`"),
            ));
        }

        Ok(EventKind::ItemRestartRequested {
            item_id,
            by_api: true,
        })
    })
    .await
}

#[derive(sqlx::FromRow)]
struct RunRow {
    id: String,
    queue_id: String,
    group_id: String,
    attempt: i32,
    status: String,
    status_reason: String,
    test_branch: String,
    test_sha: Option<String>,
    base_sha: String,
    draft_pr_number: Option<i64>,
    tested_pr_numbers: Vec<i64>,
    head_shas: Vec<String>,
    required_statuses: Vec<String>,
    required_statuses_source: Option<String>,
    started_at: OffsetDateTime,
    ended_at: Option<OffsetDateTime>,
    last_report_at: Option<OffsetDateTime>,
    failure_reason: Option<String>,
    failure_causing_statuses: Vec<String>,
    conflict_culprit: Option<Value>,
}

/// The detail of a run.
#[utoipa::path(
    get,
    path = "/orgs/{org}/runs/{run_id}",
    params(RunPath),
    responses((status = 200, body = TestRun)),
    security(("token" = ["queues:read"]))
)]
async fn test_run(
    grant: ApiGrant,
    axum::extract::State(state): axum::extract::State<ApiState>,
    ApiPath(path): ApiPath<RunPath>,
) -> Result<axum::Json<TestRun>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;

    let RunPath { org, run_id } = path;
    let _: RunId = run_id.parse().map_err(|_| ApiError::not_found("run"))?;
    let mut tx = grant.begin_in_org(&state.pool, &org).await?;
    let run: RunRow = sqlx::query_as(
        "SELECT id, queue_id, group_id, attempt, status::text, status_reason::text, test_branch,
                test_sha, base_sha, draft_pr_number, tested_pr_numbers, head_shas,
                required_statuses, required_statuses_source::text, started_at, ended_at,
                (SELECT max(c.observed_at) FROM check_observations c
                 WHERE c.run_id = test_runs.id) AS last_report_at,
                CASE WHEN status IN ('failed', 'timed_out') THEN status_reason::text END
                    AS failure_reason,
                failure_causing_statuses, conflict_culprit
         FROM test_runs WHERE id = $1",
    )
    .bind(&run_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| ApiError::not_found("run"))?;
    let (members, predecessors): (Vec<String>, Vec<String>) =
        sqlx::query_as("SELECT item_ids, predecessor_ids FROM test_groups WHERE id = $1")
            .bind(&run.group_id)
            .fetch_one(&mut *tx)
            .await?;
    let numbers: BTreeMap<String, i64> =
        sqlx::query_as("SELECT id, pr_number FROM queue_items WHERE id = ANY($1)")
            .bind(&predecessors)
            .fetch_all(&mut *tx)
            .await?
            .into_iter()
            .collect();
    let predecessor_pr_numbers = predecessors
        .iter()
        .filter_map(|id| numbers.get(id))
        .map(|n| u64::try_from(*n).map_err(ApiError::from))
        .collect::<Result<_, _>>()?;
    let checks = run_checks(&mut tx, &run_id).await?;
    let dependents = dependents(&mut tx, &run.queue_id, &run_id, &members).await?;
    let unsigned = |n: i64| u64::try_from(n).map_err(ApiError::from);

    Ok(axum::Json(TestRun {
        attempt: u32::try_from(run.attempt)?,
        status: run.status.parse()?,
        status_reason: run.status_reason.parse()?,
        draft_pr_number: run.draft_pr_number.map(unsigned).transpose()?,
        tested_pr_numbers: run
            .tested_pr_numbers
            .into_iter()
            .map(unsigned)
            .collect::<Result<_, _>>()?,
        required_statuses_source: run
            .required_statuses_source
            .map(|s| s.parse().map_err(ApiError::from))
            .transpose()?,
        failure_reason: run
            .failure_reason
            .map(|s| s.parse().map_err(ApiError::from))
            .transpose()?,
        conflict_culprit: run
            .conflict_culprit
            .map(|value| serde_json::from_value(value).map_err(ApiError::from))
            .transpose()?,
        id: run.id,
        queue_id: run.queue_id,
        group_id: run.group_id,
        test_branch: run.test_branch,
        test_sha: run.test_sha,
        base_sha: run.base_sha,
        head_shas: run.head_shas,
        predecessor_pr_numbers,
        required_statuses: run.required_statuses,
        started_at: run.started_at,
        ended_at: run.ended_at,
        last_report_at: run.last_report_at,
        failure_causing_statuses: run.failure_causing_statuses,
        checks,
        impacted_targets: None,
        dependents,
    }))
}

async fn run_checks(conn: &mut PgConnection, run_id: &str) -> Result<Vec<RunCheck>, ApiError> {
    type Row = (
        String,
        String,
        bool,
        Option<String>,
        Option<String>,
        OffsetDateTime,
    );
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT name, kind::text, required, conclusion::text, details_url, observed_at
         FROM check_observations WHERE run_id = $1 ORDER BY name, kind",
    )
    .bind(run_id)
    .fetch_all(conn)
    .await?;
    rows.into_iter()
        .map(
            |(name, kind, required, conclusion, details_url, observed_at)| {
                Ok(RunCheck {
                    name,
                    kind: kind.parse()?,
                    required,
                    conclusion: conclusion
                        .map(|c| c.parse().map_err(ApiError::from))
                        .transpose()?,
                    details_url,
                    observed_at,
                })
            },
        )
        .collect()
}

/// The live runs of the queue whose predecessors hold a member of the run.
async fn dependents(
    conn: &mut PgConnection,
    queue_id: &str,
    run_id: &str,
    members: &[String],
) -> Result<Vec<Dependent>, ApiError> {
    let rows: Vec<(String, Vec<i64>)> = sqlx::query_as(
        "SELECT r.id, r.tested_pr_numbers FROM test_runs r
         JOIN test_groups g ON g.id = r.group_id
         WHERE r.queue_id = $1 AND r.id <> $2 AND r.status IN ('assembling', 'in_progress')
           AND g.predecessor_ids && $3
         ORDER BY r.started_at, r.id",
    )
    .bind(queue_id)
    .bind(run_id)
    .bind(members)
    .fetch_all(conn)
    .await?;
    rows.into_iter()
        .map(|(run_id, numbers)| {
            Ok(Dependent {
                run_id,
                pr_numbers: numbers
                    .into_iter()
                    .map(|n| u64::try_from(n).map_err(ApiError::from))
                    .collect::<Result<_, _>>()?,
            })
        })
        .collect()
}

/// What the configuration of the SCM misses for the queue: no finding means a sound configuration.
#[utoipa::path(
    get,
    path = "/orgs/{org}/repos/{repo}/queues/{queue}/config-check",
    params(QueuePath),
    responses((status = 200, body = ConfigCheck)),
    security(("token" = ["queues:read"]))
)]
async fn config_check(
    grant: ApiGrant,
    axum::extract::State(state): axum::extract::State<ApiState>,
    ApiPath(path): ApiPath<QueuePath>,
) -> Result<axum::Json<ConfigCheck>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;

    let (mut tx, row) = begin_queue(&state, &grant, &path).await?;
    let id: QueueId = row.id.parse()?;
    let target = QueueTarget::load(&mut tx, id).await?;

    tx.commit().await?;

    let findings = state
        .scm
        .check_setup(&target.repo, &target.target_branch, JobClass::Interactive)
        .await
        .map_err(|err| match err {
            ScmError::NotFound(_) => ApiError::not_found("target branch"),
            err => ApiError::from(err),
        })?;
    let findings = findings
        .into_iter()
        .map(|finding| Finding {
            code: finding.code().to_owned(),
            message: finding_text(finding, &target.target_branch),
        })
        .collect();

    Ok(axum::Json(ConfigCheck { findings }))
}

fn finding_text(finding: SetupFinding, branch: &str) -> String {
    match finding {
        SetupFinding::UpdateRestrictionMissing => {
            format!("no rule restricts the updates of `{branch}`: a PR can merge without the queue")
        }
        SetupFinding::AppNotExempt => format!(
            "a rule restricts the updates of `{branch}` without an exemption for the App of CIHub: \
             the queue cannot merge"
        ),
        SetupFinding::TestBranchesNotExcluded => {
            "a rule covers `cihub-temp/**` or `cihub-merge/**`: exclude the test branches"
                .to_owned()
        }
        SetupFinding::UpToDateRequired => format!(
            "the rules of `{branch}` ask for branches up to date: turn it off, unless the queue \
             merges directly"
        ),
    }
}

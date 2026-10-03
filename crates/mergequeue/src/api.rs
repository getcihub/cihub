//! The routes of the merge queue under `/api/v1` (`docs/API.md`,
//! `DECISIONS.md` D159, D160). They read the projections in a transaction of the
//! organization of the token, and change a queue only through its inbox
//! (`queue_inbox`) or a job: the scheduler stays the only writer of the log.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::Router;
use axum::http::StatusCode;
use axum::routing::{get, post};
use cihub_api_types::{
    Accepted, BatchGet, BatchGetResponse, ConfigCheck, CreateQueue, Dependent, ErrorCode, Finding,
    Graph, GraphEdge, GraphNode, Item, ItemPage, MAX_BATCH_GET, MAX_TAKE, PrItem, Queue, QueueList,
    RequestedQueueState, RunCheck, SubmitPr, TestRun, TimelineEvent, UpdateQueue,
};
use cihub_domain::{
    Actor, ApiScope, EventKind, ItemId, ItemState, JobClass, Priority, QueueControl, QueueId,
    RepoId, RunId, SubmittedVia,
};
use cihub_platform::{
    ApiError, ApiGrant, ApiJson, ApiPath, ApiQuery, Job, ModuleApi, enqueue_job, mark_dirty,
};
use cihub_scm::{Scm, ScmError, SetupFinding};
use cihub_store::{PgPool, Postgres, Transaction, begin_org};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgConnection;
use time::OffsetDateTime;
use utoipa::OpenApi;

use crate::Clock;
use crate::jobs::{self, give_back, new_event_id};
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
        list_queues,
        create_queue,
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
        CreateQueue,
        Graph,
        Item,
        ItemPage,
        PrItem,
        Queue,
        QueueList,
        SubmitPr,
        TestRun,
        UpdateQueue
    ))
)]
struct MergeQueueApi;

/// The routes of the module and their `OpenAPI` document.
pub(crate) fn api(pool: PgPool, clock: Clock, scm: Arc<dyn Scm>) -> ModuleApi {
    let routes = Router::new()
        .route(
            "/repos/{owner}/{name}/queues",
            get(list_queues).post(create_queue),
        )
        .route(
            "/queues/{queue_id}",
            get(get_queue).patch(update_queue).post(queue_action),
        )
        .route("/queues/{queue_id}/items", get(list_items).post(submit))
        .route("/queues/{queue_id}/prs/{pr_number}", get(get_pr))
        .route("/queues/{queue_id}/prs:batchGet", post(batch_get))
        .route("/queues/{queue_id}/graph", get(graph))
        .route("/items/{item_id}/cancel", post(cancel_item))
        .route("/items/{item_id}/restart", post(restart_item))
        .route("/test-runs/{run_id}", get(test_run))
        .route("/queues/{queue_id}/config-check", get(config_check))
        .with_state(ApiState { pool, clock, scm });
    ModuleApi {
        routes,
        openapi: MergeQueueApi::openapi(),
    }
}

async fn begin(
    state: &ApiState,
    grant: &ApiGrant,
) -> Result<Transaction<'static, Postgres>, ApiError> {
    Ok(begin_org(&state.pool, grant.org_id).await?)
}

/// A transaction of the organization of `grant`, and the queue `id` read in
/// it.
async fn begin_queue(
    state: &ApiState,
    grant: &ApiGrant,
    id: &str,
) -> Result<(Transaction<'static, Postgres>, QueueRow), ApiError> {
    let mut tx = begin(state, grant).await?;
    let row = queue_row(&mut tx, id).await?;
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

/// The queue `id`, when the organization of the transaction has it.
async fn queue_row(conn: &mut PgConnection, id: &str) -> Result<QueueRow, ApiError> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {QUEUE_COLUMNS} FROM queues WHERE id = $1"
    )))
    .bind(id)
    .fetch_optional(conn)
    .await?
    .ok_or_else(|| ApiError::not_found("queue"))
}

/// The repository `owner/name` of the organization of the transaction.
async fn repo_id(conn: &mut PgConnection, owner: &str, name: &str) -> Result<RepoId, ApiError> {
    // The SCM does not tell names apart by their case.
    let id: Option<String> = sqlx::query_scalar(
        "SELECT id FROM repos WHERE lower(namespace) = lower($1) AND lower(name) = lower($2)
         ORDER BY id LIMIT 1",
    )
    .bind(owner)
    .bind(name)
    .fetch_optional(conn)
    .await?;
    id.ok_or_else(|| ApiError::not_found(&format!("repository {owner}/{name}")))?
        .parse()
        .map_err(ApiError::from)
}

/// Writes an action of the token of `grant` in the audit log.
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
    sqlx::query(
        "INSERT INTO audit_log (id, org_id, actor_token_id, action, target, after)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(id.to_string())
    .bind(grant.org_id.to_string())
    .bind(grant.token_id.to_string())
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
    path = "/repos/{owner}/{name}/queues",
    params(("owner" = String, Path), ("name" = String, Path)),
    responses((status = 200, body = QueueList)),
    security(("token" = ["queues:read"]))
)]
async fn list_queues(
    grant: ApiGrant,
    axum::extract::State(state): axum::extract::State<ApiState>,
    ApiPath((owner, name)): ApiPath<(String, String)>,
) -> Result<axum::Json<QueueList>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;
    let mut tx = begin(&state, &grant).await?;
    let repo = repo_id(&mut tx, &owner, &name).await?;
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

/// Creates a queue on a branch of a repository. Its settings come from the
/// file of the repository (`DECISIONS.md` D134).
#[utoipa::path(
    post,
    path = "/repos/{owner}/{name}/queues",
    params(("owner" = String, Path), ("name" = String, Path)),
    request_body = CreateQueue,
    responses((status = 201, body = Queue)),
    security(("token" = ["queues:write"]))
)]
async fn create_queue(
    grant: ApiGrant,
    axum::extract::State(state): axum::extract::State<ApiState>,
    ApiPath((owner, name)): ApiPath<(String, String)>,
    ApiJson(request): ApiJson<CreateQueue>,
) -> Result<(StatusCode, axum::Json<Queue>), ApiError> {
    grant.require(ApiScope::QueuesWrite)?;
    let branch = request.target_branch.trim();
    let queue_name = request.name.as_deref().unwrap_or(branch).trim();
    if branch.is_empty() || queue_name.is_empty() {
        return Err(ApiError::invalid("the branch and the name cannot be empty"));
    }
    let mut tx = begin(&state, &grant).await?;
    let repo = repo_id(&mut tx, &owner, &name).await?;
    #[expect(
        clippy::disallowed_methods,
        reason = "a new queue gets a new identifier"
    )]
    let id = QueueId::new();
    let inserted = sqlx::query(
        "INSERT INTO queues (id, org_id, repo_id, target_branch, name, dirty)
         VALUES ($1, $2, $3, $4, $5, true)
         ON CONFLICT (repo_id, name) DO NOTHING",
    )
    .bind(id.to_string())
    .bind(grant.org_id.to_string())
    .bind(repo.to_string())
    .bind(branch)
    .bind(queue_name)
    .execute(&mut *tx)
    .await?;
    if inserted.rows_affected() == 0 {
        return Err(ApiError::new(
            ErrorCode::QueueExists,
            format!("the repository has a queue named `{queue_name}`"),
        ));
    }
    let after = json!({ "name": queue_name, "target_branch": branch });
    audit(&mut tx, &grant, "queue.created", &id.to_string(), after).await?;
    mark_dirty(&mut tx, id).await?;
    let queue = queue_row(&mut tx, &id.to_string()).await?.into_queue()?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, axum::Json(queue)))
}

#[utoipa::path(
    get,
    path = "/queues/{queue_id}",
    params(("queue_id" = String, Path)),
    responses((status = 200, body = Queue)),
    security(("token" = ["queues:read"]))
)]
async fn get_queue(
    grant: ApiGrant,
    axum::extract::State(state): axum::extract::State<ApiState>,
    ApiPath(queue_id): ApiPath<String>,
) -> Result<axum::Json<Queue>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;
    let (_, row) = begin_queue(&state, &grant, &queue_id).await?;
    Ok(axum::Json(row.into_queue()?))
}

/// Asks the queue for a new state; the queue changes it at its next
/// reconciliation.
#[utoipa::path(
    patch,
    path = "/queues/{queue_id}",
    params(("queue_id" = String, Path)),
    request_body = UpdateQueue,
    responses((status = 202, body = Queue)),
    security(("token" = ["queues:write"]))
)]
async fn update_queue(
    grant: ApiGrant,
    axum::extract::State(state): axum::extract::State<ApiState>,
    ApiPath(queue_id): ApiPath<String>,
    ApiJson(request): ApiJson<UpdateQueue>,
) -> Result<(StatusCode, axum::Json<Queue>), ApiError> {
    grant.require(ApiScope::QueuesWrite)?;
    let (mut tx, row) = begin_queue(&state, &grant, &queue_id).await?;
    let control = match request.state {
        RequestedQueueState::Running => QueueControl::Resume,
        RequestedQueueState::Paused => QueueControl::Pause,
        RequestedQueueState::Draining => QueueControl::Drain,
    };
    let id: QueueId = row.id.parse()?;
    let now = (state.clock)();
    let facts = vec![(Actor::System, EventKind::QueueControlRequested { control })];
    give_back(&mut tx, grant.org_id, id, facts, now).await?;
    let after = json!({ "state": request.state });
    audit(&mut tx, &grant, "queue.state_requested", &queue_id, after).await?;
    let queue = row.into_queue()?;
    tx.commit().await?;
    Ok((StatusCode::ACCEPTED, axum::Json(queue)))
}

/// `POST /queues/{queue_id}:resync`: reads again from the SCM each live PR,
/// the checks of each live run and the head of the target (`DECISIONS.md`
/// D128).
#[utoipa::path(
    post,
    path = "/queues/{queue_id}:resync",
    params(("queue_id" = String, Path)),
    responses((status = 202, body = Accepted)),
    security(("token" = ["queues:write"]))
)]
async fn queue_action(
    grant: ApiGrant,
    axum::extract::State(state): axum::extract::State<ApiState>,
    ApiPath(target): ApiPath<String>,
) -> Result<(StatusCode, axum::Json<Accepted>), ApiError> {
    let Some(queue_id) = target.strip_suffix(":resync") else {
        return Err(ApiError::not_found("action"));
    };
    grant.require(ApiScope::QueuesWrite)?;
    let (mut tx, row) = begin_queue(&state, &grant, queue_id).await?;
    audit(&mut tx, &grant, "queue.resync", queue_id, json!({})).await?;
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
        Ok(Item {
            pr_number: u64::try_from(self.pr_number)?,
            state: self.state.parse()?,
            state_reason: self.state_reason.parse()?,
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
    path = "/queues/{queue_id}/items",
    params(("queue_id" = String, Path), ItemFilter),
    responses((status = 200, body = ItemPage)),
    security(("token" = ["queues:read"]))
)]
async fn list_items(
    grant: ApiGrant,
    axum::extract::State(state): axum::extract::State<ApiState>,
    ApiPath(queue_id): ApiPath<String>,
    ApiQuery(filter): ApiQuery<ItemFilter>,
) -> Result<axum::Json<ItemPage>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;
    let take = filter.take.unwrap_or(50);
    if take == 0 || take > MAX_TAKE {
        return Err(ApiError::invalid(format!("take is from 1 to {MAX_TAKE}")));
    }
    let (mut tx, _) = begin_queue(&state, &grant, &queue_id).await?;
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
    .bind(&queue_id)
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
    path = "/queues/{queue_id}/items",
    params(("queue_id" = String, Path)),
    request_body = SubmitPr,
    responses((status = 202, body = Accepted)),
    security(("token" = ["prs:write"]))
)]
async fn submit(
    grant: ApiGrant,
    axum::extract::State(state): axum::extract::State<ApiState>,
    ApiPath(queue_id): ApiPath<String>,
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
    let (mut tx, row) = begin_queue(&state, &grant, &queue_id).await?;
    let id: QueueId = row.id.parse()?;
    let target = QueueTarget::load(&mut tx, id).await?;
    let payload = jobs::SubmitPr {
        repo_id: target.repo_id,
        pr_number: request.pr_number,
        via: SubmittedVia::Api,
        requested_by: None,
        priority,
        no_batch: request.no_batch,
        direct: request.direct,
        queue: Some(target.name.clone()),
        label: None,
    };
    let cause = format!("api-{}", new_event_id());
    let connection = (target.scm_instance_id, target.repo.connection.id);
    let job: Job = jobs::submit(grant.org_id, connection, &cause, &payload);
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
    path = "/queues/{queue_id}/prs/{pr_number}",
    params(("queue_id" = String, Path), ("pr_number" = u64, Path), PrQuery),
    responses((status = 200, body = PrItem)),
    security(("token" = ["queues:read"]))
)]
async fn get_pr(
    grant: ApiGrant,
    axum::extract::State(state): axum::extract::State<ApiState>,
    ApiPath((queue_id, pr_number)): ApiPath<(String, u64)>,
    ApiQuery(query): ApiQuery<PrQuery>,
) -> Result<axum::Json<PrItem>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;
    let (mut tx, _) = begin_queue(&state, &grant, &queue_id).await?;
    let number = i64::try_from(pr_number).map_err(|_| ApiError::not_found("PR"))?;
    let row: Option<ItemRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {ITEM_COLUMNS} FROM queue_items WHERE queue_id = $1 AND pr_number = $2
         ORDER BY submitted_at DESC, id DESC LIMIT 1"
    )))
    .bind(&queue_id)
    .bind(number)
    .fetch_optional(&mut *tx)
    .await?;
    let item = row.ok_or_else(|| ApiError::not_found("PR"))?.into_item()?;
    let timeline = match query.include.as_deref() {
        Some("timeline") => Some(timeline(&mut tx, &queue_id, &item.id).await?),
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
    path = "/queues/{queue_id}/prs:batchGet",
    params(("queue_id" = String, Path)),
    request_body = BatchGet,
    responses((status = 200, body = BatchGetResponse)),
    security(("token" = ["queues:read"]))
)]
async fn batch_get(
    grant: ApiGrant,
    axum::extract::State(state): axum::extract::State<ApiState>,
    ApiPath(queue_id): ApiPath<String>,
    ApiJson(request): ApiJson<BatchGet>,
) -> Result<axum::Json<BatchGetResponse>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;
    if request.pr_numbers.len() > MAX_BATCH_GET {
        return Err(ApiError::invalid(format!(
            "at most {MAX_BATCH_GET} PRs in one read"
        )));
    }
    let (mut tx, _) = begin_queue(&state, &grant, &queue_id).await?;
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
    .bind(&queue_id)
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
    path = "/queues/{queue_id}/graph",
    params(("queue_id" = String, Path)),
    responses((status = 200, body = Graph)),
    security(("token" = ["queues:read"]))
)]
async fn graph(
    grant: ApiGrant,
    axum::extract::State(state): axum::extract::State<ApiState>,
    ApiPath(queue_id): ApiPath<String>,
) -> Result<axum::Json<Graph>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;
    let (mut tx, _) = begin_queue(&state, &grant, &queue_id).await?;
    let rows: Vec<(String, i64, String, Option<i64>)> = sqlx::query_as(
        "SELECT id, pr_number, state::text, position FROM queue_items
         WHERE queue_id = $1 AND is_current
           AND state NOT IN ('merged', 'failed', 'cancelled')
         ORDER BY position NULLS LAST, submitted_at",
    )
    .bind(&queue_id)
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
    .bind(&queue_id)
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

/// The live item `id` of the organization: its queue and its state.
async fn item_of(
    conn: &mut PgConnection,
    id: &str,
) -> Result<(QueueId, ItemId, ItemState), ApiError> {
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT queue_id, state::text FROM queue_items WHERE id = $1")
            .bind(id)
            .fetch_optional(conn)
            .await?;
    let (queue, state) = row.ok_or_else(|| ApiError::not_found("item"))?;
    Ok((queue.parse()?, id.parse()?, state.parse()?))
}

async fn give(
    state: &ApiState,
    grant: &ApiGrant,
    item: &str,
    kind: impl FnOnce(ItemId, ItemState) -> Result<EventKind, ApiError>,
) -> Result<(StatusCode, axum::Json<Accepted>), ApiError> {
    grant.require(ApiScope::PrsWrite)?;
    let mut tx = begin(state, grant).await?;
    let (queue_id, item_id, item_state) = item_of(&mut tx, item).await?;
    let fact = kind(item_id, item_state)?;
    give_back(
        &mut tx,
        grant.org_id,
        queue_id,
        vec![(Actor::System, fact)],
        (state.clock)(),
    )
    .await?;
    tx.commit().await?;
    Ok(accepted())
}

/// Cancels an item (`admin_requested`).
#[utoipa::path(
    post,
    path = "/items/{item_id}/cancel",
    params(("item_id" = String, Path)),
    responses((status = 202, body = Accepted)),
    security(("token" = ["prs:write"]))
)]
async fn cancel_item(
    grant: ApiGrant,
    axum::extract::State(state): axum::extract::State<ApiState>,
    ApiPath(item_id): ApiPath<String>,
) -> Result<(StatusCode, axum::Json<Accepted>), ApiError> {
    give(&state, &grant, &item_id, |item_id, item_state| {
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

/// Restarts the tests of an item in test (`tests_restarted_by_api_request`).
#[utoipa::path(
    post,
    path = "/items/{item_id}/restart",
    params(("item_id" = String, Path)),
    responses((status = 202, body = Accepted)),
    security(("token" = ["prs:write"]))
)]
async fn restart_item(
    grant: ApiGrant,
    axum::extract::State(state): axum::extract::State<ApiState>,
    ApiPath(item_id): ApiPath<String>,
) -> Result<(StatusCode, axum::Json<Accepted>), ApiError> {
    give(&state, &grant, &item_id, |item_id, item_state| {
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

/// The detail of a run (`DECISIONS.md` D115).
#[utoipa::path(
    get,
    path = "/test-runs/{run_id}",
    params(("run_id" = String, Path)),
    responses((status = 200, body = TestRun)),
    security(("token" = ["queues:read"]))
)]
async fn test_run(
    grant: ApiGrant,
    axum::extract::State(state): axum::extract::State<ApiState>,
    ApiPath(run_id): ApiPath<String>,
) -> Result<axum::Json<TestRun>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;
    let _: RunId = run_id.parse().map_err(|_| ApiError::not_found("run"))?;
    let mut tx = begin(&state, &grant).await?;
    let run: RunRow = sqlx::query_as(
        "SELECT id, queue_id, group_id, attempt, status::text, status_reason::text, test_branch,
                test_sha, base_sha, draft_pr_number, tested_pr_numbers, head_shas,
                required_statuses, required_statuses_source::text, started_at, ended_at,
                last_report_at, failure_reason::text, failure_causing_statuses, conflict_culprit
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

/// What the configuration of the SCM misses for the queue (`DECISIONS.md`
/// D162): no finding means a sound configuration.
#[utoipa::path(
    get,
    path = "/queues/{queue_id}/config-check",
    params(("queue_id" = String, Path)),
    responses((status = 200, body = ConfigCheck)),
    security(("token" = ["queues:read"]))
)]
async fn config_check(
    grant: ApiGrant,
    axum::extract::State(state): axum::extract::State<ApiState>,
    ApiPath(queue_id): ApiPath<String>,
) -> Result<axum::Json<ConfigCheck>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;
    let (mut tx, row) = begin_queue(&state, &grant, &queue_id).await?;
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

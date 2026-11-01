use std::collections::HashMap;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use cihub_api_types::{
    ErrorCode, Finding, Graph, GraphEdge, GraphNode, GraphRun, ItemCounts, Queue, QueueList,
    RequestedQueueState, SettingsSummary, Setup, UpdateQueue, Window,
};
use cihub_config::{QueueSettings, RepoConfig};
use cihub_domain::{ApiScope, EventKind, QueueControl, QueueId, RepoId};
use cihub_mq_core::current_window;
use cihub_platform::{ApiError, ApiGrant, ApiJson, ApiPath, ApiQuery, Page, cursor_of};
use cihub_store::{Postgres, Transaction};
use serde::Deserialize;
use serde_json::json;
use sqlx::PgConnection;
use time::{Duration, OffsetDateTime};

use super::{
    ApiState, QueueErrors, QueueListErrors, QueuePath, RepoPath, UpdateQueueErrors, actor, audit,
    begin_repo, name_part,
};
use crate::jobs::give_back;
use crate::target::queue_settings;

#[derive(sqlx::FromRow)]
pub(super) struct QueueRow {
    pub(super) id: String,
    repo_id: String,
    repo: String,
    target_branch: String,
    is_default_branch: bool,
    state: String,
    mode: String,
    state_changed_at: OffsetDateTime,
    state_changed_by: Option<String>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
    target_head_sha: Option<String>,
    pub(super) window_size: Option<i32>,
    is_suspended: bool,
    config_text: Option<String>,
    config_sha: Option<String>,
    config_error: Option<String>,
}

/// The columns of [`QueueRow`], read from [`QUEUE_SOURCE`].
pub(super) const QUEUE_COLUMNS: &str = "q.id, q.repo_id, r.name AS repo, q.target_branch,
    q.target_branch = r.default_branch AS is_default_branch, q.state::text AS state, q.mode::text AS mode,
    coalesce(q.state_changed_at, q.created_at) AS state_changed_at,
    u.login AS state_changed_by, q.created_at, q.updated_at, q.target_head_sha, q.window_size,
    (c.suspended_at IS NOT NULL OR r.removed_at IS NOT NULL) AS is_suspended, q.config_text, q.config_sha, q.config_error";

pub(super) const QUEUE_SOURCE: &str = "queues q
    JOIN repos r ON r.id = q.repo_id
    JOIN scm_connections c ON c.id = r.connection_id
    LEFT JOIN users u ON u.id = q.state_changed_by";

impl QueueRow {
    /// The settings of the last valid file of the queue.
    pub(super) fn settings(&self) -> Result<QueueSettings, ApiError> {
        let config = self
            .config_text
            .as_deref()
            .map(RepoConfig::parse)
            .transpose()?;

        Ok(queue_settings(config.as_ref(), &self.target_branch)?)
    }
}

/// The settings of a queue that its page shows.
pub(super) fn settings_summary(settings: &QueueSettings) -> SettingsSummary {
    SettingsSummary {
        batching_enabled: settings.batching_enabled.value,
        batching_size: settings.batching_size.value,
        batching_max_wait_minutes: settings.batching_max_wait_minutes.value,
        failure_budget_retries: settings.failure_budget_retries.value,
        merge_method: settings.merge_method.value,
        enqueue_label: settings
            .label_commands_enabled
            .value
            .then(|| settings.enqueue_label.value.clone()),
    }
}

/// The queue of the branch `branch` of the repository `repo`.
pub(super) async fn queue_row(
    conn: &mut PgConnection,
    repo: &RepoId,
    branch: &str,
) -> Result<QueueRow, ApiError> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {QUEUE_COLUMNS} FROM {QUEUE_SOURCE} WHERE q.repo_id = $1 AND q.target_branch = $2"
    )))
    .bind(repo.to_string())
    .bind(branch)
    .fetch_optional(conn)
    .await?
    .ok_or_else(|| ApiError::new(ErrorCode::QueueNotFound, format!("no queue of `{branch}`")))
}

/// A transaction of the organization of the URL, and the queue that the URL
/// names, read in it.
pub(super) async fn begin_queue(
    state: &ApiState,
    grant: &ApiGrant,
    path: &QueuePath,
) -> Result<(Transaction<'static, Postgres>, QueueRow), ApiError> {
    let (mut tx, repo) = begin_repo(state, grant, &path.org, &path.repo).await?;
    let row = queue_row(&mut tx, &repo, &path.queue).await?;

    Ok((tx, row))
}

/// The counters of the items of a queue, read from [`COUNT_SOURCE`].
#[derive(sqlx::FromRow)]
struct CountsRow {
    not_ready: i64,
    pending: i64,
    testing: i64,
    pending_failure: i64,
    tests_passed: i64,
    merged_last_24h: i64,
    failed_last_24h: i64,
    failed_or_cancelled_last_24h: i64,
    last_merged_at: Option<OffsetDateTime>,
}

/// The lookups of [`CountsRow`] for each queue `q`, with the time of the
/// request as `$1`, to write after `q`. Like the graph and the commands, the
/// counters read only the current item of each PR. Each lookup reads one
/// index rather than the history of the queue, `queue_items_order` or
/// `queue_items_ended`: their predicates match only states written as
/// literals.
const COUNT_SOURCE: &str = "CROSS JOIN LATERAL (
         SELECT count(*) FILTER (WHERE i.state = 'not_ready') AS not_ready,
                count(*) FILTER (WHERE i.state = 'pending') AS pending,
                count(*) FILTER (WHERE i.state = 'testing') AS testing,
                count(*) FILTER (WHERE i.state = 'pending_failure') AS pending_failure,
                count(*) FILTER (WHERE i.state = 'tests_passed') AS tests_passed
         FROM queue_items i
         WHERE i.queue_id = q.id AND i.is_current
           AND i.state NOT IN ('merged', 'failed', 'cancelled')
     ) live
     CROSS JOIN LATERAL (
         SELECT count(*) FILTER (WHERE i.state = 'merged') AS merged_last_24h,
                count(*) FILTER (WHERE i.state = 'failed') AS failed_last_24h,
                count(*) FILTER (WHERE i.state IN ('failed', 'cancelled'))
                    AS failed_or_cancelled_last_24h
         FROM queue_items i
         WHERE i.queue_id = q.id AND i.is_current
           AND i.state IN ('merged', 'failed', 'cancelled')
           AND i.state_changed_at > $1 - interval '24 hours'
     ) ended
     CROSS JOIN LATERAL (
         SELECT max(i.state_changed_at) AS last_merged_at
         FROM queue_items i
         WHERE i.queue_id = q.id AND i.is_current AND i.state = 'merged'
     ) merged";

#[derive(sqlx::FromRow)]
struct CountedQueueRow {
    #[sqlx(flatten)]
    queue: QueueRow,
    #[sqlx(flatten)]
    counts: CountsRow,
}

/// What a queue shows besides its row and its counters: its runs in progress,
/// the median duration of its runs, and its last setup read.
struct QueueFacts {
    live_runs: HashMap<String, i64>,
    medians: HashMap<String, u32>,
    setups: HashMap<String, Setup>,
}

impl QueueFacts {
    async fn read(
        conn: &mut PgConnection,
        queues: &[String],
        now: OffsetDateTime,
    ) -> Result<Self, ApiError> {
        let live_runs = sqlx::query_as(
            "SELECT queue_id, count(*) FROM test_runs
             WHERE queue_id = ANY($1) AND status IN ('assembling', 'in_progress')
             GROUP BY queue_id",
        )
        .bind(queues)
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .collect();
        let setups: Vec<(String, sqlx::types::Json<Vec<Finding>>, OffsetDateTime)> =
            sqlx::query_as(
                "SELECT queue_id, findings, checked_at FROM queue_setups
                 WHERE queue_id = ANY($1)",
            )
            .bind(queues)
            .fetch_all(&mut *conn)
            .await?;
        let medians = median_run_seconds(conn, queues, now).await?;

        let setups = setups
            .into_iter()
            .map(|(queue, findings, checked_at)| {
                let setup = Setup {
                    findings: findings.0,
                    checked_at: Some(checked_at),
                };
                (queue, setup)
            })
            .collect();
        Ok(Self {
            live_runs,
            medians,
            setups,
        })
    }

    fn queue(&mut self, row: CountedQueueRow) -> Result<Queue, ApiError> {
        let CountedQueueRow { queue: row, counts } = row;
        let settings = row.settings()?;
        let window_size = row.window_size.map(u32::try_from).transpose()?;
        let setup = self.setups.remove(&row.id).unwrap_or(Setup {
            findings: Vec::new(),
            checked_at: None,
        });

        Ok(Queue {
            state: row.state.parse()?,
            mode: row.mode.parse()?,
            strategy: settings.strategy.value,
            is_suspended: row.is_suspended,
            has_config_file: row.config_text.is_some() || row.config_error.is_some(),
            counts: ItemCounts {
                not_ready: u32::try_from(counts.not_ready)?,
                pending: u32::try_from(counts.pending)?,
                testing: u32::try_from(counts.testing)?,
                pending_failure: u32::try_from(counts.pending_failure)?,
                tests_passed: u32::try_from(counts.tests_passed)?,
            },
            merged_last_24h: u32::try_from(counts.merged_last_24h)?,
            failed_last_24h: u32::try_from(counts.failed_last_24h)?,
            failed_or_cancelled_last_24h: u32::try_from(counts.failed_or_cancelled_last_24h)?,
            last_merged_at: counts.last_merged_at,
            live_runs: u32::try_from(self.live_runs.get(&row.id).copied().unwrap_or(0))?,
            median_run_seconds: self.medians.get(&row.id).copied(),
            window: Window {
                current: current_window(window_size, &settings),
                floor: settings.window_floor.value,
                ceiling: settings.window_ceiling.value,
            },
            settings: settings_summary(&settings),
            setup,
            state_changed_at: row.state_changed_at,
            state_changed_by: row.state_changed_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
            target_head_sha: row.target_head_sha,
            id: row.id,
            repo_id: row.repo_id,
            repo: row.repo,
            target_branch: row.target_branch,
            is_default_branch: row.is_default_branch,
            config_sha: row.config_sha,
            config_error: row.config_error,
        })
    }
}

/// The sort key of a queue in a list: the name of its repository, its
/// target branch and its identifier.
type QueueKey = (String, String, String);

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct QueueQuery {
    /// Only the queues of the repositories whose name holds this text,
    /// whatever its case; from 1 to 100 characters.
    #[param(example = "web")]
    q: Option<String>,
    /// The `cursor` of the previous page.
    from: Option<String>,
    /// How many queues, from 1 to 100; 50 by default.
    limit: Option<u32>,
}

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct PageQuery {
    /// The `cursor` of the previous page.
    from: Option<String>,
    /// How many queues, from 1 to 100; 50 by default.
    limit: Option<u32>,
}

/// List the queues of the organization
///
/// Every queue of the organization, with its counters and its settings, by
/// repository name then target branch: the overview of the dashboard.
#[utoipa::path(
    get,
    path = "/orgs/{org}/queues",
    tag = "Queues",
    params(
        ("org" = String, Path, description = "The slug of the organization.", example = "acme"),
        QueueQuery
    ),
    responses((status = 200, description = "A page of queues.", body = QueueList), QueueListErrors),
    security(("token" = ["queues:read"]))
)]
pub(super) async fn list_org_queues(
    grant: ApiGrant,
    State(state): State<ApiState>,
    ApiPath(org): ApiPath<String>,
    ApiQuery(query): ApiQuery<QueueQuery>,
) -> Result<Json<QueueList>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;
    let page = Page::read("queues", query.from.as_deref(), query.limit)?;

    let name = name_part(query.q)?;

    let mut tx = grant.begin_in_org(&state.pool, &org).await?;

    let scope = QueueScope {
        repo: None,
        name,
        repos: grant.repo_filter(),
    };
    let list = queue_list(&mut tx, &scope, &page, (state.clock)()).await?;
    Ok(Json(list))
}

/// List the queues of a repository
///
/// The queues of the repository with their counters and their settings, by
/// target branch. The first `/cihub merge` on a PR of a branch creates its queue.
#[utoipa::path(
    get,
    path = "/orgs/{org}/repos/{repo}/queues",
    tag = "Queues",
    params(RepoPath, PageQuery),
    responses((status = 200, description = "A page of queues.", body = QueueList), QueueListErrors),
    security(("token" = ["queues:read"]))
)]
pub(super) async fn list_repo_queues(
    grant: ApiGrant,
    State(state): State<ApiState>,
    ApiPath(path): ApiPath<RepoPath>,
    ApiQuery(query): ApiQuery<PageQuery>,
) -> Result<Json<QueueList>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;
    let page = Page::read("queues", query.from.as_deref(), query.limit)?;

    let (mut tx, repo) = begin_repo(&state, &grant, &path.org, &path.repo).await?;

    let scope = QueueScope {
        repo: Some(repo.to_string()),
        name: None,
        repos: None,
    };
    let list = queue_list(&mut tx, &scope, &page, (state.clock)()).await?;
    Ok(Json(list))
}

/// Which queues of the organization of the transaction a list gives: those
/// of a repository, of the repositories whose name holds `name`, and of the
/// repositories that a person reads (`None` for every one).
struct QueueScope {
    repo: Option<String>,
    name: Option<String>,
    repos: Option<Vec<String>>,
}

/// The page of the queues that `scope` keeps.
async fn queue_list(
    conn: &mut PgConnection,
    scope: &QueueScope,
    page: &Page<QueueKey>,
    now: OffsetDateTime,
) -> Result<QueueList, ApiError> {
    let QueueScope { repo, name, repos } = scope;
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM queues q JOIN repos r ON r.id = q.repo_id
         WHERE ($1::text IS NULL OR q.repo_id = $1)
           AND ($2::text IS NULL OR strpos(lower(r.name), lower($2)) > 0)
           AND ($3::text[] IS NULL OR q.repo_id = ANY($3))",
    )
    .bind(repo)
    .bind(name)
    .bind(repos)
    .fetch_one(&mut *conn)
    .await?;
    let (after_repo, after_name, after_id) = match page.after.clone() {
        Some((repo, name, id)) => (Some(repo), Some(name), Some(id)),
        None => (None, None, None),
    };
    // The page first: the counters of a queue cost a few lookups each.
    let rows: Vec<CountedQueueRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT q.*, live.*, ended.*, merged.*
         FROM (
             SELECT {QUEUE_COLUMNS}
             FROM {QUEUE_SOURCE}
             WHERE ($2::text IS NULL OR q.repo_id = $2)
               AND ($3::text IS NULL OR strpos(lower(r.name), lower($3)) > 0)
               AND ($4::text[] IS NULL OR q.repo_id = ANY($4))
               AND ($5::text IS NULL OR (r.name, q.target_branch, q.id) > ($5, $6, $7))
             ORDER BY r.name, q.target_branch, q.id
             LIMIT $8
         ) q
         {COUNT_SOURCE}
         ORDER BY q.repo, q.target_branch, q.id"
    )))
    .bind(now)
    .bind(repo)
    .bind(name)
    .bind(repos)
    .bind(after_repo)
    .bind(after_name)
    .bind(after_id)
    .bind(page.rows())
    .fetch_all(&mut *conn)
    .await?;
    let (rows, has_more) = page.split(rows);
    let ids: Vec<String> = rows.iter().map(|row| row.queue.id.clone()).collect();
    let mut facts = QueueFacts::read(conn, &ids, now).await?;

    let queues = rows
        .into_iter()
        .map(|row| facts.queue(row))
        .collect::<Result<Vec<_>, _>>()?;
    let cursor = queues
        .last()
        .filter(|_| has_more)
        .map(|queue| cursor_of("queues", (&queue.repo, &queue.target_branch, &queue.id)));
    Ok(QueueList {
        queues,
        has_more,
        count: u64::try_from(count)?,
        cursor,
    })
}

/// Get a queue
///
/// The queue with its counters, its window, its settings and the last check
/// of its configuration: the page of the queue in the dashboard.
#[utoipa::path(
    get,
    path = "/orgs/{org}/repos/{repo}/queues/{queue}",
    tag = "Queues",
    params(QueuePath),
    responses((status = 200, description = "The queue.", body = Queue), QueueErrors),
    security(("token" = ["queues:read"]))
)]
pub(super) async fn get_queue(
    grant: ApiGrant,
    State(state): State<ApiState>,
    ApiPath(path): ApiPath<QueuePath>,
) -> Result<Json<Queue>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;

    let (mut tx, queue) = begin_queue(&state, &grant, &path).await?;
    let now = (state.clock)();
    let counts: CountsRow = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT live.*, ended.*, merged.* FROM (SELECT $2::text AS id) q {COUNT_SOURCE}"
    )))
    .bind(now)
    .bind(&queue.id)
    .fetch_one(&mut *tx)
    .await?;
    let mut facts = QueueFacts::read(&mut tx, std::slice::from_ref(&queue.id), now).await?;

    let queue = facts.queue(CountedQueueRow { queue, counts })?;
    Ok(Json(queue))
}

/// Change the state of a queue
///
/// Asks the queue for `running`, `paused` or `draining`; the queue changes
/// at its next reconciliation. A queue that lost its access takes the state
/// and stays stopped until the access comes back.
#[utoipa::path(
    patch,
    path = "/orgs/{org}/repos/{repo}/queues/{queue}",
    tag = "Queues",
    params(QueuePath),
    request_body = UpdateQueue,
    responses(
        (status = 202, description = "The queue takes the request."),
        UpdateQueueErrors
    ),
    security(("token" = ["queues:write"]))
)]
pub(super) async fn update_queue(
    grant: ApiGrant,
    State(state): State<ApiState>,
    ApiPath(path): ApiPath<QueuePath>,
    ApiJson(request): ApiJson<UpdateQueue>,
) -> Result<StatusCode, ApiError> {
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
    tx.commit().await?;

    Ok(StatusCode::ACCEPTED)
}

/// Sync a queue
///
/// Reads again from the SCM each live PR, the checks of each live run, the
/// head of the target branch and its configuration: the way out after an
/// incident.
#[utoipa::path(
    post,
    path = "/orgs/{org}/repos/{repo}/queues/{queue}/sync",
    tag = "Queues",
    params(QueuePath),
    responses((status = 202, description = "The queue takes the request."), QueueErrors),
    security(("token" = ["queues:write"]))
)]
pub(super) async fn sync_queue(
    grant: ApiGrant,
    State(state): State<ApiState>,
    ApiPath(path): ApiPath<QueuePath>,
) -> Result<StatusCode, ApiError> {
    grant.require(ApiScope::QueuesWrite)?;

    let (mut tx, row) = begin_queue(&state, &grant, &path).await?;

    audit(&mut tx, &grant, "queue.sync", &row.id, json!({})).await?;
    tx.commit().await?;

    let id: QueueId = row.id.parse()?;
    crate::sync_queue(&state.pool, id, (state.clock)()).await?;

    Ok(StatusCode::ACCEPTED)
}

/// Get the merge graph
///
/// The head of the target branch, the live items in order, the predecessors
/// that the test of each one merges before it, and the last run of each live
/// group.
#[utoipa::path(
    get,
    path = "/orgs/{org}/repos/{repo}/queues/{queue}/graph",
    tag = "Queues",
    params(QueuePath),
    responses((status = 200, description = "The merge graph.", body = Graph), QueueErrors),
    security(("token" = ["queues:read"]))
)]
pub(super) async fn graph(
    grant: ApiGrant,
    State(state): State<ApiState>,
    ApiPath(path): ApiPath<QueuePath>,
) -> Result<Json<Graph>, ApiError> {
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
    let edges = groups
        .iter()
        .flat_map(|(members, predecessors)| {
            members.iter().flat_map(move |to| {
                predecessors.iter().map(move |from| GraphEdge {
                    from: from.clone(),
                    to: to.clone(),
                })
            })
        })
        .collect();

    // The last run of each live group, looked up by group rather than joined
    // with every run of the organization.
    let rows: Vec<GraphRunRow> = sqlx::query_as(
        "SELECT r.id AS run_id, g.item_ids, g.predecessor_ids, r.attempt,
                r.status::text AS status, r.started_at
         FROM test_groups g
         CROSS JOIN LATERAL (
             SELECT run.id, run.attempt, run.status, run.started_at FROM test_runs run
             WHERE run.group_id = g.id ORDER BY run.attempt DESC LIMIT 1
         ) r
         WHERE g.queue_id = $1 AND g.state NOT IN ('merged', 'failed', 'bisected', 'dissolved')
         ORDER BY r.started_at, r.id",
    )
    .bind(&queue.id)
    .fetch_all(&mut *tx)
    .await?;
    let runs = rows
        .into_iter()
        .map(GraphRunRow::into_graph_run)
        .collect::<Result<_, _>>()?;

    Ok(Json(Graph {
        target_head_sha: queue.target_head_sha,
        nodes,
        edges,
        runs,
    }))
}

#[derive(sqlx::FromRow)]
struct GraphRunRow {
    run_id: String,
    item_ids: Vec<String>,
    predecessor_ids: Vec<String>,
    attempt: i32,
    status: String,
    started_at: OffsetDateTime,
}

impl GraphRunRow {
    fn into_graph_run(self) -> Result<GraphRun, ApiError> {
        Ok(GraphRun {
            attempt: u32::try_from(self.attempt)?,
            status: self.status.parse()?,
            run_id: self.run_id,
            item_ids: self.item_ids,
            predecessor_ids: self.predecessor_ids,
            started_at: self.started_at,
        })
    }
}

/// A duration in seconds from Postgres, rounded to the second.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a run lasts from 0 seconds to a few days"
)]
pub(super) fn seconds_of(seconds: f64) -> u32 {
    seconds.round().max(0.0) as u32
}

/// The median duration of the runs that succeeded in the last 24 hours, in
/// seconds, of each of `queues` that has one.
pub(super) async fn median_run_seconds(
    conn: &mut PgConnection,
    queues: &[String],
    now: OffsetDateTime,
) -> Result<HashMap<String, u32>, ApiError> {
    let rows: Vec<(String, f64)> = sqlx::query_as(
        "SELECT queue_id,
                percentile_cont(0.5) WITHIN GROUP (ORDER BY extract(epoch FROM ended_at - started_at))
         FROM test_runs
         WHERE queue_id = ANY($1) AND status = 'succeeded' AND ended_at > $2
         GROUP BY queue_id",
    )
    .bind(queues)
    .bind(now - Duration::hours(24))
    .fetch_all(conn)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(queue, seconds)| (queue, seconds_of(seconds)))
        .collect())
}

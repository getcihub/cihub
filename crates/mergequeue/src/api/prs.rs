use std::collections::HashMap;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use cihub_api_types::{ErrorCode, Event, EventList, Person, Pr, PrAhead, PrList, SubmitPr};
use cihub_domain::{
    ApiScope, EventKind, ItemId, ItemState, Priority, QueueId, Reason, RepoId, SubmittedVia,
};
use cihub_mq_core::current_window;
use cihub_platform::{
    ApiError, ApiGrant, ApiJson, ApiPath, ApiQuery, Job, Page, cursor_of, enqueue_job,
};
use cihub_store::event_kind;
use serde::{Deserialize, Deserializer};
use serde_json::{Value, json};
use sqlx::{PgConnection, Postgres, QueryBuilder};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use super::queues::{
    QUEUE_COLUMNS, QUEUE_SOURCE, QueueRow, median_run_seconds, queue_row, seconds_of,
};
use super::{
    ApiState, CancelErrors, EventErrors, OrgPrListErrors, PrErrors, PrPath, RepoPath,
    RepoPrListErrors, RestartErrors, SubmitErrors, actor, audit, begin_repo, requester,
};
use crate::MergeQueueError;
use crate::jobs::{self, give_back, new_event_id};
use crate::surfaces::{blocks_sentence, transition_sentence};
use crate::target::RepoTarget;

#[derive(sqlx::FromRow)]
struct ItemRow {
    id: String,
    queue_id: String,
    repo: String,
    queue: String,
    pr_number: i64,
    pr_head_sha: String,
    state: String,
    state_reason: String,
    state_changed_at: OffsetDateTime,
    priority_value: i16,
    position: Option<i64>,
    block_reasons: Value,
    group_id: Option<String>,
    run_id: Option<String>,
    merged_sha: Option<String>,
    no_batch: bool,
    profile: Option<String>,
    submitted_by: Option<String>,
    submitted_via: String,
    submitted_at: OffsetDateTime,
    title: Option<String>,
    pr_url: Option<String>,
    author: String,
    author_avatar_url: Option<String>,
}

/// The columns of [`ItemRow`], read from the items `i` and [`ITEM_JOINS`]. The
/// title is a lookup by PR rather than a join, which would read every title
/// of the organization.
const ITEM_COLUMNS: &str = "i.id, i.queue_id, r.name AS repo, q.target_branch AS queue,
    i.pr_number, i.pr_head_sha, i.state::text AS state, i.state_reason::text AS state_reason,
    i.state_changed_at, i.priority_value, i.position, i.block_reasons, i.group_id,
    (SELECT run.id FROM test_runs run WHERE run.group_id = i.group_id
     ORDER BY run.attempt DESC LIMIT 1) AS run_id,
    i.merged_sha, i.no_batch, i.profile, submitter.login AS submitted_by,
    i.submitted_via::text AS submitted_via, i.submitted_at,
    (SELECT t.title FROM pr_titles t WHERE t.repo_id = q.repo_id AND t.pr_number = i.pr_number)
        AS title,
    i.pr_url,
    author.login AS author, author.avatar_url AS author_avatar_url";

const ITEM_JOINS: &str = "JOIN queues q ON q.id = i.queue_id
    JOIN repos r ON r.id = q.repo_id
    JOIN users author ON author.id = i.pr_author_id
    LEFT JOIN users submitter ON submitter.id = i.submitted_by";

/// An admitted PR of a queue of the page.
#[derive(sqlx::FromRow)]
struct AdmittedRow {
    queue_id: String,
    pr_number: i64,
    state: String,
    position: i64,
}

/// What the estimate of a queue needs: its window and the median duration of
/// its runs.
struct QueuePace {
    window: u32,
    median_run_seconds: Option<u32>,
}

/// What the PRs of a page show besides their rows: the admitted PRs of their
/// queues, the pace of their queues, and when their runs started.
struct Progress {
    admitted: Vec<AdmittedRow>,
    paces: HashMap<String, QueuePace>,
    started: HashMap<String, OffsetDateTime>,
    now: OffsetDateTime,
}

impl Progress {
    async fn read(
        conn: &mut PgConnection,
        rows: &[ItemRow],
        now: OffsetDateTime,
    ) -> Result<Self, ApiError> {
        let queues: Vec<String> = rows.iter().map(|row| row.queue_id.clone()).collect();
        let runs: Vec<String> = rows.iter().filter_map(|row| row.run_id.clone()).collect();
        let admitted = sqlx::query_as(
            "SELECT queue_id, pr_number, state::text AS state, position FROM queue_items
             WHERE queue_id = ANY($1) AND is_current AND position IS NOT NULL
               AND state NOT IN ('merged', 'failed', 'cancelled')
             ORDER BY queue_id, position",
        )
        .bind(&queues)
        .fetch_all(&mut *conn)
        .await?;
        let started = sqlx::query_as("SELECT id, started_at FROM test_runs WHERE id = ANY($1)")
            .bind(&runs)
            .fetch_all(&mut *conn)
            .await?
            .into_iter()
            .collect();
        let paces = paces(conn, &queues, now).await?;

        Ok(Self {
            admitted,
            paces,
            started,
            now,
        })
    }

    /// The PR of `row`, the admitted PRs of its queue before it (all of them
    /// for a PR not admitted yet, none for a merged one), and its estimate.
    fn pr(&self, row: ItemRow) -> Result<Pr, ApiError> {
        let is_merged = row.state == "merged";
        let before = row.position.unwrap_or(i64::MAX);
        let ahead = self
            .admitted
            .iter()
            .filter(|other| !is_merged && other.queue_id == row.queue_id && other.position < before)
            .map(|other| {
                Ok(PrAhead {
                    pr_number: u64::try_from(other.pr_number)?,
                    state: other.state.parse()?,
                })
            })
            .collect::<Result<Vec<_>, ApiError>>()?;
        let elapsed = row
            .run_id
            .as_ref()
            .and_then(|run| self.started.get(run))
            .map(|start| seconds_of((self.now - *start).as_seconds_f64()));
        let state: ItemState = row.state.parse()?;
        let rank = u32::try_from(ahead.len() + 1)?;
        let merges_in_seconds = self
            .paces
            .get(&row.queue_id)
            .and_then(|pace| merges_in(state, rank, pace, elapsed));
        let state_reason: Reason = row.state_reason.parse()?;

        Ok(Pr {
            number: u64::try_from(row.pr_number)?,
            state,
            state_reason,
            state_description: state_reason.describe().to_owned(),
            priority: Priority(u8::try_from(row.priority_value)?),
            block_reasons: serde_json::from_value(row.block_reasons)?,
            submitted_via: row.submitted_via.parse()?,
            ahead,
            merges_in_seconds,
            id: row.id,
            queue_id: row.queue_id,
            repo: row.repo,
            queue: row.queue,
            head_sha: row.pr_head_sha,
            state_changed_at: row.state_changed_at,
            position: row.position,
            group_id: row.group_id,
            run_id: row.run_id,
            merged_sha: row.merged_sha,
            no_batch: row.no_batch,
            profile: row.profile,
            submitted_by: row.submitted_by,
            submitted_at: row.submitted_at,
            title: row.title,
            url: row.pr_url,
            author: Person {
                login: row.author,
                avatar_url: row.author_avatar_url,
            },
        })
    }
}

/// The window and the median duration of the runs of each of `queues`.
async fn paces(
    conn: &mut PgConnection,
    queues: &[String],
    now: OffsetDateTime,
) -> Result<HashMap<String, QueuePace>, ApiError> {
    let rows: Vec<QueueRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {QUEUE_COLUMNS} FROM {QUEUE_SOURCE} WHERE q.id = ANY($1)"
    )))
    .bind(queues)
    .fetch_all(&mut *conn)
    .await?;
    let medians = median_run_seconds(conn, queues, now).await?;

    rows.into_iter()
        .map(|row| {
            let settings = row.settings()?;
            let window_size = row.window_size.map(u32::try_from).transpose()?;
            let pace = QueuePace {
                window: current_window(window_size, &settings),
                median_run_seconds: medians.get(&row.id).copied(),
            };
            Ok((row.id, pace))
        })
        .collect()
}

/// About how many seconds until an item in `state` merges: a pending PR waits
/// a run for each `window` PRs up to its `rank`, a tested one the rest of its
/// run. `None` for an item that does not move toward the merge, or for a queue
/// without a recent run.
fn merges_in(
    state: ItemState,
    rank: u32,
    pace: &QueuePace,
    elapsed_seconds: Option<u32>,
) -> Option<u32> {
    let median = pace.median_run_seconds?;

    match state {
        ItemState::Pending => Some(rank.div_ceil(pace.window.max(1)) * median),
        ItemState::Testing => Some(median.saturating_sub(elapsed_seconds.unwrap_or(0))),
        ItemState::TestsPassed => Some(0),
        _ => None,
    }
}

/// The order of a list of PRs.
#[derive(Clone, Copy, Default, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub(super) enum PrSort {
    /// The last submitted first.
    #[default]
    Newest,
    /// By repository, queue, then order in the queue: the head of each queue
    /// first, then the PRs not admitted yet, then the ended ones.
    Position,
}

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct PrQuery {
    /// Only the PRs of this author, whatever the case of the login.
    #[param(example = "octocat")]
    author: Option<String>,
    /// Only the PRs in these states, separated by commas: `not_ready`,
    /// `pending`, `testing`, `tests_passed`, `pending_failure`, `merged`,
    /// `failed`, `cancelled`.
    #[serde(default, deserialize_with = "item_states")]
    #[param(value_type = Option<String>, example = "pending,testing")]
    state: Vec<ItemState>,
    /// Submitted at or after this time (RFC 3339).
    #[serde(default, with = "time::serde::rfc3339::option")]
    #[param(value_type = Option<String>, format = DateTime)]
    submitted_since: Option<OffsetDateTime>,
    /// Submitted before this time (RFC 3339).
    #[serde(default, with = "time::serde::rfc3339::option")]
    #[param(value_type = Option<String>, format = DateTime)]
    submitted_until: Option<OffsetDateTime>,
    /// Reached their state at or after this time (RFC 3339).
    #[serde(default, with = "time::serde::rfc3339::option")]
    #[param(value_type = Option<String>, format = DateTime)]
    state_changed_since: Option<OffsetDateTime>,
    /// `newest` (by default) or `position`.
    #[serde(default)]
    #[param(inline)]
    sort: PrSort,
    /// The `cursor` of the previous page.
    from: Option<String>,
    /// How many PRs, from 1 to 100; 50 by default.
    limit: Option<u32>,
}

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct QueueFilter {
    /// Only the PRs of the queue of this target branch.
    #[param(example = "main")]
    queue: Option<String>,
}

fn item_states<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<ItemState>, D::Error> {
    let text = String::deserialize(deserializer)?;

    text.split(',')
        .map(|state| {
            state
                .parse()
                .map_err(|_| serde::de::Error::custom(format!("unknown state `{state}`")))
        })
        .collect()
}

/// The sort key of a PR in a list: by `newest`, its submission and its
/// identifier; by `position`, its repository, its queue and its position
/// before them.
type NewestKey = (String, String);
type PositionKey = (String, String, i64, String, String);

/// The filters of a list of PRs, which [`push_filter`] writes.
struct PrFilter {
    org: String,
    author: Option<String>,
    repo: Option<String>,
    /// The repositories that a person reads; `None` for every one.
    repos: Option<Vec<String>>,
    queue: Option<String>,
    states: Vec<ItemState>,
    submitted_since: Option<OffsetDateTime>,
    submitted_until: Option<OffsetDateTime>,
    state_changed_since: Option<OffsetDateTime>,
}

/// The conditions of `filter` on the items `i`: the current item of each PR
/// in each queue, then only the filters that the request gives, so that the
/// plan has no `$n IS NULL OR` to keep. The states are literals of the closed
/// vocabulary: the partial indexes of `queue_items` match only literals.
fn push_filter(sql: &mut QueryBuilder<Postgres>, filter: &PrFilter) {
    // Row level security keeps the organization. The condition serves only the
    // index of the newest items of the organization: with a narrower filter,
    // the planner would take that index instead of the narrower one.
    if filter.author.is_none() && filter.repo.is_none() && filter.queue.is_none() {
        sql.push("i.org_id = ")
            .push_bind(filter.org.clone())
            .push(" AND ");
    }
    sql.push("i.is_current");
    if let Some(author) = &filter.author {
        // An array that Postgres computes once, then looks up in the index of
        // the authors.
        sql.push(
            " AND i.pr_author_id = ANY (ARRAY(SELECT id FROM users WHERE lower(login) = lower(",
        )
        .push_bind(author.clone())
        .push(")))");
    }
    if let Some(repo) = &filter.repo {
        sql.push(" AND i.queue_id IN (SELECT id FROM queues WHERE repo_id = ")
            .push_bind(repo.clone())
            .push(")");
    }
    if let Some(queue) = &filter.queue {
        sql.push(" AND i.queue_id = ").push_bind(queue.clone());
    }
    if let Some(repos) = &filter.repos {
        sql.push(" AND i.queue_id IN (SELECT id FROM queues WHERE repo_id = ANY(")
            .push_bind(repos.clone())
            .push("))");
    }
    if !filter.states.is_empty() {
        sql.push(" AND i.state IN (");
        let mut states = sql.separated(", ");
        for state in &filter.states {
            states.push(format_args!("'{}'", state.as_str()));
        }
        sql.push(")");
    }
    if let Some(since) = filter.submitted_since {
        sql.push(" AND i.submitted_at >= ").push_bind(since);
    }
    if let Some(until) = filter.submitted_until {
        sql.push(" AND i.submitted_at < ").push_bind(until);
    }
    if let Some(since) = filter.state_changed_since {
        sql.push(" AND i.state_changed_at >= ").push_bind(since);
    }
}

/// List the PRs of the organization
///
/// The PRs in the queues of every repository of the organization: the
/// current submission of each PR in each queue, with the PRs that merge before
/// it and an estimate. Without a filter, every PR, live or ended.
#[utoipa::path(
    get,
    path = "/orgs/{org}/prs",
    tag = "Pull requests",
    params(
        ("org" = String, Path, description = "The slug of the organization.", example = "acme"),
        PrQuery
    ),
    responses((status = 200, description = "A page of PRs.", body = PrList), OrgPrListErrors),
    security(("token" = ["queues:read"]))
)]
pub(super) async fn list_org_prs(
    grant: ApiGrant,
    State(state): State<ApiState>,
    ApiPath(org): ApiPath<String>,
    ApiQuery(query): ApiQuery<PrQuery>,
) -> Result<Json<PrList>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;

    let mut tx = grant.begin_in_org(&state.pool, &org).await?;

    pr_list(&mut tx, &state, &grant, (None, None), &query).await
}

/// List the PRs of a repository
///
/// The PRs in the queues of the repository, or of one of its queues: the
/// current submission of each PR in each queue, with the PRs that merge
/// before it and an estimate. Without a filter, every PR, live or ended.
#[utoipa::path(
    get,
    path = "/orgs/{org}/repos/{repo}/prs",
    tag = "Pull requests",
    params(RepoPath, QueueFilter, PrQuery),
    responses((status = 200, description = "A page of PRs.", body = PrList), RepoPrListErrors),
    security(("token" = ["queues:read"]))
)]
pub(super) async fn list_repo_prs(
    grant: ApiGrant,
    State(state): State<ApiState>,
    ApiPath(path): ApiPath<RepoPath>,
    ApiQuery(filter): ApiQuery<QueueFilter>,
    ApiQuery(query): ApiQuery<PrQuery>,
) -> Result<Json<PrList>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;

    let (mut tx, repo) = begin_repo(&state, &grant, &path.org, &path.repo).await?;
    let queue = match &filter.queue {
        Some(name) => Some(queue_row(&mut tx, &repo, name).await?.id),
        None => None,
    };

    pr_list(&mut tx, &state, &grant, (Some(repo), queue), &query).await
}

/// The page of the PRs that `query` keeps, in the organization of the
/// transaction, or only in a repository and one of its queues.
async fn pr_list(
    tx: &mut PgConnection,
    state: &ApiState,
    grant: &ApiGrant,
    (repo, queue): (Option<RepoId>, Option<String>),
    query: &PrQuery,
) -> Result<Json<PrList>, ApiError> {
    let filter = PrFilter {
        org: grant.org_id.to_string(),
        author: query.author.clone(),
        repo: repo.map(|repo| repo.to_string()),
        repos: grant.repo_filter(),
        queue,
        states: query.state.clone(),
        submitted_since: query.submitted_since,
        submitted_until: query.submitted_until,
        state_changed_since: query.state_changed_since,
    };
    let mut count = QueryBuilder::new("SELECT count(*) FROM queue_items i WHERE ");
    push_filter(&mut count, &filter);
    let count: i64 = count.build_query_scalar().fetch_one(&mut *tx).await?;
    let now = (state.clock)();

    let (prs, has_more) = pr_page(tx, &filter, query).await?;
    let progress = Progress::read(tx, &prs, now).await?;

    let prs = prs
        .into_iter()
        .map(|row| progress.pr(row))
        .collect::<Result<Vec<_>, _>>()?;
    let cursor = prs
        .last()
        .filter(|_| has_more)
        .map(|pr| pr_cursor(pr, query.sort))
        .transpose()?;
    Ok(Json(PrList {
        prs,
        has_more,
        count: u64::try_from(count)?,
        cursor,
    }))
}

/// The place of a PR in a sort by position: its position, then the PRs not
/// admitted yet, then the ended ones, which can keep a position.
const PLACE: &str =
    "CASE WHEN i.state IN ('merged', 'failed', 'cancelled') THEN 9223372036854775807
    ELSE coalesce(i.position, 9223372036854775806) END";

/// The place of `pr` in a sort by position, as [`PLACE`] computes it.
fn place_of(pr: &Pr) -> i64 {
    if pr.state.is_terminal() {
        return i64::MAX;
    }

    pr.position.unwrap_or(i64::MAX - 1)
}

/// The page of the PRs that `filter` keeps, in the order that `query` asks
/// for, and whether another page follows.
async fn pr_page(
    conn: &mut PgConnection,
    filter: &PrFilter,
    query: &PrQuery,
) -> Result<(Vec<ItemRow>, bool), ApiError> {
    match query.sort {
        PrSort::Newest => {
            let page: Page<NewestKey> =
                Page::read("prs.newest", query.from.as_deref(), query.limit)?;

            // The page of the items first, by the index of the newest ones;
            // then what the page shows of them.
            let mut sql = QueryBuilder::new(format!(
                "SELECT {ITEM_COLUMNS} FROM (SELECT * FROM queue_items i WHERE "
            ));
            push_filter(&mut sql, filter);
            if let Some((at, id)) = page.after.clone() {
                sql.push(" AND (i.submitted_at, i.id) < (")
                    .push_bind(at)
                    .push("::timestamptz, ")
                    .push_bind(id)
                    .push(")");
            }
            sql.push(" ORDER BY i.submitted_at DESC, i.id DESC LIMIT ")
                .push_bind(page.rows())
                .push(format_args!(
                    ") i {ITEM_JOINS} ORDER BY i.submitted_at DESC, i.id DESC"
                ));
            let rows: Vec<ItemRow> = sql.build_query_as().fetch_all(&mut *conn).await?;
            Ok(page.split(rows))
        }
        PrSort::Position => {
            let page: Page<PositionKey> =
                Page::read("prs.position", query.from.as_deref(), query.limit)?;

            // The items of the filter first: the sort needs the names of
            // their repositories and queues, and a join first would make the
            // plan guess how many PRs an author has.
            let mut sql =
                QueryBuilder::new("WITH i AS MATERIALIZED (SELECT * FROM queue_items i WHERE ");
            push_filter(&mut sql, filter);
            sql.push(format_args!(") SELECT {ITEM_COLUMNS} FROM i {ITEM_JOINS}"));
            if let Some((repo, queue, position, at, id)) = page.after.clone() {
                sql.push(format_args!(
                    " WHERE (r.name, q.target_branch, {PLACE}, i.submitted_at, i.id) > ("
                ));
                let mut key = sql.separated(", ");
                key.push_bind(repo)
                    .push_bind(queue)
                    .push_bind(position)
                    .push_bind(at)
                    .push_unseparated("::timestamptz")
                    .push_bind(id);
                sql.push(")");
            }
            sql.push(format_args!(
                " ORDER BY r.name, q.target_branch, {PLACE}, i.submitted_at, i.id LIMIT "
            ))
            .push_bind(page.rows());
            let rows: Vec<ItemRow> = sql.build_query_as().fetch_all(&mut *conn).await?;
            Ok(page.split(rows))
        }
    }
}

/// The cursor of the page after `pr`, in a list sorted by `sort`.
fn pr_cursor(pr: &Pr, sort: PrSort) -> Result<String, ApiError> {
    let submitted_at = pr.submitted_at.format(&Rfc3339)?;

    Ok(match sort {
        PrSort::Newest => cursor_of("prs.newest", (submitted_at, &pr.id)),
        PrSort::Position => {
            let key = (&pr.repo, &pr.queue, place_of(pr), submitted_at, &pr.id);
            cursor_of("prs.position", key)
        }
    })
}

fn pr_not_found(number: u64) -> ApiError {
    ApiError::new(
        ErrorCode::PrNotFound,
        format!("PR {number} was never submitted to a queue of the repository"),
    )
}

/// The number of the PR, as Postgres keeps it.
fn stored(number: u64) -> Result<i64, ApiError> {
    i64::try_from(number).map_err(|_| pr_not_found(number))
}

/// Get a PR
///
/// The last submission of the PR to a queue of the repository, with the PRs
/// that merge before it and an estimate.
#[utoipa::path(
    get,
    path = "/orgs/{org}/repos/{repo}/prs/{number}",
    tag = "Pull requests",
    params(PrPath),
    responses((status = 200, description = "The PR.", body = Pr), PrErrors),
    security(("token" = ["queues:read"]))
)]
pub(super) async fn get_pr(
    grant: ApiGrant,
    State(state): State<ApiState>,
    ApiPath(path): ApiPath<PrPath>,
) -> Result<Json<Pr>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;

    let (mut tx, repo) = begin_repo(&state, &grant, &path.org, &path.repo).await?;
    let row: Option<ItemRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {ITEM_COLUMNS} FROM queue_items i {ITEM_JOINS} WHERE q.repo_id = $1 AND i.pr_number = $2
         ORDER BY i.submitted_at DESC, i.id DESC LIMIT 1"
    )))
    .bind(repo.to_string())
    .bind(stored(path.number)?)
    .fetch_optional(&mut *tx)
    .await?;
    let row = row.ok_or_else(|| pr_not_found(path.number))?;
    let progress = Progress::read(&mut tx, std::slice::from_ref(&row), (state.clock)()).await?;

    Ok(Json(progress.pr(row)?))
}

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct EventQuery {
    /// The `cursor` of the previous page.
    from: Option<String>,
    /// How many events, from 1 to 100; 50 by default.
    limit: Option<u32>,
}

/// The sort key of an event of a PR: its time, its sequence number and its
/// identifier.
type EventKey = (String, i64, String);

/// The submissions of the PR `$2` to the queues of the repository `$1`.
const PR_ITEMS: &str =
    "SELECT i.queue_id, i.id FROM queue_items i JOIN queues q ON q.id = i.queue_id
    WHERE q.repo_id = $1 AND i.pr_number = $2";

/// List the events of a PR
///
/// The history of the PR in the queues of the repository, every submission
/// together, oldest first.
#[utoipa::path(
    get,
    path = "/orgs/{org}/repos/{repo}/prs/{number}/events",
    tag = "Pull requests",
    params(PrPath, EventQuery),
    responses((status = 200, description = "A page of events.", body = EventList), EventErrors),
    security(("token" = ["queues:read"]))
)]
pub(super) async fn list_events(
    grant: ApiGrant,
    State(state): State<ApiState>,
    ApiPath(path): ApiPath<PrPath>,
    ApiQuery(query): ApiQuery<EventQuery>,
) -> Result<Json<EventList>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;
    let page: Page<EventKey> = Page::read("events", query.from.as_deref(), query.limit)?;

    let (mut tx, repo) = begin_repo(&state, &grant, &path.org, &path.repo).await?;
    let number = stored(path.number)?;
    let is_submitted: bool =
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT EXISTS ({PR_ITEMS})")))
            .bind(repo.to_string())
            .bind(number)
            .fetch_one(&mut *tx)
            .await?;
    if !is_submitted {
        return Err(pr_not_found(path.number));
    }

    let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT count(*) FROM events e WHERE (e.queue_id, e.item_id) IN ({PR_ITEMS})"
    )))
    .bind(repo.to_string())
    .bind(number)
    .fetch_one(&mut *tx)
    .await?;
    let rows = event_rows(&mut tx, &repo, number, &page).await?;
    let (rows, has_more) = page.split(rows);

    let events = rows
        .into_iter()
        .map(event_of)
        .collect::<Result<Vec<_>, _>>()?;
    let cursor = events
        .last()
        .filter(|_| has_more)
        .map(event_cursor)
        .transpose()?;
    Ok(Json(EventList {
        events,
        has_more,
        count: u64::try_from(count)?,
        cursor,
    }))
}

type EventRow = (
    String,
    i64,
    String,
    String,
    String,
    Option<String>,
    OffsetDateTime,
    Value,
);

async fn event_rows(
    conn: &mut PgConnection,
    repo: &RepoId,
    number: i64,
    page: &Page<EventKey>,
) -> Result<Vec<EventRow>, ApiError> {
    let (at, seq, id) = match page.after.clone() {
        Some((at, seq, id)) => (Some(at), Some(seq), Some(id)),
        None => (None, None, None),
    };

    // A decision acts as CIHub: the person who asked for it is the actor of
    // its cause.
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT e.id, e.seq, e.item_id, e.type::text, e.actor_kind::text,
                coalesce(u.login, cu.login), e.occurred_at, e.payload
         FROM events e
         LEFT JOIN users u ON u.id = e.actor_user_id
         LEFT JOIN events c ON c.queue_id = e.queue_id AND c.id = e.cause_id
         LEFT JOIN users cu ON cu.id = c.actor_user_id
         WHERE (e.queue_id, e.item_id) IN ({PR_ITEMS})
           AND ($3::text IS NULL OR (e.occurred_at, e.seq, e.id) > ($3::timestamptz, $4, $5))
         ORDER BY e.occurred_at, e.seq, e.id
         LIMIT $6"
    )))
    .bind(repo.to_string())
    .bind(number)
    .bind(at)
    .bind(seq)
    .bind(id)
    .bind(page.rows())
    .fetch_all(conn)
    .await?)
}

fn event_of(row: EventRow) -> Result<Event, ApiError> {
    let (id, seq, item_id, kind, actor_kind, actor, occurred_at, payload) = row;
    let read = event_kind(&kind, &payload).map_err(|source| MergeQueueError::ReadEvent {
        id: id.clone(),
        source,
    })?;

    Ok(Event {
        description: describe(&read),
        id,
        seq,
        item_id,
        kind,
        actor_kind,
        actor,
        occurred_at,
        payload,
    })
}

fn event_cursor(event: &Event) -> Result<String, ApiError> {
    let occurred_at = event.occurred_at.format(&Rfc3339)?;

    Ok(cursor_of("events", (occurred_at, event.seq, &event.id)))
}

/// The sentence of an event of an item, as the history of the dashboard shows
/// it. Every event is named, so that a new event of an item gets its
/// sentence.
fn describe(kind: &EventKind) -> String {
    match kind {
        EventKind::ItemTransitioned {
            reason, merged_sha, ..
        } => transition_sentence(*reason, merged_sha.as_deref()),
        EventKind::ItemReadinessChanged { block_reasons, .. } => {
            blocks_sentence(block_reasons).unwrap_or_else(|| "nothing blocks the PR".to_owned())
        }
        EventKind::ItemCancelRequested { .. } => "a cancellation was asked".to_owned(),
        EventKind::ItemRestartRequested { .. } => "a new test was asked".to_owned(),
        EventKind::MergeSucceeded { .. } => "the merge job merged the PR".to_owned(),
        // The next transition gives the reason.
        EventKind::MergeFailed { .. } => "the merge job reported an error".to_owned(),
        // These events name no item: `EventKind::subjects`.
        EventKind::ItemSubmitted { .. }
        | EventKind::PrObserved { .. }
        | EventKind::TargetPushed { .. }
        | EventKind::CheckObserved { .. }
        | EventKind::TargetsReceived { .. }
        | EventKind::QueueControlRequested { .. }
        | EventKind::QueueSettingsChanged { .. }
        | EventKind::AccessLost
        | EventKind::AccessRestored
        | EventKind::GroupFormed { .. }
        | EventKind::GroupTransitioned { .. }
        | EventKind::RunTransitioned { .. }
        | EventKind::TestBranchAssembled { .. }
        | EventKind::TestBranchAssemblyFailed { .. }
        | EventKind::QueueRenumbered { .. }
        | EventKind::QueueTransitioned { .. }
        | EventKind::QueueWindowChanged { .. } => kind.event_type().as_str().to_owned(),
    }
}

/// Submit a PR
///
/// The queue of the base branch of the PR takes it at its next
/// reconciliation. A PR already in a queue answers `409` at once; the
/// refusals that need a read of the SCM, such as a queue that accepts
/// nothing, come later, on the PR.
#[utoipa::path(
    post,
    path = "/orgs/{org}/repos/{repo}/prs/{number}/submit",
    tag = "Pull requests",
    params(PrPath),
    request_body = SubmitPr,
    responses((status = 202, description = "The queue takes the request."), SubmitErrors),
    security(("token" = ["prs:write"]))
)]
pub(super) async fn submit(
    grant: ApiGrant,
    State(state): State<ApiState>,
    ApiPath(path): ApiPath<PrPath>,
    ApiJson(request): ApiJson<SubmitPr>,
) -> Result<StatusCode, ApiError> {
    grant.require(ApiScope::PrsWrite)?;
    let priority = request
        .priority
        .as_ref()
        .map(|input| {
            input.priority().ok_or_else(|| {
                ApiError::new(
                    ErrorCode::UnknownPriority,
                    "the name of the priority is not `urgent`, `high`, `medium` or `low`".into(),
                )
            })
        })
        .transpose()?;

    let (mut tx, repo) = begin_repo(&state, &grant, &path.org, &path.repo).await?;
    let current: Option<String> = sqlx::query_scalar(
        "SELECT i.state::text FROM queue_items i JOIN queues q ON q.id = i.queue_id
         WHERE q.repo_id = $1 AND i.pr_number = $2 AND i.is_current
           AND i.state NOT IN ('merged', 'failed', 'cancelled')
         LIMIT 1",
    )
    .bind(repo.to_string())
    .bind(stored(path.number)?)
    .fetch_optional(&mut *tx)
    .await?;

    // The refusal that needs no read of the SCM; the others come later, on
    // the PR (principle V).
    if let Some(item_state) = current {
        return Err(ApiError::new(
            ErrorCode::PrAlreadyQueued,
            format!("the PR is already in a queue, `{item_state}`"),
        ));
    }

    let target = RepoTarget::load(&mut tx, repo).await?;
    let payload = jobs::SubmitPr {
        repo_id: repo,
        pr_number: path.number,
        via: SubmittedVia::Api,
        requested_by: requester(&mut tx, &grant).await?,
        priority,
        no_batch: request.no_batch,
        direct: request.direct,
        profile: request.profile,
        label: None,
        comment_id: None,
    };
    let cause = format!("api-{}", new_event_id());
    let job: Job = jobs::submit(grant.org_id, target.repo.connection.id, &cause, &payload);
    let now = (state.clock)();

    enqueue_job(&mut tx, &job, now, now).await?;
    tx.commit().await?;

    Ok(StatusCode::ACCEPTED)
}

/// The current item of the PR `number` in the repository `repo`, its state
/// and its queue: the last one submitted, when the PR changed its base branch.
async fn current_item(
    conn: &mut PgConnection,
    repo: &RepoId,
    number: u64,
) -> Result<(ItemId, ItemState, QueueId), ApiError> {
    let row: Option<(String, String, String)> = sqlx::query_as(
        "SELECT i.id, i.state::text, i.queue_id FROM queue_items i
         JOIN queues q ON q.id = i.queue_id
         WHERE q.repo_id = $1 AND i.pr_number = $2 AND i.is_current
         ORDER BY i.submitted_at DESC, i.id DESC LIMIT 1",
    )
    .bind(repo.to_string())
    .bind(stored(number)?)
    .fetch_optional(conn)
    .await?;
    let (id, state, queue) = row.ok_or_else(|| pr_not_found(number))?;

    Ok((id.parse()?, state.parse()?, queue.parse()?))
}

/// Gives the queue the request `kind` on the current item of the PR.
async fn give(
    state: &ApiState,
    grant: &ApiGrant,
    path: &PrPath,
    kind: impl FnOnce(ItemId, ItemState) -> Result<EventKind, ApiError>,
) -> Result<StatusCode, ApiError> {
    grant.require(ApiScope::PrsWrite)?;

    let (mut tx, repo) = begin_repo(state, grant, &path.org, &path.repo).await?;
    let (item_id, item_state, queue_id) = current_item(&mut tx, &repo, path.number).await?;
    let fact = kind(item_id, item_state)?;
    // The audit names the request as the log does.
    let action = fact.event_type().as_str();

    give_back(
        &mut tx,
        grant.org_id,
        queue_id,
        vec![(actor(grant), fact)],
        (state.clock)(),
    )
    .await?;

    let after = json!({ "queue_id": queue_id.to_string(), "pr_number": path.number });
    audit(&mut tx, grant, action, &item_id.to_string(), after).await?;
    tx.commit().await?;

    Ok(StatusCode::ACCEPTED)
}

/// Cancel a PR
///
/// Takes the PR out of its queue: its current submission ends `cancelled`
/// (`admin_requested`), and stays readable.
#[utoipa::path(
    post,
    path = "/orgs/{org}/repos/{repo}/prs/{number}/cancel",
    tag = "Pull requests",
    params(PrPath),
    responses((status = 202, description = "The queue takes the request."), CancelErrors),
    security(("token" = ["prs:write"]))
)]
pub(super) async fn cancel(
    grant: ApiGrant,
    State(state): State<ApiState>,
    ApiPath(path): ApiPath<PrPath>,
) -> Result<StatusCode, ApiError> {
    give(&state, &grant, &path, |item_id, item_state| {
        if item_state.is_terminal() {
            return Err(ApiError::new(
                ErrorCode::ItemEnded,
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

/// Restart the tests of a PR
///
/// Tests again the current submission of the PR, in `testing` or
/// `pending_failure` (`tests_restarted_by_api_request`).
#[utoipa::path(
    post,
    path = "/orgs/{org}/repos/{repo}/prs/{number}/restart",
    tag = "Pull requests",
    params(PrPath),
    responses((status = 202, description = "The queue takes the request."), RestartErrors),
    security(("token" = ["prs:write"]))
)]
pub(super) async fn restart(
    grant: ApiGrant,
    State(state): State<ApiState>,
    ApiPath(path): ApiPath<PrPath>,
) -> Result<StatusCode, ApiError> {
    give(&state, &grant, &path, |item_id, item_state| {
        if !matches!(item_state, ItemState::Testing | ItemState::PendingFailure) {
            return Err(ApiError::new(
                ErrorCode::ItemNotInTest,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_estimate_counts_a_run_for_each_window_up_to_the_pr() {
        let pace = QueuePace {
            window: 2,
            median_run_seconds: Some(600),
        };
        let without_run = QueuePace {
            window: 2,
            median_run_seconds: None,
        };

        assert_eq!(merges_in(ItemState::Pending, 1, &pace, None), Some(600));
        assert_eq!(merges_in(ItemState::Pending, 3, &pace, None), Some(1200));
        assert_eq!(
            merges_in(ItemState::Testing, 1, &pace, Some(240)),
            Some(360)
        );
        assert_eq!(merges_in(ItemState::Testing, 1, &pace, Some(900)), Some(0));
        assert_eq!(merges_in(ItemState::TestsPassed, 1, &pace, None), Some(0));
        assert_eq!(merges_in(ItemState::NotReady, 1, &pace, None), None);
        assert_eq!(merges_in(ItemState::Pending, 1, &without_run, None), None);
    }
}

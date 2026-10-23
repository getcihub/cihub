use std::collections::BTreeMap;

use axum::Json;
use axum::extract::State;
use cihub_api_types::{Dependent, ErrorCode, Run, RunCheck};
use cihub_domain::{ApiScope, RepoId, RunId};
use cihub_platform::{ApiError, ApiGrant, ApiPath};
use cihub_scm::{ConnectionRef, RepoRef};
use serde_json::Value;
use sqlx::PgConnection;
use time::OffsetDateTime;

use super::{ApiState, RunErrors, RunPath, begin_repo};

fn run_not_found(run_id: &str) -> ApiError {
    ApiError::new(ErrorCode::RunNotFound, format!("run `{run_id}` not found"))
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

/// Get a run
///
/// The run, its test branch, the PRs that it tests and their predecessors,
/// every check observed on it, and the live runs that depend on it. A job of
/// the CI reads its run identifier in the last segment of its branch; a run of
/// another repository answers `run_not_found`.
#[utoipa::path(
    get,
    path = "/orgs/{org}/repos/{repo}/runs/{run_id}",
    tag = "Runs",
    params(RunPath),
    responses((status = 200, description = "The run.", body = Run), RunErrors),
    security(("token" = ["queues:read"]))
)]
pub(super) async fn get_run(
    grant: ApiGrant,
    State(state): State<ApiState>,
    ApiPath(path): ApiPath<RunPath>,
) -> Result<Json<Run>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;

    let RunPath { org, repo, run_id } = path;
    let _: RunId = run_id.parse().map_err(|_| run_not_found(&run_id))?;
    let (mut tx, repo) = begin_repo(&state, &grant, &org, &repo).await?;
    let run: RunRow = sqlx::query_as(
        "SELECT id, queue_id, group_id, attempt, status::text, status_reason::text, test_branch,
                test_sha, base_sha, draft_pr_number, tested_pr_numbers, head_shas,
                required_statuses, required_statuses_source::text, started_at, ended_at,
                (SELECT max(c.observed_at) FROM check_observations c
                 WHERE c.run_id = test_runs.id) AS last_report_at,
                CASE WHEN status IN ('failed', 'timed_out') THEN status_reason::text END
                    AS failure_reason,
                failure_causing_statuses, conflict_culprit
         FROM test_runs
         WHERE id = $1 AND queue_id IN (SELECT id FROM queues WHERE repo_id = $2)",
    )
    .bind(&run_id)
    .bind(repo.to_string())
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| run_not_found(&run_id))?;
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
    let predecessor_pr_numbers = pr_numbers(
        predecessors
            .iter()
            .filter_map(|id| numbers.get(id).copied()),
    )?;
    let draft_pr_url = match run.draft_pr_number {
        Some(number) => Some(draft_pr_url(&state, &mut tx, repo, u64::try_from(number)?).await?),
        None => None,
    };
    let checks = run_checks(&mut tx, &run_id).await?;
    let dependents = dependents(&mut tx, &run.queue_id, &run_id, &members).await?;

    Ok(Json(Run {
        attempt: u32::try_from(run.attempt)?,
        status: run.status.parse()?,
        status_reason: run.status_reason.parse()?,
        draft_pr_number: run.draft_pr_number.map(u64::try_from).transpose()?,
        draft_pr_url,
        tested_pr_numbers: pr_numbers(run.tested_pr_numbers)?,
        required_statuses_source: run
            .required_statuses_source
            .as_deref()
            .map(str::parse)
            .transpose()?,
        failure_reason: run.failure_reason.as_deref().map(str::parse).transpose()?,
        conflict_culprit: run
            .conflict_culprit
            .map(serde_json::from_value)
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

/// The page on the SCM of the draft PR `number` of `repo`.
async fn draft_pr_url(
    state: &ApiState,
    conn: &mut PgConnection,
    repo: RepoId,
    number: u64,
) -> Result<String, ApiError> {
    let (connection_id, connection_external_id, external_id, namespace, name): (
        String,
        String,
        String,
        String,
        String,
    ) = sqlx::query_as(
        "SELECT c.id, c.external_id, r.external_id, r.namespace, r.name
         FROM repos r JOIN scm_connections c ON c.id = r.connection_id
         WHERE r.id = $1",
    )
    .bind(repo.to_string())
    .fetch_one(conn)
    .await?;
    let repo = RepoRef {
        connection: ConnectionRef {
            id: connection_id.parse()?,
            external_id: connection_external_id,
        },
        external_id,
        namespace,
        name,
    };

    Ok(state.scm.pr_url(&repo, number))
}

async fn run_checks(conn: &mut PgConnection, run_id: &str) -> Result<Vec<RunCheck>, ApiError> {
    type Row = (
        String,
        String,
        bool,
        String,
        Option<String>,
        Option<String>,
        Option<OffsetDateTime>,
        Option<OffsetDateTime>,
        OffsetDateTime,
    );
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT name, kind::text, required, status::text, conclusion::text, details_url,
                started_at, completed_at, observed_at
         FROM check_observations WHERE run_id = $1 ORDER BY name, kind",
    )
    .bind(run_id)
    .fetch_all(conn)
    .await?;
    rows.into_iter()
        .map(
            |(
                name,
                kind,
                required,
                status,
                conclusion,
                details_url,
                started_at,
                completed_at,
                observed_at,
            )| {
                Ok(RunCheck {
                    name,
                    kind: kind.parse()?,
                    required,
                    status: status.parse()?,
                    conclusion: conclusion.as_deref().map(str::parse).transpose()?,
                    details_url,
                    started_at,
                    completed_at,
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
                pr_numbers: pr_numbers(numbers)?,
            })
        })
        .collect()
}

/// The PR numbers that Postgres keeps, as the API gives them.
fn pr_numbers(numbers: impl IntoIterator<Item = i64>) -> Result<Vec<u64>, ApiError> {
    numbers
        .into_iter()
        .map(|number| Ok(u64::try_from(number)?))
        .collect()
}

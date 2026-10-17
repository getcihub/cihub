use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use cihub_api_types::{
    ErrorCode, Permission, Repo, RepoDetail, RepoList, RepoSetup, SetupState, SetupSummary,
};
use cihub_domain::ApiScope;
use cihub_platform::{ApiError, ApiGrant, ApiPath, ApiQuery, Page, cursor_of};
use serde::de::{Error as _, IntoDeserializer};
use serde::{Deserialize, Deserializer};

use super::{ApiState, RepoErrors, RepoListErrors, RepoPath, name_part};

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct RepoQuery {
    /// Only the repositories whose setup is in these states, separated by
    /// commas: `started`, `checking`, `ready`, `ready_with_warnings`,
    /// `cannot_merge`, `invalid_config`.
    #[serde(default, deserialize_with = "setup_states")]
    #[param(value_type = Option<String>, example = "checking,ready,cannot_merge")]
    state: Vec<SetupState>,
    /// Only the repositories whose name holds this text, whatever its case;
    /// from 1 to 100 characters.
    #[param(example = "web")]
    q: Option<String>,
    /// The `cursor` of the previous page.
    from: Option<String>,
    /// How many repositories, from 1 to 100; 50 by default.
    limit: Option<u32>,
}

fn setup_states<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<SetupState>, D::Error> {
    let text = String::deserialize(deserializer)?;

    text.split(',')
        .map(|name| {
            let state: Result<SetupState, serde::de::value::Error> =
                SetupState::deserialize(name.into_deserializer());
            state.map_err(|_| D::Error::custom(format!("unknown state `{name}`")))
        })
        .collect()
}

/// A repository, and the target branch of the queue of its default branch.
type RepoRow = (String, String, String, String, Option<String>);

/// Until the setup of a repository is read, its verdict says only whether its
/// default branch has its queue.
const REPO_SOURCE: &str = "repos r
     LEFT JOIN queues q ON q.repo_id = r.id AND q.target_branch = r.default_branch";

/// List the repositories
///
/// The repositories of the organization that the App reaches, by namespace
/// then name, each with what the caller may do on it and the verdict of its
/// setup. Reads Postgres only.
#[utoipa::path(
    get,
    path = "/orgs/{org}/repos",
    tag = "Repositories",
    params(
        ("org" = String, Path, description = "The slug of the organization.", example = "acme"),
        RepoQuery
    ),
    responses(
        (status = 200, description = "A page of repositories.", body = RepoList),
        RepoListErrors
    ),
    security(("token" = ["queues:read"]))
)]
pub(super) async fn list_repos(
    grant: ApiGrant,
    State(state): State<ApiState>,
    ApiPath(org): ApiPath<String>,
    ApiQuery(query): ApiQuery<RepoQuery>,
) -> Result<Json<RepoList>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;
    let page: Page<(String, String, String)> =
        Page::read("repos", query.from.as_deref(), query.limit)?;
    let name = name_part(query.q)?;
    let wants = |wanted: SetupState| query.state.is_empty() || query.state.contains(&wanted);
    let (started, checking) = (wants(SetupState::Started), wants(SetupState::Checking));

    let filter = "r.removed_at IS NULL
         AND ($1::text IS NULL OR strpos(lower(r.name), lower($1)) > 0)
         AND (($2 AND q.id IS NOT NULL) OR ($3 AND q.id IS NULL))";
    let mut tx = grant.begin_in_org(&state.pool, &org).await?;
    let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT count(*) FROM {REPO_SOURCE} WHERE {filter}"
    )))
    .bind(&name)
    .bind(started)
    .bind(checking)
    .fetch_one(&mut *tx)
    .await?;
    let (namespace, after_name, id) = match page.after.clone() {
        Some((namespace, name, id)) => (Some(namespace), Some(name), Some(id)),
        None => (None, None, None),
    };
    // A repository deleted then created again keeps its row: the identifier
    // tells two repositories of the same name apart.
    let rows: Vec<RepoRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT r.id, r.namespace, r.name, r.default_branch, q.target_branch
         FROM {REPO_SOURCE}
         WHERE {filter}
           AND ($4::text IS NULL OR (r.namespace, r.name, r.id) > ($4, $5, $6))
         ORDER BY r.namespace, r.name, r.id LIMIT $7"
    )))
    .bind(&name)
    .bind(started)
    .bind(checking)
    .bind(namespace)
    .bind(after_name)
    .bind(id)
    .bind(page.rows())
    .fetch_all(&mut *tx)
    .await?;

    let permission = permission_of(&grant);
    let repos = rows
        .into_iter()
        .map(|row| repo_of(row, permission))
        .collect();
    let (repos, has_more): (Vec<Repo>, bool) = page.split(repos);
    let cursor = repos
        .last()
        .filter(|_| has_more)
        .map(|repo| cursor_of("repos", (&repo.namespace, &repo.name, &repo.id)));
    Ok(Json(RepoList {
        repos,
        has_more,
        count: u64::try_from(count)?,
        cursor,
    }))
}

/// Get a repository
///
/// The repository, whatever the case of its name, with what the caller may
/// do on it and its whole setup. Reads Postgres only.
#[utoipa::path(
    get,
    path = "/orgs/{org}/repos/{repo}",
    tag = "Repositories",
    params(RepoPath),
    responses((status = 200, description = "The repository.", body = RepoDetail), RepoErrors),
    security(("token" = ["queues:read"]))
)]
pub(super) async fn get_repo(
    grant: ApiGrant,
    State(state): State<ApiState>,
    ApiPath(path): ApiPath<RepoPath>,
) -> Result<Json<RepoDetail>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;

    let mut tx = grant.begin_in_org(&state.pool, &path.org).await?;
    // The SCM does not tell names apart by their case.
    let row: Option<RepoRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT r.id, r.namespace, r.name, r.default_branch, q.target_branch
         FROM {REPO_SOURCE}
         WHERE lower(r.name) = lower($1) ORDER BY r.id LIMIT 1"
    )))
    .bind(&path.repo)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((id, namespace, name, default_branch, queue)) = row else {
        return Err(ApiError::new(
            ErrorCode::RepoNotFound,
            format!("repository `{}` not found", path.repo),
        ));
    };

    Ok(Json(RepoDetail {
        id,
        namespace,
        name,
        default_branch,
        permission: permission_of(&grant),
        setup: RepoSetup {
            state: state_of(queue.as_ref()),
            queue,
            rules: Vec::new(),
            target_branch_missing: false,
            rules_url: None,
            config: None,
            checked_at: None,
            requested_at: None,
        },
    }))
}

/// Sync the setup of a repository
///
/// Asks CIHub to read the setup of the repository from the SCM now, at most
/// once a minute; its `requested_at` shows the read until it ends.
#[utoipa::path(
    post,
    path = "/orgs/{org}/repos/{repo}/sync",
    tag = "Repositories",
    params(RepoPath),
    responses((status = 202, description = "CIHub takes the request."), RepoErrors),
    security(("token" = ["queues:write"]))
)]
pub(super) async fn sync_repo(
    grant: ApiGrant,
    State(_state): State<ApiState>,
    ApiPath(_path): ApiPath<RepoPath>,
) -> Result<StatusCode, ApiError> {
    grant.require(ApiScope::QueuesWrite)?;

    Err(ApiError::new(
        ErrorCode::RouteNotFound,
        "the sync of a setup is not served yet".to_owned(),
    ))
}

/// What the scopes of the caller let it do on a repository of the
/// organization.
fn permission_of(grant: &ApiGrant) -> Permission {
    Permission {
        read: grant.scopes.contains(&ApiScope::QueuesRead),
        write: grant.scopes.contains(&ApiScope::PrsWrite),
        admin: grant.scopes.contains(&ApiScope::QueuesWrite),
    }
}

fn state_of(queue: Option<&String>) -> SetupState {
    if queue.is_some() {
        SetupState::Started
    } else {
        SetupState::Checking
    }
}

fn repo_of((id, namespace, name, default_branch, queue): RepoRow, permission: Permission) -> Repo {
    Repo {
        id,
        namespace,
        name,
        default_branch,
        permission,
        setup: SetupSummary {
            state: state_of(queue.as_ref()),
            checked_at: None,
            requested_at: None,
        },
    }
}

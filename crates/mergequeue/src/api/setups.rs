use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use cihub_api_types::{ErrorCode, RepoSetup, SetupList, SetupState};
use cihub_domain::ApiScope;
use cihub_platform::{ApiError, ApiGrant, ApiPath, ApiQuery};
use serde::de::{Error as _, IntoDeserializer};
use serde::{Deserialize, Deserializer};

use super::{ApiState, RepoPath, RepoSetupErrors, SetupListErrors};

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
#[expect(dead_code, reason = "the list of the setups has no body yet")]
pub(super) struct SetupQuery {
    /// Only the setups in these states, separated by commas: `started`,
    /// `checking`, `ready`, `ready_with_warnings`, `cannot_merge`,
    /// `invalid_config`.
    #[serde(default, deserialize_with = "setup_states")]
    #[param(value_type = Option<String>, example = "checking,ready,cannot_merge")]
    state: Vec<SetupState>,
    /// Only the repositories whose name holds this text, whatever its case;
    /// from 1 to 100 characters.
    #[param(example = "web")]
    q: Option<String>,
    /// The `cursor` of the previous page.
    from: Option<String>,
    /// How many setups, from 1 to 100; 50 by default.
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

/// The answer of a route whose body is not written yet.
fn not_written() -> ApiError {
    ApiError::new(
        ErrorCode::RouteNotFound,
        "the setups are not served yet".to_owned(),
    )
}

/// List the setups of the repositories
///
/// The setup of each repository that the App reaches, by name: what CIHub
/// found about the rules of its default branch and its configuration file,
/// with a verdict. A repository whose default branch has its queue is
/// `started`. Reads Postgres only.
#[utoipa::path(
    get,
    path = "/orgs/{org}/setups",
    tag = "Setups",
    params(
        ("org" = String, Path, description = "The slug of the organization.", example = "acme"),
        SetupQuery
    ),
    responses((status = 200, description = "A page of setups.", body = SetupList), SetupListErrors),
    security(("token" = ["queues:read"]))
)]
pub(super) async fn list_setups(
    grant: ApiGrant,
    State(_state): State<ApiState>,
    ApiPath(_org): ApiPath<String>,
    ApiQuery(_query): ApiQuery<SetupQuery>,
) -> Result<Json<SetupList>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;

    Err(not_written())
}

/// Get the setup of a repository
///
/// What CIHub found about the rules of the default branch of the repository
/// and its configuration file, with a verdict. Reads Postgres only.
#[utoipa::path(
    get,
    path = "/orgs/{org}/repos/{repo}/setup",
    tag = "Setups",
    params(RepoPath),
    responses((status = 200, description = "The setup.", body = RepoSetup), RepoSetupErrors),
    security(("token" = ["queues:read"]))
)]
pub(super) async fn get_setup(
    grant: ApiGrant,
    State(_state): State<ApiState>,
    ApiPath(_path): ApiPath<RepoPath>,
) -> Result<Json<RepoSetup>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;

    Err(not_written())
}

/// Read the setup of a repository again
///
/// Asks CIHub to read the setup of the repository from the SCM now, at most
/// once a minute; its `requested_at` shows the read until it ends.
#[utoipa::path(
    post,
    path = "/orgs/{org}/repos/{repo}/resync",
    tag = "Setups",
    params(RepoPath),
    responses((status = 202, description = "CIHub takes the request."), RepoSetupErrors),
    security(("token" = ["queues:write"]))
)]
pub(super) async fn resync_repo(
    grant: ApiGrant,
    State(_state): State<ApiState>,
    ApiPath(_path): ApiPath<RepoPath>,
) -> Result<StatusCode, ApiError> {
    grant.require(ApiScope::QueuesWrite)?;

    Err(not_written())
}

/// Read the setups of the organization again
///
/// Asks CIHub to read from the SCM now the setups of at most 50 repositories
/// without a queue, the oldest read first.
#[utoipa::path(
    post,
    path = "/orgs/{org}/setups/resync",
    tag = "Setups",
    params(("org" = String, Path, description = "The slug of the organization.", example = "acme")),
    responses((status = 202, description = "CIHub takes the request.")),
    security(("token" = ["queues:write"]))
)]
pub(super) async fn resync_setups(
    grant: ApiGrant,
    State(_state): State<ApiState>,
    ApiPath(_org): ApiPath<String>,
) -> Result<StatusCode, ApiError> {
    grant.require(ApiScope::QueuesWrite)?;

    Err(not_written())
}

//! `/api/v1` (`docs/API.md`, `DECISIONS.md` D159): the authentication of each
//! request by its API token, the rate limit of each token, the routes of the
//! platform (`/repos`, `/orgs/{org}/audit`), the routes of the modules, and
//! the `OpenAPI` document of all of them.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use axum::extract::{DefaultBodyLimit, Request, State};
use axum::http::header::AUTHORIZATION;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use cihub_api_types::{AuditEntry, AuditPage, ErrorCode, MAX_TAKE, Problem, Repo, RepoList};
use cihub_domain::{ApiScope, ApiTokenId};
use cihub_platform::{ApiError, ApiGrant, ApiPath, ApiQuery, ModuleApi};
use cihub_store::{PgPool, authenticate, begin_org};
use secrecy::SecretString;
use serde::Deserialize;
use serde_json::Value;
use time::OffsetDateTime;
use utoipa::openapi::security::{Http, HttpAuthScheme, SecurityScheme};
use utoipa::{Modify, OpenApi};

pub const REQUESTS_PER_MINUTE: u32 = 600;

const MAX_BODY_BYTES: usize = 25 * 1024 * 1024;

#[derive(Clone)]
struct ApiState {
    pool: PgPool,
    now: fn() -> OffsetDateTime,
    windows: Arc<Mutex<HashMap<ApiTokenId, (i64, u32)>>>,
}

struct Security;

impl Modify for Security {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "token",
            SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
        );
    }
}

#[derive(OpenApi)]
#[openapi(
    info(title = "CIHub API", version = "1"),
    servers((url = "/api/v1")),
    paths(list_repos, list_audit),
    components(schemas(AuditEntry, AuditPage, ErrorCode, Problem, Repo, RepoList)),
    modifiers(&Security)
)]
struct PlatformApi;

pub fn openapi(modules: &[ModuleApi]) -> utoipa::openapi::OpenApi {
    let mut document = PlatformApi::openapi();
    for module in modules {
        document.merge(module.openapi.clone());
    }
    document
}

pub(crate) fn router(pool: PgPool, now: fn() -> OffsetDateTime, modules: Vec<ModuleApi>) -> Router {
    let document = openapi(&modules);
    let state = ApiState {
        pool,
        now,
        windows: Arc::default(),
    };
    let mut routes = Router::new()
        .route("/repos", get(list_repos))
        .route("/orgs/{org}/audit", get(list_audit))
        .with_state(state.clone());

    for module in modules {
        routes = routes.merge(module.routes);
    }

    let routes = routes
        .fallback(|| async { ApiError::not_found("route") })
        .layer(middleware::from_fn_with_state(state, authenticated));

    Router::new()
        .route(
            "/openapi.json",
            get(move || {
                let document = document.clone();
                async move { Json(document) }
            }),
        )
        .merge(routes)
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
}

async fn authenticated(
    State(state): State<ApiState>,
    mut request: Request,
    next: Next,
) -> Response {
    let secret = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(|token| SecretString::from(token.trim().to_owned()));
    let unauthorized = || ApiError::new(ErrorCode::Unauthorized, "no valid API token".into());
    let Some(secret) = secret else {
        return unauthorized().into_response();
    };

    let now = (state.now)();
    let grant = match authenticate(&state.pool, &secret, now).await {
        Ok(Some(grant)) => grant,
        Ok(None) => return unauthorized().into_response(),
        Err(err) => return ApiError::from(err).into_response(),
    };

    if let Some(wait) = over_limit(&state, grant.id, now) {
        let mut limited = ApiError::new(
            ErrorCode::RateLimited,
            format!("at most {REQUESTS_PER_MINUTE} requests per minute and per token"),
        );
        limited.retry_after = Some(wait);
        return limited.into_response();
    }

    request.extensions_mut().insert(ApiGrant {
        org_id: grant.org_id,
        token_id: grant.id,
        scopes: grant.scopes,
    });

    next.run(request).await
}

fn over_limit(state: &ApiState, token: ApiTokenId, now: OffsetDateTime) -> Option<u64> {
    let minute = now.unix_timestamp().div_euclid(60);
    let mut windows = state.windows.lock().unwrap_or_else(PoisonError::into_inner);

    windows.retain(|_, (window, _)| *window == minute);
    let (_, count) = windows.entry(token).or_insert((minute, 0));
    if *count >= REQUESTS_PER_MINUTE {
        let elapsed = now.unix_timestamp().rem_euclid(60);
        return Some(u64::try_from(60 - elapsed).unwrap_or(60));
    }

    *count += 1;
    None
}

/// The repositories of the organization of the token.
#[utoipa::path(
    get,
    path = "/repos",
    responses((status = 200, body = RepoList)),
    security(("token" = ["queues:read"]))
)]
async fn list_repos(
    grant: ApiGrant,
    State(state): State<ApiState>,
) -> Result<Json<RepoList>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;
    let mut tx = begin_org(&state.pool, grant.org_id).await?;
    let rows: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT id, namespace, name, default_branch FROM repos ORDER BY namespace, name",
    )
    .fetch_all(&mut *tx)
    .await?;
    let repos = rows
        .into_iter()
        .map(|(id, namespace, name, default_branch)| Repo {
            id,
            namespace,
            name,
            default_branch,
        })
        .collect();

    Ok(Json(RepoList { repos }))
}

#[derive(Deserialize, utoipa::IntoParams)]
struct AuditQuery {
    /// The `next_cursor` of the previous page.
    cursor: Option<String>,
    /// At most 100; 50 by default.
    take: Option<u32>,
}

type AuditRow = (
    String,
    String,
    String,
    Option<String>,
    Option<Value>,
    Option<Value>,
    OffsetDateTime,
);

/// The audit log of the organization, newest first.
#[utoipa::path(
    get,
    path = "/orgs/{org}/audit",
    params(("org" = String, Path, description = "The slug of the organization"), AuditQuery),
    responses((status = 200, body = AuditPage)),
    security(("token" = ["queues:write"]))
)]
async fn list_audit(
    grant: ApiGrant,
    State(state): State<ApiState>,
    ApiPath(org): ApiPath<String>,
    ApiQuery(query): ApiQuery<AuditQuery>,
) -> Result<Json<AuditPage>, ApiError> {
    grant.require(ApiScope::QueuesWrite)?;
    let take = query.take.unwrap_or(50);
    if take == 0 || take > MAX_TAKE {
        return Err(ApiError::invalid(format!("take is from 1 to {MAX_TAKE}")));
    }

    let mut tx = begin_org(&state.pool, grant.org_id).await?;
    let known: Option<String> = sqlx::query_scalar("SELECT id FROM orgs WHERE slug = $1")
        .bind(&org)
        .fetch_optional(&mut *tx)
        .await?;

    if known.is_none() {
        return Err(ApiError::not_found("organization"));
    }

    let rows: Vec<AuditRow> = sqlx::query_as(
        "SELECT id, action, target, actor_user_id, before, after, occurred_at FROM audit_log
         WHERE $1::text IS NULL OR (occurred_at, id) <
               (SELECT occurred_at, id FROM audit_log WHERE id = $1)
         ORDER BY occurred_at DESC, id DESC
         LIMIT $2",
    )
    .bind(&query.cursor)
    .bind(i64::from(take) + 1)
    .fetch_all(&mut *tx)
    .await?;
    let mut entries: Vec<AuditEntry> = rows
        .into_iter()
        .map(
            |(id, action, target, actor_user_id, before, after, occurred_at)| AuditEntry {
                id,
                action,
                target,
                actor_user_id,
                before,
                after,
                occurred_at,
            },
        )
        .collect();
    let next_cursor = if entries.len() > take as usize {
        entries.truncate(take as usize);
        entries.last().map(|entry| entry.id.clone())
    } else {
        None
    };

    Ok(Json(AuditPage {
        entries,
        next_cursor,
    }))
}

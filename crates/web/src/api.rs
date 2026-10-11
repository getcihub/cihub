//! `/api/v1` (`specs/API.md`): the authentication of each
//! request by its API token or by a session of the dashboard, the rate limit
//! of each of them, the routes of the platform (`/me`, `/orgs/{org}/repos`,
//! `/orgs/{org}/audit`), the routes of the modules, and the `OpenAPI` document
//! of all of them.

use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, Mutex, PoisonError};

use axum::extract::{DefaultBodyLimit, Request, State};
use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, Method};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Extension, Json, Router};
use cihub_api_types::{
    AuditEntry, AuditPage, ErrorCode, MAX_TAKE, Me, OrgRole, Problem, QueueChanged, Repo, RepoList,
};
use cihub_domain::{ApiScope, ApiTokenId, JobClass, SessionId};
use cihub_platform::{ApiError, ApiGrant, ApiPath, ApiQuery, GrantActor, ModuleApi, QueueNewsFeed};
use cihub_scm::Scm;
use cihub_store::{
    PgPool, SessionGrant, authenticate, authenticate_session, login_of, memberships_of,
};
use secrecy::SecretString;
use serde::Deserialize;
use serde_json::Value;
use time::OffsetDateTime;
use tracing::warn;
use utoipa::openapi::security::{
    ApiKey, ApiKeyValue, Http, HttpAuthScheme, SecurityRequirement, SecurityScheme,
};
use utoipa::{Modify, OpenApi};

use crate::session::{SESSION_COOKIE, cookie, csrf_refused, has_csrf_token, needs_csrf_token};
use crate::stream::{__path_stream, Streams, stream};

pub const REQUESTS_PER_MINUTE: u32 = 600;

const MAX_BODY_BYTES: usize = 25 * 1024 * 1024;

#[derive(Clone)]
struct ApiState {
    pool: PgPool,
    now: fn() -> OffsetDateTime,
    scm: Arc<dyn Scm>,
    windows: Arc<Mutex<HashMap<Caller, (i64, u32)>>>,
}

/// Who calls, for the rate limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Caller {
    Token(ApiTokenId),
    Session(SessionId),
}

struct Security;

impl Modify for Security {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "token",
            SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
        );
        components.add_security_scheme(
            "session",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new(SESSION_COOKIE))),
        );
    }
}

#[derive(OpenApi)]
#[openapi(
    info(title = "CIHub API", version = "1"),
    servers((url = "/api/v1")),
    paths(me, list_repos, list_audit, stream),
    components(schemas(
        AuditEntry,
        AuditPage,
        ErrorCode,
        Me,
        OrgRole,
        Problem,
        QueueChanged,
        Repo,
        RepoList
    )),
    modifiers(&Security)
)]
struct PlatformApi;

/// The document of the platform and of the parts that the modules give.
pub fn openapi(modules: &[utoipa::openapi::OpenApi]) -> utoipa::openapi::OpenApi {
    let mut document = PlatformApi::openapi();
    for module in modules {
        document.merge(module.clone());
    }

    accept_sessions(&mut document);
    document
}

/// Every route that a token reaches also answers a session, whose role gives
/// the scopes: the client of the dashboard is generated from this document.
fn accept_sessions(document: &mut utoipa::openapi::OpenApi) {
    let session = SecurityRequirement::new("session", Vec::<String>::new());
    for item in document.paths.paths.values_mut() {
        let operations = [
            &mut item.get,
            &mut item.put,
            &mut item.post,
            &mut item.delete,
            &mut item.patch,
        ];
        for security in operations
            .into_iter()
            .flatten()
            .filter_map(|op| op.security.as_mut())
        {
            if !security.contains(&session) {
                security.push(session.clone());
            }
        }
    }
}

pub(crate) fn router(
    pool: PgPool,
    now: fn() -> OffsetDateTime,
    modules: Vec<ModuleApi>,
    scm: Arc<dyn Scm>,
    news: QueueNewsFeed,
) -> Router {
    let parts: Vec<_> = modules
        .iter()
        .map(|module| module.openapi.clone())
        .collect();
    let document = openapi(&parts);
    let streams = Streams {
        pool: pool.clone(),
        news,
    };
    let state = ApiState {
        pool,
        now,
        scm,
        windows: Arc::default(),
    };
    let mut routes = Router::new()
        .route("/me", get(me))
        .route("/orgs/{org}/repos", get(list_repos))
        .route("/orgs/{org}/audit", get(list_audit))
        .with_state(state.clone())
        .merge(
            Router::new()
                .route("/orgs/{org}/stream", get(stream))
                .with_state(streams),
        );

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
    let now = (state.now)();
    let bearer = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(|token| SecretString::from(token.trim().to_owned()));
    let authenticated = if let Some(secret) = bearer {
        by_token(&state, &secret, now).await
    } else {
        let (headers, method, path) = (request.headers(), request.method(), request.uri().path());
        by_session(&state, headers, method, path, now).await
    };
    let authenticated = match authenticated {
        Ok(authenticated) => authenticated,
        Err(err) => return err.into_response(),
    };

    if let Some(wait) = over_limit(&state, authenticated.caller(), now) {
        let mut limited = ApiError::new(
            ErrorCode::RateLimited,
            format!("at most {REQUESTS_PER_MINUTE} requests per minute and per token or session"),
        );
        limited.retry_after = Some(wait);
        return limited.into_response();
    }

    let extensions = request.extensions_mut();
    match authenticated {
        Authenticated::Token(_, grant) => {
            extensions.insert(grant);
        }
        Authenticated::Session(session, grant) => {
            extensions.insert(session);
            if let Some(grant) = grant {
                extensions.insert(grant);
            }
        }
    }

    next.run(request).await
}

/// A token always has a grant in its organization. A session has one only
/// on a route of an organization where its user has a role.
enum Authenticated {
    Token(ApiTokenId, ApiGrant),
    Session(SessionGrant, Option<ApiGrant>),
}

impl Authenticated {
    fn caller(&self) -> Caller {
        match self {
            Self::Token(id, _) => Caller::Token(*id),
            Self::Session(session, _) => Caller::Session(session.id),
        }
    }
}

fn unauthorized() -> ApiError {
    ApiError::new(
        ErrorCode::Unauthorized,
        "no valid API token or session".into(),
    )
}

async fn by_token(
    state: &ApiState,
    secret: &SecretString,
    now: OffsetDateTime,
) -> Result<Authenticated, ApiError> {
    let grant = authenticate(&state.pool, secret, now)
        .await?
        .ok_or_else(unauthorized)?;

    Ok(Authenticated::Token(
        grant.id,
        ApiGrant {
            org_id: grant.org_id,
            actor: GrantActor::Token(grant.id),
            scopes: grant.scopes,
        },
    ))
}

/// A session, and on a route of an organization the grant of the role of
/// its user there. An organization where the user has no role answers
/// `404`, as for a token of another organization.
async fn by_session(
    state: &ApiState,
    headers: &HeaderMap,
    method: &Method,
    path: &str,
    now: OffsetDateTime,
) -> Result<Authenticated, ApiError> {
    let secret = cookie(headers, SESSION_COOKIE).ok_or_else(unauthorized)?;
    let session = authenticate_session(&state.pool, &secret, now)
        .await?
        .ok_or_else(unauthorized)?;

    if needs_csrf_token(method) && !has_csrf_token(headers, &secret) {
        return Err(csrf_refused());
    }

    let Some(slug) = org_of_path(path) else {
        return Ok(Authenticated::Session(session, None));
    };

    let memberships = memberships_of(&state.pool, session.user_id).await?;
    let membership = memberships
        .into_iter()
        .find(|membership| membership.org_slug.eq_ignore_ascii_case(slug))
        .ok_or_else(|| ApiError::not_found("organization"))?;
    let grant = ApiGrant {
        org_id: membership.org_id,
        actor: GrantActor::User(session.user_id),
        scopes: membership.role.scopes().to_vec(),
    };

    Ok(Authenticated::Session(session, Some(grant)))
}

/// The slug of the organization that the path names, as `/orgs/{org}/…`.
fn org_of_path(path: &str) -> Option<&str> {
    path.strip_prefix("/orgs/")?
        .split('/')
        .next()
        .filter(|slug| !slug.is_empty())
}

fn over_limit(state: &ApiState, caller: Caller, now: OffsetDateTime) -> Option<u64> {
    let minute = now.unix_timestamp().div_euclid(60);
    let mut windows = state.windows.lock().unwrap_or_else(PoisonError::into_inner);

    windows.retain(|_, (window, _)| *window == minute);
    let (_, count) = windows.entry(caller).or_insert((minute, 0));
    if *count >= REQUESTS_PER_MINUTE {
        let elapsed = now.unix_timestamp().rem_euclid(60);
        return Some(u64::try_from(60 - elapsed).unwrap_or(60));
    }

    *count += 1;
    None
}

/// The user of the session and the organizations that they reach.
#[utoipa::path(
    get,
    path = "/me",
    responses((status = 200, body = Me)),
    security(("session" = []))
)]
async fn me(
    State(state): State<ApiState>,
    session: Option<Extension<SessionGrant>>,
) -> Result<Json<Me>, ApiError> {
    let Some(Extension(session)) = session else {
        return Err(ApiError::new(
            ErrorCode::Unauthorized,
            "`/me` needs a session of the dashboard".into(),
        ));
    };

    let login = login_of(&state.pool, session.user_id)
        .await?
        .ok_or_else(unauthorized)?;
    let orgs = memberships_of(&state.pool, session.user_id)
        .await?
        .into_iter()
        .map(|membership| OrgRole {
            slug: membership.org_slug,
            role: membership.role,
        })
        .collect();

    let app_install_url = match state.scm.app_install_url(JobClass::Interactive).await {
        Ok(url) => Some(url),
        Err(err) => {
            warn!(
                error = &err as &dyn Error,
                "install page of the App not read"
            );
            None
        }
    };

    Ok(Json(Me {
        login,
        orgs,
        version: env!("CARGO_PKG_VERSION").to_owned(),
        app_install_url,
    }))
}

/// The repositories of the organization.
#[utoipa::path(
    get,
    path = "/orgs/{org}/repos",
    params(("org" = String, Path, description = "The slug of the organization")),
    responses((status = 200, body = RepoList)),
    security(("token" = ["queues:read"]))
)]
async fn list_repos(
    grant: ApiGrant,
    State(state): State<ApiState>,
    ApiPath(org): ApiPath<String>,
) -> Result<Json<RepoList>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;

    let mut tx = grant.begin_in_org(&state.pool, &org).await?;
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
#[into_params(parameter_in = Query)]
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

    let mut tx = grant.begin_in_org(&state.pool, &org).await?;
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

//! `/api/v1` (`specs/API.md`): the authentication of each
//! request by its API token or by the session of a person, the rate limit
//! of each of them, the routes of the platform (`/user`, `/instance`,
//! `/orgs/{org}` and its audit log, the sign-in of the command line), the routes of the modules, and the `OpenAPI` document of all
//! of them.

use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::sync::{Arc, Mutex, PoisonError};

use axum::extract::{DefaultBodyLimit, Request, State};
use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Extension, Json, Router};
use cihub_api_types::{
    AuditEntry, AuditList, CliSession, DeviceCode, DeviceCodeToken, ErrorCode, Instance, Org,
    OrgList, Problem, QueueChanged, User,
};
use cihub_domain::{
    ApiScope, ApiTokenId, JobClass, MembershipRole, OrgId, Permission, RepoId, SessionId, UserId,
};
use cihub_platform::{
    Access, ApiError, ApiGrant, ApiPath, ApiQuery, GrantActor, ModuleApi, PAGE_ERRORS, Page,
    PlatformError, QueueNewsFeed, cursor_of, document_error, error_responses, repo_permission_now,
    request_user_sync, request_user_sync_when_old, role_now,
};
use cihub_scm::Scm;
use cihub_store::{
    DatabaseCipher, ORG_IS_SUSPENDED, PgPool, SessionClient, SessionGrant, authenticate,
    authenticate_session, begin_org, begin_system, delete_session_by_id, delete_sessions_of,
    forget_tokens_without_session, memberships_of, profile_of, readable_repos,
};
use secrecy::SecretString;
use serde::Deserialize;
use serde_json::Value;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tracing::warn;
use utoipa::openapi::path::{Operation, PathItem};
use utoipa::openapi::security::{
    ApiKey, ApiKeyValue, Http, HttpAuthScheme, SecurityRequirement, SecurityScheme,
};
use utoipa::openapi::{RefOr, Response as DocumentResponse};
use utoipa::{Modify, OpenApi};

use crate::device::{
    __path_approve_device_code, __path_deny_device_code, __path_get_device_session,
    __path_start_device_sign_in, approve_device_code, deny_device_code, get_device_session,
    start_device_sign_in,
};
use crate::session::{SESSION_COOKIE, cookie, csrf_refused, has_csrf_token, needs_csrf_token};
use crate::stream::{__path_stream, Streams, stream};

pub const REQUESTS_PER_MINUTE: u32 = 600;

const MAX_BODY_BYTES: usize = 25 * 1024 * 1024;

#[derive(Clone)]
pub(crate) struct ApiState {
    pub pool: PgPool,
    pub now: fn() -> OffsetDateTime,
    scm: Arc<dyn Scm>,
    /// Opens the tokens of the people; `None` when nobody signs in.
    pub(crate) cipher: Option<Arc<DatabaseCipher>>,
    /// The root address of the instance, where the dashboard is.
    pub public_url: Option<String>,
    windows: Arc<Mutex<Windows>>,
}

/// Who calls, for the rate limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Caller {
    Token(ApiTokenId),
    Session(SessionId),
}

/// The requests of the current minute, per caller.
#[derive(Default)]
struct Windows {
    minute: i64,
    counts: HashMap<Caller, u32>,
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
    servers(
        (url = "/api/v1", description = "This instance"),
        (
            url = "{base_url}/api/v1",
            description = "Another instance",
            variables(("base_url" = (
                default = "https://cihub.example.com",
                description = "The address of the instance"
            )))
        )
    ),
    tags(
        (
            name = "Session",
            description = "The person signed in, the sign-in of the command line, and the \
                           instance."
        ),
        (
            name = "Organizations",
            description = "An organization is the account of an installation of the App, with \
                           its audit log."
        ),
        (
            name = "Live updates",
            description = "The changes of the queues of an organization, as server-sent events."
        )
    ),
    paths(
        get_user,
        list_user_orgs,
        delete_user_session,
        sync_user,
        start_device_sign_in,
        get_device_session,
        approve_device_code,
        deny_device_code,
        get_instance,
        get_org,
        list_audit,
        stream
    ),
    components(schemas(
        AuditEntry,
        AuditList,
        CliSession,
        DeviceCode,
        DeviceCodeToken,
        ErrorCode,
        Instance,
        Org,
        OrgList,
        Problem,
        QueueChanged,
        User
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

    document.info.description = Some(introduction());
    for (path, item) in &mut document.paths.paths {
        for (operation, changes_state) in operations(item) {
            accept_session(operation);
            document_access(path, operation, changes_state);
        }
    }
    document
}

/// The first page of the document: how to call the API, and every error code.
fn introduction() -> String {
    let codes: Vec<String> = ErrorCode::all()
        .map(|code| format!("| `{code}` | {} | {} |", code.status(), code.sentence()))
        .collect();

    format!(
        "{}\n| Code | Status | Meaning |\n| --- | --- | --- |\n{}\n",
        include_str!("openapi.md"),
        codes.join("\n")
    )
}

/// The operations of a path, each with whether it can change a state.
fn operations(item: &mut PathItem) -> impl Iterator<Item = (&mut Operation, bool)> {
    [
        (&mut item.get, false),
        (&mut item.put, true),
        (&mut item.post, true),
        (&mut item.delete, true),
        (&mut item.patch, true),
    ]
    .into_iter()
    .filter_map(|(operation, changes_state)| operation.as_mut().map(|op| (op, changes_state)))
}

/// Every route that a token reaches also answers a session, whose role gives
/// the scopes: the client of the dashboard is generated from this document.
fn accept_session(operation: &mut Operation) {
    let session = SecurityRequirement::new("session", Vec::<String>::new());

    if let Some(security) = operation.security.as_mut()
        && !security.contains(&session)
    {
        security.push(session);
    }
}

/// Says who may call the route, and adds the errors that every route of its
/// kind answers: the routes of a module declare only their own.
fn document_access(path: &str, operation: &mut Operation, changes_state: bool) {
    if operation.security.is_none() {
        document_public(operation);
        return;
    }

    let scopes = token_scopes(operation);
    let access = match &scopes {
        None => {
            "**Access**: the session of a person: the dashboard, or the command line.".to_owned()
        }
        Some(scopes) if scopes.is_empty() => "**Access**: any API token or session.".to_owned(),
        Some(scopes) => format!(
            "**Scope**: {}, for an API token, or for the role of the user of a session or their \
             permission on the repository of the route.",
            scopes.join(", ")
        ),
    };
    let mut codes = vec![
        ErrorCode::Unauthorized,
        ErrorCode::RateLimited,
        ErrorCode::Internal,
    ];

    operation.description = Some(match operation.description.take() {
        Some(description) => format!("{description}\n\n{access}"),
        None => access,
    });
    if path.starts_with("/orgs/{org}") {
        // A session checks the access of its person on the SCM after an hour.
        codes.extend([ErrorCode::OrgNotFound, ErrorCode::ScmUnavailable]);
    }
    if scopes.is_some_and(|scopes| !scopes.is_empty()) {
        codes.push(ErrorCode::ScopeMissing);
    }
    if changes_state {
        codes.push(ErrorCode::CsrfRefused);
    }
    for code in codes {
        document_error(&mut operation.responses, code);
    }
}

/// A route that needs no token or session: the sign-in of the command line.
/// No caller is known, so no rate limit applies.
fn document_public(operation: &mut Operation) {
    let access = "**Access**: no token or session.";

    operation.description = Some(match operation.description.take() {
        Some(description) => format!("{description}\n\n{access}"),
        None => access.to_owned(),
    });
    document_error(&mut operation.responses, ErrorCode::Internal);
}

/// The scopes that an API token needs on the route, as `` `queues:read` ``;
/// `None` for a route that no token reaches.
fn token_scopes(operation: &Operation) -> Option<Vec<String>> {
    let security = serde_json::to_value(&operation.security).unwrap_or_default();
    let token = security
        .as_array()?
        .iter()
        .find_map(|requirement| requirement.get("token")?.as_array())?;

    Some(
        token
            .iter()
            .filter_map(Value::as_str)
            .map(|scope| format!("`{scope}`"))
            .collect(),
    )
}

pub(crate) fn router(
    pool: PgPool,
    now: fn() -> OffsetDateTime,
    modules: Vec<ModuleApi>,
    scm: Arc<dyn Scm>,
    cipher: Option<Arc<DatabaseCipher>>,
    news: QueueNewsFeed,
    public_url: Option<String>,
) -> Router {
    let parts: Vec<_> = modules
        .iter()
        .map(|module| module.openapi.clone())
        .collect();
    let document = openapi(&parts);
    let streams = Streams {
        pool: pool.clone(),
        news,
        open: Arc::default(),
    };
    let state = ApiState {
        pool,
        now,
        scm,
        cipher,
        public_url,
        windows: Arc::default(),
    };
    let mut routes = Router::new()
        .route("/user", get(get_user))
        .route("/user/orgs", get(list_user_orgs))
        .route("/user/session", delete(delete_user_session))
        .route("/user/sync", post(sync_user))
        .route(
            "/device-codes/{user_code}/approve",
            post(approve_device_code),
        )
        .route("/device-codes/{user_code}/deny", post(deny_device_code))
        .route("/instance", get(get_instance))
        .route("/orgs/{org}", get(get_org))
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
        .fallback(|| async {
            ApiError::new(ErrorCode::RouteNotFound, "no route has this path".into())
        })
        .layer(middleware::from_fn_with_state(state.clone(), authenticated));
    let sign_in = Router::new()
        .route("/device-codes", post(start_device_sign_in))
        .route("/device-codes/token", post(get_device_session))
        .with_state(state);

    Router::new()
        .route(
            "/openapi.json",
            get(move || {
                let document = document.clone();
                async move { Json(document) }
            }),
        )
        .merge(sign_in)
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
    let path = request.uri().path();
    let authenticated = match bearer {
        Some(secret) if SessionClient::of(&secret) == Some(SessionClient::CommandLine) => {
            by_session(&state, &secret, path, now).await
        }
        Some(secret) => by_token(&state, &secret, now).await,
        None => by_cookie(&state, request.headers(), request.method(), path, now).await,
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

async fn by_token(
    state: &ApiState,
    secret: &SecretString,
    now: OffsetDateTime,
) -> Result<Authenticated, ApiError> {
    let grant = authenticate(&state.pool, secret, now)
        .await?
        .ok_or_else(ApiError::unauthorized)?;

    Ok(Authenticated::Token(
        grant.id,
        ApiGrant {
            org_id: grant.org_id,
            actor: GrantActor::Token(grant.id),
            scopes: grant.scopes,
            repos: None,
        },
    ))
}

/// The session of the dashboard in the cookie of the request. A request
/// that can change a state also carries its CSRF token; a session of the
/// command line is never a cookie.
async fn by_cookie(
    state: &ApiState,
    headers: &HeaderMap,
    method: &Method,
    path: &str,
    now: OffsetDateTime,
) -> Result<Authenticated, ApiError> {
    let secret = cookie(headers, SESSION_COOKIE)
        .filter(|secret| SessionClient::of(secret) == Some(SessionClient::Dashboard))
        .ok_or_else(ApiError::unauthorized)?;
    // Before the session acts: a cross-site request costs no SCM call and
    // no job.
    if needs_csrf_token(method) && !has_csrf_token(headers, &secret) {
        return Err(csrf_refused());
    }

    by_session(state, &secret, path, now).await
}

/// A session, and on a route of an organization the grant of its user there
/// (`specs/SECURITY.md`): on a route of a repository, the scopes of their
/// permission on it; elsewhere, the scopes of their role and the
/// repositories that they read. An access older than an hour is checked
/// again on the SCM with the token of the person. An organization where the
/// user has no role answers `404`, as for a token of another organization,
/// and a repository that they do not read too.
async fn by_session(
    state: &ApiState,
    secret: &SecretString,
    path: &str,
    now: OffsetDateTime,
) -> Result<Authenticated, ApiError> {
    let session = authenticate_session(&state.pool, secret, now)
        .await?
        .ok_or_else(ApiError::unauthorized)?;

    let Some(slug) = org_of_path(path) else {
        return Ok(Authenticated::Session(session, None));
    };

    let user_id = session.user_id;
    let org_not_found = || {
        ApiError::new(
            ErrorCode::OrgNotFound,
            format!("organization `{slug}` not found"),
        )
    };
    let membership = memberships_of(&state.pool, user_id)
        .await?
        .into_iter()
        .find(|membership| membership.org_slug.eq_ignore_ascii_case(slug))
        .ok_or_else(org_not_found)?;
    let cipher = state.cipher.as_deref().ok_or_else(sign_in_again)?;
    let role = role_now(
        &state.pool,
        state.scm.as_ref(),
        cipher,
        user_id,
        &membership,
        now,
    )
    .await;
    let role = granted(state, user_id, role)
        .await?
        .ok_or_else(org_not_found)?;
    if let Err(err) = request_user_sync_when_old(&state.pool, user_id, now).await {
        warn!(
            error = &err as &dyn Error,
            "sync of the person not requested"
        );
    }

    let Some(name) = repo_of_path(path) else {
        let mut tx = begin_org(&state.pool, membership.org_id).await?;
        let repos = readable_repos(&mut tx, user_id).await?;
        tx.commit().await?;
        let grant = ApiGrant {
            org_id: membership.org_id,
            actor: GrantActor::User(user_id),
            scopes: role.scopes().to_vec(),
            repos: Some(repos),
        };
        return Ok(Authenticated::Session(session, Some(grant)));
    };

    let permission = repo_permission_now(
        &state.pool,
        state.scm.as_ref(),
        cipher,
        user_id,
        membership.org_id,
        name,
        now,
    )
    .await;
    let grant = repo_grant(
        (membership.org_id, user_id, role),
        name,
        granted(state, user_id, permission).await?,
    )?;
    Ok(Authenticated::Session(session, Some(grant)))
}

/// What a check of the access gave, or why the request stops: `503` when the
/// SCM does not answer, `401` when the person must sign in again, whose
/// sessions then end.
async fn granted<T>(
    state: &ApiState,
    user_id: UserId,
    access: Result<Access<T>, PlatformError>,
) -> Result<T, ApiError> {
    match access {
        Ok(Access::Granted(granted)) => Ok(granted),
        Ok(Access::SignInAgain) => {
            delete_sessions_of(&state.pool, user_id).await?;
            Err(sign_in_again())
        }
        Err(PlatformError::Scm(err)) => {
            warn!(
                error = &err as &dyn Error,
                "access of the person not checked"
            );
            Err(ApiError::new(
                ErrorCode::ScmUnavailable,
                "the SCM did not answer the check of the access".into(),
            ))
        }
        Err(err) => Err(err.into()),
    }
}

fn sign_in_again() -> ApiError {
    ApiError::new(
        ErrorCode::Unauthorized,
        "the SCM refuses the token of the person, or CIHub has none: sign in again".into(),
    )
}

/// The grant of a route of the repository `name`, from the permission of
/// the person on it. A repository that the organization does not have is
/// left to the route, which answers `404` with the scopes of `role`.
fn repo_grant(
    (org_id, user_id, role): (OrgId, UserId, MembershipRole),
    name: &str,
    permission: Option<(RepoId, Permission)>,
) -> Result<ApiGrant, ApiError> {
    let scopes = match permission {
        Some((_, Permission::None)) => {
            return Err(ApiError::new(
                ErrorCode::RepoNotFound,
                format!("repository `{name}` not found"),
            ));
        }
        Some((_, Permission::Read)) => vec![ApiScope::QueuesRead],
        Some((_, Permission::Write)) => vec![ApiScope::QueuesRead, ApiScope::PrsWrite],
        Some((_, Permission::Admin)) => vec![
            ApiScope::QueuesRead,
            ApiScope::QueuesWrite,
            ApiScope::PrsWrite,
        ],
        None => role.scopes().to_vec(),
    };

    Ok(ApiGrant {
        org_id,
        actor: GrantActor::User(user_id),
        scopes,
        repos: None,
    })
}

/// The slug of the organization that the path names, as `/orgs/{org}/…`.
fn org_of_path(path: &str) -> Option<&str> {
    path.strip_prefix("/orgs/")?
        .split('/')
        .next()
        .filter(|slug| !slug.is_empty())
}

/// The name of the repository that the path names, as
/// `/orgs/{org}/repos/{repo}/…`.
fn repo_of_path(path: &str) -> Option<&str> {
    let mut segments = path.strip_prefix("/orgs/")?.split('/').skip(1);

    (segments.next() == Some("repos"))
        .then(|| segments.next())
        .flatten()
        .filter(|name| !name.is_empty())
}

fn over_limit(state: &ApiState, caller: Caller, now: OffsetDateTime) -> Option<u64> {
    let minute = now.unix_timestamp().div_euclid(60);
    let mut windows = state.windows.lock().unwrap_or_else(PoisonError::into_inner);

    if windows.minute != minute {
        windows.minute = minute;
        windows.counts.clear();
    }
    let count = windows.counts.entry(caller).or_insert(0);
    if *count >= REQUESTS_PER_MINUTE {
        let elapsed = now.unix_timestamp().rem_euclid(60);
        return Some(u64::try_from(60 - elapsed).unwrap_or(60));
    }

    *count += 1;
    None
}

/// Forgets the token of each person whose last session ended.
pub(crate) async fn forget_tokens_of_the_gone(pool: &PgPool) -> Result<(), ApiError> {
    let mut tx = begin_system(pool).await?;
    forget_tokens_without_session(&mut tx).await?;
    tx.commit().await?;
    Ok(())
}

/// The session of the request; `401` for an API token, which stands for no
/// person.
pub(crate) fn session_of(
    session: Option<Extension<SessionGrant>>,
) -> Result<SessionGrant, ApiError> {
    session.map(|Extension(session)| session).ok_or_else(|| {
        ApiError::new(
            ErrorCode::Unauthorized,
            "the route needs the session of a person".into(),
        )
    })
}

/// Get the user
///
/// The person of the session of the dashboard. An API token stands for no
/// person: it gets `401`.
#[utoipa::path(
    get,
    path = "/user",
    tag = "Session",
    responses((status = 200, description = "The person of the session.", body = User)),
    security(("session" = []))
)]
async fn get_user(
    State(state): State<ApiState>,
    session: Option<Extension<SessionGrant>>,
) -> Result<Json<User>, ApiError> {
    let session = session_of(session)?;

    let profile = profile_of(&state.pool, session.user_id)
        .await?
        .ok_or_else(ApiError::unauthorized)?;

    Ok(Json(User {
        login: profile.login,
        name: profile.name,
        avatar_url: profile.avatar_url,
    }))
}

/// End the session
///
/// Ends the session of the request: the sign-out of the command line. The
/// dashboard signs out with `POST /auth/sign-out`, which also clears its
/// cookies.
#[utoipa::path(
    delete,
    path = "/user/session",
    tag = "Session",
    responses((status = 204, description = "The session ended.")),
    security(("session" = []))
)]
async fn delete_user_session(
    State(state): State<ApiState>,
    session: Option<Extension<SessionGrant>>,
) -> Result<StatusCode, ApiError> {
    let session = session_of(session)?;

    delete_session_by_id(&state.pool, session.id).await?;
    forget_tokens_of_the_gone(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Sync the user
///
/// Asks CIHub to read again from the SCM now the installations and the
/// repositories that the person of the session reaches, at most once a
/// minute.
#[utoipa::path(
    post,
    path = "/user/sync",
    tag = "Session",
    responses((status = 202, description = "CIHub takes the request.")),
    security(("session" = []))
)]
async fn sync_user(
    State(state): State<ApiState>,
    session: Option<Extension<SessionGrant>>,
) -> Result<StatusCode, ApiError> {
    let session = session_of(session)?;
    let now = (state.now)();
    let minute = format!("asked-{}", now.unix_timestamp().div_euclid(60));

    let mut tx = begin_system(&state.pool).await?;
    request_user_sync(&mut tx, session.user_id, &minute, now).await?;
    tx.commit().await?;

    Ok(StatusCode::ACCEPTED)
}

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
struct PageQuery {
    /// The `cursor` of the previous page.
    from: Option<String>,
    /// How many entries, from 1 to 100; 50 by default.
    limit: Option<u32>,
}

/// The errors of a route that gives a page, and nothing else.
struct PageErrors;

impl utoipa::IntoResponses for PageErrors {
    fn responses() -> BTreeMap<String, RefOr<DocumentResponse>> {
        error_responses(PAGE_ERRORS)
    }
}

/// List the organizations of the user
///
/// The organizations where the person of the session has a role, by slug;
/// none before the App is installed on an account of the person.
#[utoipa::path(
    get,
    path = "/user/orgs",
    tag = "Session",
    params(PageQuery),
    responses(
        (status = 200, description = "A page of organizations.", body = OrgList),
        PageErrors
    ),
    security(("session" = []))
)]
async fn list_user_orgs(
    State(state): State<ApiState>,
    session: Option<Extension<SessionGrant>>,
    ApiQuery(query): ApiQuery<PageQuery>,
) -> Result<Json<OrgList>, ApiError> {
    let session = session_of(session)?;
    let page: Page<String> = Page::read("orgs", query.from.as_deref(), query.limit)?;

    let mut memberships = memberships_of(&state.pool, session.user_id).await?;
    let count = u64::try_from(memberships.len())?;
    // The page compares the slugs as Rust does: they are sorted the same way,
    // not by the collation of Postgres.
    memberships.sort_by(|a, b| a.org_slug.cmp(&b.org_slug));
    let rows: Vec<Org> = memberships
        .into_iter()
        .filter(|membership| {
            page.after
                .as_ref()
                .is_none_or(|after| membership.org_slug > *after)
        })
        .map(|membership| Org {
            slug: membership.org_slug,
            role: Some(membership.role),
            is_suspended: membership.is_org_suspended,
        })
        .collect();
    let (orgs, has_more) = page.split(rows);
    let cursor = orgs
        .last()
        .filter(|_| has_more)
        .map(|org| cursor_of("orgs", &org.slug));

    Ok(Json(OrgList {
        orgs,
        has_more,
        count,
        cursor,
    }))
}

/// Get the instance
///
/// The version of the instance, and the page of the SCM that installs the
/// App.
#[utoipa::path(
    get,
    path = "/instance",
    tag = "Session",
    responses((status = 200, description = "The instance.", body = Instance)),
    security(("token" = []))
)]
async fn get_instance(State(state): State<ApiState>) -> Json<Instance> {
    let app_install_url = state
        .scm
        .app_install_url(JobClass::Interactive)
        .await
        .inspect_err(|err| {
            warn!(
                error = err as &dyn Error,
                "install page of the App not read"
            );
        })
        .ok();

    Json(Instance {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        app_install_url,
    })
}

/// Get an organization
///
/// The organization, and the role of the person of the session in it.
#[utoipa::path(
    get,
    path = "/orgs/{org}",
    tag = "Organizations",
    params(("org" = String, Path, description = "The slug of the organization.", example = "acme")),
    responses((status = 200, description = "The organization.", body = Org)),
    security(("token" = []))
)]
async fn get_org(
    grant: ApiGrant,
    State(state): State<ApiState>,
    ApiPath(org): ApiPath<String>,
) -> Result<Json<Org>, ApiError> {
    let mut tx = grant.begin_in_org(&state.pool, &org).await?;
    let (slug, is_suspended): (String, bool) = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT o.slug, {ORG_IS_SUSPENDED} FROM orgs o WHERE o.id = $1"
    )))
    .bind(grant.org_id.to_string())
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;

    let role = match grant.actor {
        GrantActor::Token(_) => None,
        GrantActor::User(user_id) => memberships_of(&state.pool, user_id)
            .await?
            .into_iter()
            .find(|membership| membership.org_id == grant.org_id)
            .map(|membership| membership.role),
    };

    Ok(Json(Org {
        slug,
        role,
        is_suspended,
    }))
}

/// List the audit log
///
/// The changes of the queues and the actions on the PRs that a person or a
/// token asked for, newest first.
#[utoipa::path(
    get,
    path = "/orgs/{org}/audit",
    tag = "Organizations",
    params(
        ("org" = String, Path, description = "The slug of the organization.", example = "acme"),
        PageQuery
    ),
    responses(
        (status = 200, description = "A page of the audit log.", body = AuditList),
        PageErrors
    ),
    security(("token" = ["queues:write"]))
)]
async fn list_audit(
    grant: ApiGrant,
    State(state): State<ApiState>,
    ApiPath(org): ApiPath<String>,
    ApiQuery(query): ApiQuery<PageQuery>,
) -> Result<Json<AuditList>, ApiError> {
    grant.require(ApiScope::QueuesWrite)?;
    let page: Page<(String, String)> = Page::read("audit", query.from.as_deref(), query.limit)?;

    let mut tx = grant.begin_in_org(&state.pool, &org).await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_log")
        .fetch_one(&mut *tx)
        .await?;
    let (occurred_at, id) = page.after.clone().unzip();
    let rows: Vec<AuditRow> = sqlx::query_as(
        "SELECT id, action, target, actor_user_id, before, after, occurred_at FROM audit_log
         WHERE $1::text IS NULL OR (occurred_at, id) < ($1::timestamptz, $2)
         ORDER BY occurred_at DESC, id DESC
         LIMIT $3",
    )
    .bind(occurred_at)
    .bind(id)
    .bind(page.rows())
    .fetch_all(&mut *tx)
    .await?;
    let entries = rows
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
    let (entries, has_more): (Vec<AuditEntry>, bool) = page.split(entries);
    let cursor = entries
        .last()
        .filter(|_| has_more)
        .map(audit_cursor)
        .transpose()?;

    Ok(Json(AuditList {
        entries,
        has_more,
        count: u64::try_from(count)?,
        cursor,
    }))
}

fn audit_cursor(entry: &AuditEntry) -> Result<String, ApiError> {
    let occurred_at = entry.occurred_at.format(&Rfc3339)?;

    Ok(cursor_of("audit", (occurred_at, &entry.id)))
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

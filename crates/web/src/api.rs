//! `/api/v1` (`specs/API.md`): the authentication of each
//! request by its API token or by a session of the dashboard, the rate limit
//! of each of them, the routes of the platform (`/user`, `/instance`,
//! `/orgs/{org}`, its repositories and its audit log), the routes of the
//! modules, and the `OpenAPI` document of all of them.

use std::collections::{BTreeMap, HashMap};
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
    AuditEntry, AuditList, ErrorCode, Instance, Org, OrgList, Problem, QueueChanged, Repo,
    RepoList, User,
};
use cihub_domain::{ApiScope, ApiTokenId, JobClass, SessionId};
use cihub_platform::{
    ApiError, ApiGrant, ApiPath, ApiQuery, GrantActor, ModuleApi, PAGE_ERRORS, Page, QueueNewsFeed,
    cursor_of, document_error, error_responses,
};
use cihub_scm::Scm;
use cihub_store::{
    PgPool, SessionGrant, authenticate, authenticate_session, memberships_of, profile_of,
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
            description = "The person signed in to the dashboard, and the instance."
        ),
        (
            name = "Organizations",
            description = "An organization is the account of an installation of the App: its \
                           repositories and its audit log."
        ),
        (
            name = "Live updates",
            description = "The changes of the queues of an organization, as server-sent events."
        )
    ),
    paths(
        get_user,
        list_user_orgs,
        get_instance,
        get_org,
        list_repos,
        get_repo,
        list_audit,
        stream
    ),
    components(schemas(
        AuditEntry,
        AuditList,
        ErrorCode,
        Instance,
        Org,
        OrgList,
        Problem,
        QueueChanged,
        Repo,
        RepoList,
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
    let scopes = token_scopes(operation);
    let access = match &scopes {
        None => "**Access**: a session of the dashboard only.".to_owned(),
        Some(scopes) if scopes.is_empty() => "**Access**: any API token or session.".to_owned(),
        Some(scopes) => format!(
            "**Scope**: {}, for an API token or for the role of the user of a session.",
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
        codes.push(ErrorCode::OrgNotFound);
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
        open: Arc::default(),
    };
    let state = ApiState {
        pool,
        now,
        scm,
        windows: Arc::default(),
    };
    let mut routes = Router::new()
        .route("/user", get(get_user))
        .route("/user/orgs", get(list_user_orgs))
        .route("/instance", get(get_instance))
        .route("/orgs/{org}", get(get_org))
        .route("/orgs/{org}/repos", get(list_repos))
        .route("/orgs/{org}/repos/{repo}", get(get_repo))
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
    let secret = cookie(headers, SESSION_COOKIE).ok_or_else(ApiError::unauthorized)?;
    let session = authenticate_session(&state.pool, &secret, now)
        .await?
        .ok_or_else(ApiError::unauthorized)?;

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
        .ok_or_else(|| {
            ApiError::new(
                ErrorCode::OrgNotFound,
                format!("organization `{slug}` not found"),
            )
        })?;
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

/// The session of the request; `401` for an API token, which stands for no
/// person.
fn session_of(session: Option<Extension<SessionGrant>>) -> Result<SessionGrant, ApiError> {
    session.map(|Extension(session)| session).ok_or_else(|| {
        ApiError::new(
            ErrorCode::Unauthorized,
            "the route needs a session of the dashboard".into(),
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
    let slug: String = sqlx::query_scalar("SELECT slug FROM orgs WHERE id = $1")
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

    Ok(Json(Org { slug, role }))
}

/// List the repositories
///
/// The repositories of the organization that the App reaches, by namespace
/// then name.
#[utoipa::path(
    get,
    path = "/orgs/{org}/repos",
    tag = "Organizations",
    params(
        ("org" = String, Path, description = "The slug of the organization.", example = "acme"),
        PageQuery
    ),
    responses(
        (status = 200, description = "A page of repositories.", body = RepoList),
        PageErrors
    ),
    security(("token" = ["queues:read"]))
)]
async fn list_repos(
    grant: ApiGrant,
    State(state): State<ApiState>,
    ApiPath(org): ApiPath<String>,
    ApiQuery(query): ApiQuery<PageQuery>,
) -> Result<Json<RepoList>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;
    let page: Page<(String, String, String)> =
        Page::read("repos", query.from.as_deref(), query.limit)?;

    let mut tx = grant.begin_in_org(&state.pool, &org).await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM repos")
        .fetch_one(&mut *tx)
        .await?;
    let (namespace, name, id) = match page.after.clone() {
        Some((namespace, name, id)) => (Some(namespace), Some(name), Some(id)),
        None => (None, None, None),
    };
    // A repository deleted then created again keeps its row: the identifier
    // tells two repositories of the same name apart.
    let rows: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT id, namespace, name, default_branch FROM repos
         WHERE $1::text IS NULL OR (namespace, name, id) > ($1, $2, $3)
         ORDER BY namespace, name, id LIMIT $4",
    )
    .bind(namespace)
    .bind(name)
    .bind(id)
    .bind(page.rows())
    .fetch_all(&mut *tx)
    .await?;
    let repos = rows.into_iter().map(repo_of).collect();
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

fn repo_of((id, namespace, name, default_branch): (String, String, String, String)) -> Repo {
    Repo {
        id,
        namespace,
        name,
        default_branch,
    }
}

/// The errors of the route of a repository.
struct RepoErrors;

impl utoipa::IntoResponses for RepoErrors {
    fn responses() -> BTreeMap<String, RefOr<DocumentResponse>> {
        error_responses(&[ErrorCode::RepoNotFound])
    }
}

/// Get a repository
#[utoipa::path(
    get,
    path = "/orgs/{org}/repos/{repo}",
    tag = "Organizations",
    description = "The repository, whatever the case of its name.",
    params(
        ("org" = String, Path, description = "The slug of the organization.", example = "acme"),
        ("repo" = String, Path, description = "The name of the repository.", example = "web")
    ),
    responses((status = 200, description = "The repository.", body = Repo), RepoErrors),
    security(("token" = ["queues:read"]))
)]
async fn get_repo(
    grant: ApiGrant,
    State(state): State<ApiState>,
    ApiPath((org, repo)): ApiPath<(String, String)>,
) -> Result<Json<Repo>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;

    let mut tx = grant.begin_in_org(&state.pool, &org).await?;
    // The SCM does not tell names apart by their case.
    let row: Option<(String, String, String, String)> = sqlx::query_as(
        "SELECT id, namespace, name, default_branch FROM repos
         WHERE lower(name) = lower($1) ORDER BY id LIMIT 1",
    )
    .bind(&repo)
    .fetch_optional(&mut *tx)
    .await?;

    row.map(|row| Json(repo_of(row))).ok_or_else(|| {
        ApiError::new(
            ErrorCode::RepoNotFound,
            format!("repository `{repo}` not found"),
        )
    })
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

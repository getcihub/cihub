use std::collections::BTreeMap;
use std::error::Error;

use axum::Json;
use axum::extract::{FromRequest, FromRequestParts, Path, Query, Request};
use axum::http::request::Parts;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use cihub_api_types::{ErrorCode, PROBLEM_JSON, Problem};
use cihub_domain::{ApiScope, ApiTokenId, OrgId, RepoId, UserId};
use cihub_store::{PgPool, Postgres, Transaction, begin_org};
use serde::de::DeserializeOwned;
use tracing::error;
use utoipa::openapi::example::Example;
use utoipa::openapi::schema::{ObjectBuilder, Type};
use utoipa::openapi::{self, Content, Header, Ref, RefOr, Responses};

/// What a request may do in the organization of its URL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiGrant {
    pub org_id: OrgId,
    pub actor: GrantActor,
    pub scopes: Vec<ApiScope>,
    /// The repositories that a person reads, for the lists of the
    /// organization; `None` for every repository: an API token, or a route of
    /// a repository that the person reads.
    pub repos: Option<Vec<RepoId>>,
}

/// Who acts: an API token, or a user through a session of the dashboard,
/// with the scopes of their role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GrantActor {
    Token(ApiTokenId),
    User(UserId),
}

impl ApiGrant {
    /// The identifiers of [`ApiGrant::repos`], as a filter of a query: a
    /// `NULL` array keeps every repository.
    pub fn repo_filter(&self) -> Option<Vec<String>> {
        self.repos
            .as_ref()
            .map(|repos| repos.iter().map(ToString::to_string).collect())
    }

    /// # Errors
    ///
    /// The token, or the role or the permission of the user, lacks `scope`.
    pub fn require(&self, scope: ApiScope) -> Result<(), ApiError> {
        if self.scopes.contains(&scope) {
            return Ok(());
        }

        let who = match self.actor {
            GrantActor::Token(_) => "the token",
            GrantActor::User(_) => "the role or the permission of the user",
        };
        Err(ApiError::new(
            ErrorCode::ScopeMissing,
            format!("{who} lacks the scope `{scope}`"),
        ))
    }

    /// A transaction of the organization whose slug is `org`, which the URL
    /// names (`specs/API.md`).
    ///
    /// # Errors
    ///
    /// Not found when the grant does not belong to `org`, never forbidden: a
    /// caller does not learn that another organization exists. Postgres fails.
    pub async fn begin_in_org(
        &self,
        pool: &PgPool,
        org: &str,
    ) -> Result<Transaction<'static, Postgres>, ApiError> {
        let mut tx = begin_org(pool, self.org_id).await?;

        // Row level security shows only the organization of the grant; the SCM
        // does not tell names apart by their case.
        let is_grant_org: bool =
            sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM orgs WHERE lower(slug) = lower($1))")
                .bind(org)
                .fetch_one(&mut *tx)
                .await?;

        if !is_grant_org {
            return Err(ApiError::new(
                ErrorCode::OrgNotFound,
                format!("organization `{org}` not found"),
            ));
        }

        Ok(tx)
    }
}

impl<S: Send + Sync> FromRequestParts<S> for ApiGrant {
    type Rejection = ApiError;

    fn from_request_parts(
        parts: &mut Parts,
        _: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        std::future::ready(
            parts
                .extensions
                .get::<Self>()
                .cloned()
                .ok_or_else(ApiError::unauthorized),
        )
    }
}

#[derive(Debug)]
pub struct ApiError {
    pub code: ErrorCode,
    pub detail: String,
    pub retry_after: Option<u64>,
}

impl ApiError {
    pub fn new(code: ErrorCode, detail: String) -> Self {
        Self {
            code,
            detail,
            retry_after: None,
        }
    }

    pub fn unauthorized() -> Self {
        Self::new(
            ErrorCode::Unauthorized,
            "no valid API token or session".into(),
        )
    }

    pub fn invalid(detail: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidRequest, detail.into())
    }

    fn internal(err: &(dyn Error + 'static)) -> Self {
        error!(error = err, "API request failed");
        Self::new(ErrorCode::Internal, "the server failed".into())
    }

    pub fn status(&self) -> StatusCode {
        status_of(self.code)
    }
}

fn status_of(code: ErrorCode) -> StatusCode {
    StatusCode::from_u16(code.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
}

fn problem(code: ErrorCode, detail: String) -> Problem {
    let status = status_of(code);

    Problem {
        kind: "about:blank".to_owned(),
        title: status.canonical_reason().unwrap_or_default().to_owned(),
        status: status.as_u16(),
        detail,
        code,
    }
}

/// The responses of the `OpenAPI` document for the errors `codes`, as an
/// `IntoResponses` of a route gives them.
pub fn error_responses(codes: &[ErrorCode]) -> BTreeMap<String, RefOr<openapi::Response>> {
    let mut responses = Responses::new();
    for code in codes {
        document_error(&mut responses, *code);
    }

    responses.responses
}

/// The example of each error code, for the components of the `OpenAPI`
/// document: the responses of the routes refer to them by name.
pub fn error_examples() -> BTreeMap<String, RefOr<Example>> {
    ErrorCode::all()
        .map(|code| {
            let mut example = Example::new();
            example.summary = code.to_string();
            example.value = serde_json::to_value(problem(code, code.sentence().to_owned())).ok();

            (code.to_string(), RefOr::T(example))
        })
        .collect()
}

/// Adds the error `code` to the responses of a route in the `OpenAPI`
/// document: the response of its status lists it, with a reference to its
/// example in [`error_examples`].
pub fn document_error(responses: &mut Responses, code: ErrorCode) {
    let status = status_of(code);
    let entry = responses
        .responses
        .entry(status.as_str().to_owned())
        .or_insert_with(|| RefOr::T(openapi::Response::new("")));
    let RefOr::T(response) = entry else {
        return;
    };

    let entry = response
        .content
        .entry(PROBLEM_JSON.to_owned())
        .or_insert_with(|| RefOr::T(Content::new(Some(Ref::from_schema_name("Problem")))));
    let RefOr::T(content) = entry else {
        return;
    };

    let name = code.to_string();

    if content.examples.contains_key(&name) {
        return;
    }

    content.examples.insert(
        name.clone(),
        RefOr::Ref(Ref::new(format!("#/components/examples/{name}"))),
    );

    let line = format!("- `{name}`: {}", code.sentence());
    response.description = if response.description.is_empty() {
        line
    } else {
        format!("{}\n{line}", response.description)
    };

    if code == ErrorCode::RateLimited {
        let mut retry_after = Header::new(ObjectBuilder::new().schema_type(Type::Integer).build());
        retry_after.description = Some("The seconds to wait before the next request.".to_owned());
        response
            .headers
            .insert("Retry-After".to_owned(), RefOr::T(retry_after));
    }
}

impl<E: Error + 'static> From<E> for ApiError {
    fn from(err: E) -> Self {
        Self::internal(&err)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = (self.status(), Json(problem(self.code, self.detail))).into_response();
        let headers = response.headers_mut();

        headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(PROBLEM_JSON));
        if let Some(seconds) = self.retry_after {
            headers.insert(header::RETRY_AFTER, HeaderValue::from(seconds));
        }

        response
    }
}

pub struct ApiJson<T>(pub T);

impl<T: DeserializeOwned, S: Send + Sync> FromRequest<S> for ApiJson<T> {
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        Json::<T>::from_request(request, state)
            .await
            .map(|Json(value)| Self(value))
            .map_err(|err| ApiError::invalid(err.body_text()))
    }
}

pub struct ApiPath<T>(pub T);

impl<T: DeserializeOwned + Send, S: Send + Sync> FromRequestParts<S> for ApiPath<T> {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Path::<T>::from_request_parts(parts, state)
            .await
            .map(|Path(value)| Self(value))
            .map_err(|err| ApiError::invalid(err.body_text()))
    }
}

pub struct ApiQuery<T>(pub T);

impl<T: DeserializeOwned, S: Send + Sync> FromRequestParts<S> for ApiQuery<T> {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Query::<T>::from_request_parts(parts, state)
            .await
            .map(|Query(value)| Self(value))
            .map_err(|err| ApiError::invalid(err.body_text()))
    }
}

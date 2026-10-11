use std::error::Error;

use axum::Json;
use axum::extract::{FromRequest, FromRequestParts, Path, Query, Request};
use axum::http::request::Parts;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use cihub_api_types::{ErrorCode, PROBLEM_JSON, Problem};
use cihub_domain::{ApiScope, ApiTokenId, OrgId, UserId};
use cihub_store::{PgPool, Postgres, Transaction, begin_org};
use serde::de::DeserializeOwned;
use tracing::error;

/// What a request may do in the organization of its URL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiGrant {
    pub org_id: OrgId,
    pub actor: GrantActor,
    pub scopes: Vec<ApiScope>,
}

/// Who acts: an API token, or a user through a session of the dashboard,
/// with the scopes of their role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GrantActor {
    Token(ApiTokenId),
    User(UserId),
}

impl ApiGrant {
    /// # Errors
    ///
    /// The token, or the role of the user, lacks `scope`.
    pub fn require(&self, scope: ApiScope) -> Result<(), ApiError> {
        if self.scopes.contains(&scope) {
            return Ok(());
        }

        let who = match self.actor {
            GrantActor::Token(_) => "the token",
            GrantActor::User(_) => "the role of the user",
        };
        Err(ApiError::new(
            ErrorCode::Forbidden,
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
            return Err(ApiError::not_found("organization"));
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
        std::future::ready(parts.extensions.get::<Self>().cloned().ok_or_else(|| {
            ApiError::new(
                ErrorCode::Unauthorized,
                "no valid API token or session".into(),
            )
        }))
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

    pub fn not_found(what: &str) -> Self {
        Self::new(ErrorCode::NotFound, format!("{what} not found"))
    }

    pub fn invalid(detail: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidRequest, detail.into())
    }

    fn internal(err: &(dyn Error + 'static)) -> Self {
        error!(error = err, "API request failed");
        Self::new(ErrorCode::Internal, "the server failed".into())
    }

    pub fn status(&self) -> StatusCode {
        match self.code {
            ErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
            ErrorCode::Forbidden | ErrorCode::CsrfRefused => StatusCode::FORBIDDEN,
            ErrorCode::NotFound => StatusCode::NOT_FOUND,
            ErrorCode::InvalidRequest => StatusCode::BAD_REQUEST,
            ErrorCode::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            ErrorCode::InvalidState => StatusCode::CONFLICT,
            ErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl<E: Error + 'static> From<E> for ApiError {
    fn from(err: E) -> Self {
        Self::internal(&err)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status();
        let problem = Problem {
            kind: "about:blank".to_owned(),
            title: status.canonical_reason().unwrap_or_default().to_owned(),
            status: status.as_u16(),
            detail: self.detail,
            code: self.code,
        };
        let mut response = (status, Json(problem)).into_response();
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

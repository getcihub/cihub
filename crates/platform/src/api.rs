use std::error::Error;

use axum::Json;
use axum::extract::{FromRequest, FromRequestParts, Path, Query, Request};
use axum::http::request::Parts;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use cihub_api_types::{ErrorCode, PROBLEM_JSON, Problem};
use cihub_domain::{ApiScope, ApiTokenId, OrgId};
use serde::de::DeserializeOwned;
use tracing::error;

/// What the API token of a request lets its caller do, as the web role
/// authenticates it (`DECISIONS.md` D159). A route reads it to know the
/// organization and to check its scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiGrant {
    pub org_id: OrgId,
    pub token_id: ApiTokenId,
    pub scopes: Vec<ApiScope>,
}

impl ApiGrant {
    /// # Errors
    ///
    /// The token lacks `scope`.
    pub fn require(&self, scope: ApiScope) -> Result<(), ApiError> {
        if self.scopes.contains(&scope) {
            Ok(())
        } else {
            Err(ApiError::new(
                ErrorCode::Forbidden,
                format!("the token lacks the scope `{scope}`"),
            ))
        }
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
                .ok_or_else(|| ApiError::new(ErrorCode::Unauthorized, "no valid API token".into())),
        )
    }
}

/// An error of the API, rendered as `application/problem+json`.
#[derive(Debug)]
pub struct ApiError {
    pub code: ErrorCode,
    pub detail: String,
    /// The seconds before a new request, for `rate_limited`.
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

    /// An error of the server: the log keeps its cause, the answer does not.
    fn internal(err: &(dyn Error + 'static)) -> Self {
        error!(error = err, "API request failed");
        Self::new(ErrorCode::Internal, "the server failed".into())
    }

    pub fn status(&self) -> StatusCode {
        match self.code {
            ErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
            ErrorCode::Forbidden => StatusCode::FORBIDDEN,
            ErrorCode::NotFound => StatusCode::NOT_FOUND,
            ErrorCode::InvalidRequest => StatusCode::BAD_REQUEST,
            ErrorCode::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            ErrorCode::QueueExists | ErrorCode::InvalidState => StatusCode::CONFLICT,
            ErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// Any error that a route does not name is an error of the server.
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

/// A JSON body; a body that does not parse gives `invalid_request`.
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

/// The parameters of the path; a path that does not parse gives
/// `invalid_request`.
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

/// The query string; a query that does not parse gives `invalid_request`.
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

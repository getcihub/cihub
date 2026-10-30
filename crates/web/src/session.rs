//! The sessions of the dashboard and of the command line on the server: their
//! table, the cookies of the dashboard, its CSRF token and the sign-out
//! (`specs/SECURITY.md`).

use std::fmt::Write;

use axum::extract::State;
use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use cihub_api_types::ErrorCode;
use cihub_domain::{
    SessionId, UserId, format_cli_session_token, format_session_token, is_cli_session_token,
    is_session_token,
};
use cihub_platform::ApiError;
use cihub_store::{PgPool, StoreError, draw_secret_bytes, secret_hash};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use sqlx::PgConnection;
use time::{Duration, OffsetDateTime};

use crate::api::forget_tokens_of_the_gone;

/// The cookie that holds the secret of the session; no script reads it.
pub(crate) const SESSION_COOKIE: &str = "__Host-cihub_session";
/// The cookie that gives the CSRF token to the scripts of the dashboard,
/// which send it back in [`CSRF_HEADER`].
pub(crate) const CSRF_COOKIE: &str = "__Host-cihub_csrf";
pub(crate) const CSRF_HEADER: &str = "x-csrf-token";
/// The attributes of both cookies. The prefix `__Host-` makes the browser
/// require `Secure` and `Path=/` and refuse `Domain`, so that a site on
/// another subdomain cannot plant a session.
const COOKIE_ATTRIBUTES: &str = "Path=/; Secure; SameSite=Lax";
pub const SIGN_OUT_PATH: &str = "/auth/sign-out";

/// The value of the cookie `name`, as a secret; `None` when the request
/// carries it twice, as nothing tells which one CIHub set.
pub(crate) fn cookie(headers: &HeaderMap, name: &str) -> Option<SecretString> {
    let mut values = headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .filter(|(key, _)| *key == name)
        .map(|(_, value)| value);
    let value = values.next()?;

    values
        .next()
        .is_none()
        .then(|| SecretString::from(value.to_owned()))
}

/// The `Set-Cookie` value of the cookie `name`; every cookie but the CSRF
/// one is hidden from the scripts.
pub(crate) fn set_cookie(name: &str, value: &str, max_age_seconds: i64) -> String {
    let hidden = if name == CSRF_COOKIE {
        ""
    } else {
        "; HttpOnly"
    };
    format!("{name}={value}; {COOKIE_ATTRIBUTES}; Max-Age={max_age_seconds}{hidden}")
}

/// A request that can change a state needs the CSRF token: a page of
/// another site can make the browser send the cookie, not the header.
pub(crate) fn needs_csrf_token(method: &Method) -> bool {
    !matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}

pub(crate) fn has_csrf_token(headers: &HeaderMap, secret: &SecretString) -> bool {
    headers
        .get(CSRF_HEADER)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|sent| is_same_text(sent, &csrf_token(secret)))
}

/// Compares every byte, so that the time of the answer does not tell how
/// much of a guess was right.
pub(crate) fn is_same_text(left: &str, right: &str) -> bool {
    left.len() == right.len()
        && left
            .bytes()
            .zip(right.bytes())
            .fold(0, |differences, (a, b)| differences | (a ^ b))
            == 0
}

pub(crate) fn csrf_refused() -> ApiError {
    ApiError::new(
        ErrorCode::CsrfRefused,
        format!("the header `{CSRF_HEADER}` does not carry the CSRF token of the session"),
    )
}

/// Deletes the session of the request and clears its cookies, and forgets
/// the token of the person when it was their last session. Without a session,
/// it only clears the cookies.
pub(crate) async fn sign_out(
    State(pool): State<PgPool>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    if let Some(secret) = cookie(&headers, SESSION_COOKIE) {
        if !has_csrf_token(&headers, &secret) {
            return Err(csrf_refused());
        }
        delete_session(&pool, &secret).await?;
        forget_tokens_of_the_gone(&pool).await?;
    }

    let mut response = StatusCode::NO_CONTENT.into_response();
    for name in [SESSION_COOKIE, CSRF_COOKIE] {
        let cleared = HeaderValue::from_str(&set_cookie(name, "", 0))?;
        response.headers_mut().append(SET_COOKIE, cleared);
    }
    Ok(response)
}

/// A session of the command line ends after this long without use.
pub const CLI_SESSION_IDLE_LIFETIME: Duration = Duration::days(90);
/// Each use pushes the end of a session of the command line back, at most
/// this often, so that a read does not write each time.
const CLI_SESSION_EXTENSION_PRECISION: Duration = Duration::hours(1);

pub struct NewSession {
    pub id: SessionId,
    pub user_id: UserId,
    pub created_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
}

/// The user that a session token stands for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionGrant {
    pub id: SessionId,
    pub user_id: UserId,
    pub expires_at: OffsetDateTime,
    pub client: SessionClient,
}

/// Who holds a session, which the prefix of its secret tells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionClient {
    /// `chs_…`, in the cookie of the dashboard.
    Dashboard,
    /// `chc_…`, in the `Authorization` header of the command line.
    CommandLine,
}

impl SessionClient {
    /// The client of `secret`; `None` when it is not a session token.
    pub fn of(secret: &SecretString) -> Option<Self> {
        let text = secret.expose_secret();
        if is_session_token(text) {
            return Some(Self::Dashboard);
        }

        is_cli_session_token(text).then_some(Self::CommandLine)
    }
}

/// # Errors
///
/// The system has no random source.
pub fn new_session_secret() -> Result<SecretString, StoreError> {
    Ok(format_session_token(&draw_secret_bytes()?).into())
}

/// # Errors
///
/// The system has no random source.
pub fn new_cli_session_secret() -> Result<SecretString, StoreError> {
    Ok(format_cli_session_token(&draw_secret_bytes()?).into())
}

/// A secret of a sign-in in progress, for its `state` or its PKCE verifier:
/// 32 random bytes in hexadecimal, 64 characters, which RFC 7636 allows.
///
/// # Errors
///
/// The system has no random source.
pub fn new_sign_in_secret() -> Result<SecretString, StoreError> {
    Ok(hexadecimal(&draw_secret_bytes::<32>()?).into())
}

/// The CSRF token of the session of `secret`: the hexadecimal SHA-256 of a
/// label and the secret. A script of another site can neither read the
/// cookie of the session nor compute it, and the server keeps nothing more.
pub fn csrf_token(secret: &SecretString) -> String {
    let digest = Sha256::new()
        .chain_update(b"cihub-csrf:")
        .chain_update(secret.expose_secret().as_bytes())
        .finalize();
    hexadecimal(&digest)
}

fn hexadecimal(bytes: &[u8]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        })
}

/// Stores `session` with the hash of `secret`.
///
/// # Errors
///
/// Postgres refuses the row: an unknown user, or a secret already stored.
pub async fn create_session(
    conn: &mut PgConnection,
    session: &NewSession,
    secret: &SecretString,
) -> Result<(), StoreError> {
    sqlx::query(
        "INSERT INTO sessions (id, user_id, token_hash, created_at, expires_at)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(session.id.to_string())
    .bind(session.user_id.to_string())
    .bind(secret_hash(secret))
    .bind(session.created_at)
    .bind(session.expires_at)
    .execute(conn)
    .await
    .map_err(StoreError::Query)?;
    Ok(())
}

/// The session of `secret` at `now`: `None` when it is not a session token,
/// unknown or expired. A session of the command line ends
/// [`CLI_SESSION_IDLE_LIFETIME`] after its last use.
///
/// # Errors
///
/// Postgres fails, or the row does not match the identifiers of
/// `cihub-domain`.
pub async fn authenticate_session(
    pool: &PgPool,
    secret: &SecretString,
    now: OffsetDateTime,
) -> Result<Option<SessionGrant>, StoreError> {
    let Some(client) = SessionClient::of(secret) else {
        return Ok(None);
    };

    let row: Option<(String, String, OffsetDateTime)> = sqlx::query_as(
        "SELECT id, user_id, expires_at FROM sessions WHERE token_hash = $1 AND expires_at > $2",
    )
    .bind(secret_hash(secret))
    .bind(now)
    .fetch_optional(pool)
    .await
    .map_err(StoreError::Query)?;
    let Some((id, user_id, mut expires_at)) = row else {
        return Ok(None);
    };

    let extended = now + CLI_SESSION_IDLE_LIFETIME;
    if client == SessionClient::CommandLine
        && expires_at <= extended - CLI_SESSION_EXTENSION_PRECISION
    {
        sqlx::query("UPDATE sessions SET expires_at = $2 WHERE id = $1")
            .bind(&id)
            .bind(extended)
            .execute(pool)
            .await
            .map_err(StoreError::Query)?;
        expires_at = extended;
    }

    let grant = SessionGrant {
        id: id.parse().map_err(StoreError::read("session", &id))?,
        user_id: user_id.parse().map_err(StoreError::read("session", &id))?,
        expires_at,
        client,
    };
    Ok(Some(grant))
}

/// Deletes the session of `secret`: a sign-out. Without a session, it does
/// nothing.
///
/// # Errors
///
/// Postgres fails.
pub async fn delete_session(pool: &PgPool, secret: &SecretString) -> Result<(), StoreError> {
    sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
        .bind(secret_hash(secret))
        .execute(pool)
        .await
        .map_err(StoreError::Query)?;
    Ok(())
}

/// Deletes the session `id`: a sign-out of the command line, which knows its
/// session but not the secret of the request any more.
///
/// # Errors
///
/// Postgres fails.
pub async fn delete_session_by_id(pool: &PgPool, id: SessionId) -> Result<(), StoreError> {
    sqlx::query("DELETE FROM sessions WHERE id = $1")
        .bind(id.to_string())
        .execute(pool)
        .await
        .map_err(StoreError::Query)?;
    Ok(())
}

/// Ends every session of `user_id`, of the dashboard and of the command
/// line: the SCM refused the token of the person.
///
/// # Errors
///
/// Postgres fails.
pub async fn delete_sessions_of(pool: &PgPool, user_id: UserId) -> Result<(), StoreError> {
    sqlx::query("DELETE FROM sessions WHERE user_id = $1")
        .bind(user_id.to_string())
        .execute(pool)
        .await
        .map_err(StoreError::Query)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cookie_is_found_among_the_others() {
        let mut headers = HeaderMap::new();
        headers.append(
            COOKIE,
            HeaderValue::from_static("theme=dark; __Host-cihub_session=chs_a"),
        );
        headers.append(COOKIE, HeaderValue::from_static("__Host-cihub_csrf=b"));

        let session = cookie(&headers, SESSION_COOKIE);
        let csrf = cookie(&headers, CSRF_COOKIE);

        assert!(
            session.is_some_and(|secret| secrecy::ExposeSecret::expose_secret(&secret) == "chs_a")
        );
        assert!(csrf.is_some());
        assert!(cookie(&headers, "other").is_none());
    }

    #[test]
    fn a_cookie_sent_twice_is_refused() {
        let mut headers = HeaderMap::new();
        headers.append(
            COOKIE,
            HeaderValue::from_static("__Host-cihub_session=a; __Host-cihub_session=b"),
        );

        assert!(cookie(&headers, SESSION_COOKIE).is_none());
    }

    #[test]
    fn only_the_session_cookie_is_hidden_from_the_scripts() {
        assert_eq!(
            set_cookie(SESSION_COOKIE, "chs_a", 60),
            "__Host-cihub_session=chs_a; Path=/; Secure; SameSite=Lax; Max-Age=60; HttpOnly"
        );
        assert_eq!(
            set_cookie(CSRF_COOKIE, "", 0),
            "__Host-cihub_csrf=; Path=/; Secure; SameSite=Lax; Max-Age=0"
        );
    }

    #[test]
    fn only_the_token_of_the_session_passes() {
        let secret = SecretString::from("chs_secret");
        let mut headers = HeaderMap::new();

        assert!(!has_csrf_token(&headers, &secret));

        headers.insert(CSRF_HEADER, HeaderValue::from_static("guess"));
        assert!(!has_csrf_token(&headers, &secret));

        let token = HeaderValue::from_str(&csrf_token(&secret)).unwrap();
        headers.insert(CSRF_HEADER, token);
        assert!(has_csrf_token(&headers, &secret));
        assert!(!needs_csrf_token(&Method::GET));
        assert!(needs_csrf_token(&Method::PATCH));
    }
}

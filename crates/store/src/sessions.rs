use std::fmt::Write;

use cihub_domain::{
    SessionId, UserId, format_cli_session_token, format_session_token, is_cli_session_token,
    is_session_token,
};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool};
use time::{Duration, OffsetDateTime};

use crate::StoreError;
use crate::tokens::{draw_secret_bytes, hash};

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
    .bind(hash(secret))
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
    .bind(hash(secret))
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
        .bind(hash(secret))
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

use cihub_domain::{SessionId, UserId, format_session_token, is_session_token};
use secrecy::{ExposeSecret, SecretString};
use sqlx::{PgConnection, PgPool};
use time::OffsetDateTime;

use crate::StoreError;
use crate::tokens::{draw_secret_bytes, hash};

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
}

/// # Errors
///
/// The system has no random source.
pub fn new_session_secret() -> Result<SecretString, StoreError> {
    Ok(format_session_token(&draw_secret_bytes()?).into())
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
/// unknown or expired.
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
    if !is_session_token(secret.expose_secret()) {
        return Ok(None);
    }

    let row: Option<(String, String, OffsetDateTime)> = sqlx::query_as(
        "SELECT id, user_id, expires_at FROM sessions WHERE token_hash = $1 AND expires_at > $2",
    )
    .bind(hash(secret))
    .bind(now)
    .fetch_optional(pool)
    .await
    .map_err(StoreError::Query)?;
    let Some((id, user_id, expires_at)) = row else {
        return Ok(None);
    };

    let grant = SessionGrant {
        id: id.parse().map_err(StoreError::read("session", &id))?,
        user_id: user_id.parse().map_err(StoreError::read("session", &id))?,
        expires_at,
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

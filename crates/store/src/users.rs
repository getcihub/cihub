use cihub_domain::UserId;
use sqlx::{PgConnection, PgPool};
use time::OffsetDateTime;

use crate::StoreError;

/// The account `external_id` of the SCM, created with `id` when it is missing.
/// Its login follows a rename: the log points to the account, never to a login.
///
/// # Errors
///
/// Postgres fails, or the row does not match the identifiers of
/// `cihub-domain`.
pub async fn ensure_user(
    conn: &mut PgConnection,
    id: UserId,
    external_id: &str,
    login: &str,
) -> Result<UserId, StoreError> {
    let id: String = sqlx::query_scalar(
        "INSERT INTO users (id, external_id, login) VALUES ($1, $2, $3)
         ON CONFLICT (external_id) DO UPDATE SET
             login = excluded.login,
             updated_at = CASE WHEN users.login = excluded.login THEN users.updated_at ELSE now() END
         RETURNING id",
    )
    .bind(id.to_string())
    .bind(external_id)
    .bind(login)
    .fetch_one(conn)
    .await
    .map_err(StoreError::Query)?;
    id.parse().map_err(StoreError::read("user", &id))
}

/// How a user shows on the dashboard.
#[derive(Clone, Debug, PartialEq, Eq, sqlx::FromRow)]
pub struct UserProfile {
    pub login: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    /// When CIHub last read the repositories that the user reaches.
    pub synced_at: Option<OffsetDateTime>,
}

/// Records the name and the avatar that the SCM gives at the sign-in of
/// `user_id`.
///
/// # Errors
///
/// Postgres fails.
pub async fn record_profile(
    conn: &mut PgConnection,
    user_id: UserId,
    name: Option<&str>,
    avatar_url: Option<&str>,
) -> Result<(), StoreError> {
    sqlx::query(
        "UPDATE users SET name = $2, avatar_url = $3, updated_at = now()
         WHERE id = $1 AND (name, avatar_url) IS DISTINCT FROM ($2, $3)",
    )
    .bind(user_id.to_string())
    .bind(name)
    .bind(avatar_url)
    .execute(conn)
    .await
    .map_err(StoreError::Query)?;
    Ok(())
}

/// Records the avatar that the list of the members of an account gives for
/// `user_id`. The list does not give the name, which stays.
///
/// # Errors
///
/// Postgres fails.
pub async fn record_avatar(
    conn: &mut PgConnection,
    user_id: UserId,
    avatar_url: Option<&str>,
) -> Result<(), StoreError> {
    sqlx::query(
        "UPDATE users SET avatar_url = $2, updated_at = now()
         WHERE id = $1 AND avatar_url IS DISTINCT FROM $2",
    )
    .bind(user_id.to_string())
    .bind(avatar_url)
    .execute(conn)
    .await
    .map_err(StoreError::Query)?;
    Ok(())
}

/// The profile of the user `user_id`, or `None` when no user has it. Users
/// are global: no context of an organization applies.
///
/// # Errors
///
/// Postgres fails.
pub async fn profile_of(pool: &PgPool, user_id: UserId) -> Result<Option<UserProfile>, StoreError> {
    sqlx::query_as("SELECT login, name, avatar_url, synced_at FROM users WHERE id = $1")
        .bind(user_id.to_string())
        .fetch_optional(pool)
        .await
        .map_err(StoreError::Query)
}

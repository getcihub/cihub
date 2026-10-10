use cihub_domain::UserId;
use sqlx::PgConnection;

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
         ON CONFLICT (external_id) DO UPDATE SET login = excluded.login
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

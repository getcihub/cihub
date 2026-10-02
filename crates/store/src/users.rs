use cihub_domain::{ScmInstanceId, UserId};
use sqlx::PgConnection;

use crate::StoreError;

/// The account `external_id` of the SCM instance, created with `id` when it
/// is missing (`DECISIONS.md` D42, D51). Its login follows a rename: the log
/// points to the account, never to a login.
///
/// # Errors
///
/// Postgres fails, or the row does not match the identifiers of
/// `cihub-domain`.
pub async fn ensure_user(
    conn: &mut PgConnection,
    id: UserId,
    scm_instance_id: ScmInstanceId,
    external_id: &str,
    login: &str,
) -> Result<UserId, StoreError> {
    let id: String = sqlx::query_scalar(
        "INSERT INTO users (id, scm_instance_id, external_id, login) VALUES ($1, $2, $3, $4)
         ON CONFLICT (scm_instance_id, external_id) DO UPDATE SET login = excluded.login
         RETURNING id",
    )
    .bind(id.to_string())
    .bind(scm_instance_id.to_string())
    .bind(external_id)
    .bind(login)
    .fetch_one(conn)
    .await
    .map_err(StoreError::Query)?;
    id.parse().map_err(|source| StoreError::ReadUser {
        id,
        source: Box::new(source),
    })
}

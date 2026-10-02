use cihub_domain::{ScmInstanceId, ScmKind};
use sqlx::PgConnection;

use crate::StoreError;

/// The SCM instance at `base_url`, created with `id` when it is missing
/// (`DECISIONS.md` D47, D79). Two processes that start together get the same
/// instance.
///
/// # Errors
///
/// Postgres fails, or the row does not match the identifiers of
/// `cihub-domain`.
pub async fn ensure_scm_instance(
    conn: &mut PgConnection,
    id: ScmInstanceId,
    kind: ScmKind,
    base_url: &str,
) -> Result<ScmInstanceId, StoreError> {
    sqlx::query(
        "INSERT INTO scm_instances (id, kind, base_url) VALUES ($1, $2::scm_kind, $3)
         ON CONFLICT (base_url) DO NOTHING",
    )
    .bind(id.to_string())
    .bind(kind.as_str())
    .bind(base_url)
    .execute(&mut *conn)
    .await
    .map_err(StoreError::Query)?;
    let (id,): (String,) = sqlx::query_as("SELECT id FROM scm_instances WHERE base_url = $1")
        .bind(base_url)
        .fetch_one(conn)
        .await
        .map_err(StoreError::Query)?;
    id.parse().map_err(|source| StoreError::ReadInstance {
        id,
        source: Box::new(source),
    })
}

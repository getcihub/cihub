use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use cihub_domain::{ConnectionId, JobClass, OrgId, RepoId};
use cihub_scm::{ConnectionRef, Scm, ScmError};
use cihub_store::{PgPool, begin_system};
use serde_json::json;
use sqlx::PgConnection;
use time::OffsetDateTime;
use tracing::info;

use crate::jobs::hour_window;
use crate::{Job, Outcome, PlatformError, enqueue_job, error_chain};

pub const SYNC_REPOS: &str = "sync_repos";

pub fn sync_repos_job(org_id: OrgId, connection: ConnectionId, cause: &str) -> Job {
    Job {
        idempotency_key: format!("{SYNC_REPOS}:{connection}-{cause}:1"),
        job_type: SYNC_REPOS.to_owned(),
        class: JobClass::Repair,
        org_id: Some(org_id),
        connection: Some(connection),
        queue_id: None,
        payload: json!({}),
        attempts: 0,
    }
}

pub(crate) async fn request_repo_sync(
    conn: &mut PgConnection,
    org_id: OrgId,
    connection: ConnectionId,
    delivery_id: &str,
    now: OffsetDateTime,
) -> Result<bool, PlatformError> {
    let job = sync_repos_job(org_id, connection, delivery_id);
    enqueue_job(conn, &job, now, now).await
}

/// Requests the hourly read of the repositories of each connection that is
/// not suspended, and returns the number of new requests. Each scheduler
/// does it; the key of the hour admits one job per connection.
///
/// # Errors
///
/// Postgres fails, or a row does not match the identifiers of `cihub-domain`.
pub async fn request_repo_syncs(
    conn: &mut PgConnection,
    now: OffsetDateTime,
) -> Result<usize, PlatformError> {
    let connections: Vec<(String, String)> = sqlx::query_as(
        "SELECT org_id, id FROM scm_connections
         WHERE suspended_at IS NULL ORDER BY id",
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(PlatformError::Query)?;
    let window = hour_window(now);
    let mut requested = 0;
    for (org, connection) in connections {
        let read = || PlatformError::read("connection", &connection);
        let job = sync_repos_job(
            org.parse().map_err(read())?,
            connection.parse().map_err(read())?,
            &window,
        );
        if enqueue_job(conn, &job, now, now).await? {
            requested += 1;
        }
    }
    Ok(requested)
}

/// Reads the repositories of the connection of `job`, creates those that
/// CIHub does not know and updates the others, and returns their number. A
/// repository that the connection no longer reaches stays (`DECISIONS.md`
/// D155); one that another organization has is left to it.
///
/// # Errors
///
/// The job has no connection, or Postgres or the SCM fails.
pub async fn run_sync_repos(
    pool: &PgPool,
    scm: &dyn Scm,
    job: &Job,
) -> Result<usize, PlatformError> {
    let (Some(org_id), Some(connection_id)) = (job.org_id, job.connection) else {
        return Err(PlatformError::JobWithoutConnection(
            job.idempotency_key.clone(),
        ));
    };
    let mut tx = begin_system(pool).await.map_err(PlatformError::Store)?;
    let external_id: Option<String> = sqlx::query_scalar(
        "SELECT external_id FROM scm_connections WHERE id = $1 AND suspended_at IS NULL",
    )
    .bind(connection_id.to_string())
    .fetch_optional(&mut *tx)
    .await
    .map_err(PlatformError::Query)?;
    tx.commit().await.map_err(PlatformError::Query)?;
    let Some(external_id) = external_id else {
        return Ok(0);
    };
    let connection = ConnectionRef {
        id: connection_id,
        external_id,
    };
    let repos = scm
        .list_repos(&connection, job.class)
        .await
        .map_err(PlatformError::Scm)?;
    #[expect(
        clippy::disallowed_methods,
        reason = "a repository new to CIHub gets an identifier; the others keep theirs"
    )]
    let ids: Vec<String> = repos.iter().map(|_| RepoId::new().to_string()).collect();
    let column = |field: fn(&cihub_scm::ScmRepo) -> &String| -> Vec<&String> {
        repos.iter().map(field).collect()
    };
    let mut tx = begin_system(pool).await.map_err(PlatformError::Store)?;
    let written = sqlx::query(
        "INSERT INTO repos (id, org_id, connection_id, external_id, namespace, name,
                            default_branch)
         SELECT r.id, $2, $3, r.external_id, r.namespace, r.name, r.default_branch
         FROM unnest($1::text[], $4::text[], $5::text[], $6::text[], $7::text[])
              AS r (id, external_id, namespace, name, default_branch)
         ON CONFLICT (external_id) DO UPDATE
         SET connection_id = EXCLUDED.connection_id, namespace = EXCLUDED.namespace,
             name = EXCLUDED.name, default_branch = EXCLUDED.default_branch
         WHERE repos.org_id = EXCLUDED.org_id",
    )
    .bind(&ids)
    .bind(org_id.to_string())
    .bind(connection_id.to_string())
    .bind(column(|repo| &repo.external_id))
    .bind(column(|repo| &repo.namespace))
    .bind(column(|repo| &repo.name))
    .bind(column(|repo| &repo.default_branch))
    .execute(&mut *tx)
    .await
    .map_err(PlatformError::Query)?;
    tx.commit().await.map_err(PlatformError::Query)?;
    let written = usize::try_from(written.rows_affected()).unwrap_or(usize::MAX);
    info!(
        org_id = %org_id,
        connection_id = %connection_id,
        repos = written,
        "repositories read"
    );
    Ok(written)
}

pub fn sync_repos_executor(
    scm: Arc<dyn Scm>,
) -> impl Fn(PgPool, Job, OffsetDateTime) -> Pin<Box<dyn Future<Output = Outcome> + Send>>
+ Send
+ Sync
+ 'static {
    move |pool, job, _now| {
        let scm = Arc::clone(&scm);
        Box::pin(async move {
            match run_sync_repos(&pool, scm.as_ref(), &job).await {
                Ok(_) => Outcome::Done,
                Err(err @ PlatformError::Scm(ScmError::AccessLost(_) | ScmError::NotFound(_))) => {
                    Outcome::Failed(error_chain(&err))
                }
                Err(err) => Outcome::Retry(error_chain(&err)),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use ulid::Ulid;

    use super::*;

    #[test]
    fn a_read_of_repositories_has_one_job_per_hour_or_per_delivery() {
        let at = |hour, minute| {
            OffsetDateTime::from_unix_timestamp(1_789_000_000)
                .unwrap()
                .replace_hour(hour)
                .unwrap()
                .replace_minute(minute)
                .unwrap()
        };
        let (org, connection) = (OrgId::from_ulid(Ulid(1)), ConnectionId::from_ulid(Ulid(2)));
        let key = |cause: &str| sync_repos_job(org, connection, cause).idempotency_key;
        assert_eq!(key(&hour_window(at(9, 1))), key(&hour_window(at(9, 59))));
        assert_ne!(key(&hour_window(at(9, 59))), key(&hour_window(at(10, 0))));
        assert_eq!(key("d-1"), format!("sync_repos:{connection}-d-1:1"));
        assert_eq!(
            key(&hour_window(at(9, 7))),
            format!("sync_repos:{connection}-20260910T09:1")
        );
    }
}

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use cihub_domain::{ConnectionId, EventKind, JobClass, OrgId, RepoId};
use cihub_scm::{ConnectionRef, Scm, ScmError};
use cihub_store::{PgPool, begin_system};
use serde_json::json;
use sqlx::PgConnection;
use time::OffsetDateTime;
use tracing::info;

use crate::installations::{give_to_repo_queues, request_installation_sync};
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
    request_for_each_connection(conn, now, sync_repos_job).await
}

/// Requests the job that `job` makes, for the hour of `now`, for each
/// connection that is not suspended, and returns the number of new requests.
pub(crate) async fn request_for_each_connection(
    conn: &mut PgConnection,
    now: OffsetDateTime,
    job: fn(OrgId, ConnectionId, &str) -> Job,
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
        let job = job(
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
/// repository that the connection no longer reaches keeps its row and its
/// history with `removed_at`, and its queues lose their access until it comes
/// back; one that another organization has is left to it. When the SCM
/// answers that the access is lost or the installation is not found, requests
/// the sync of the installation.
///
/// # Errors
///
/// The job has no connection, or Postgres or the SCM fails.
pub async fn run_sync_repos(
    pool: &PgPool,
    scm: &dyn Scm,
    job: &Job,
    now: OffsetDateTime,
) -> Result<usize, PlatformError> {
    let (Some(org_id), Some(connection_id)) = (job.org_id, job.connection) else {
        return Err(PlatformError::JobWithoutConnection(
            job.idempotency_key.clone(),
        ));
    };

    let Some(connection) = active_connection(pool, connection_id).await? else {
        return Ok(0);
    };

    let listed = scm.list_repos(&connection, job.class).await;

    sync_installation_after(pool, &connection, &listed, now).await?;
    let repos = listed.map_err(PlatformError::Scm)?;

    #[expect(
        clippy::disallowed_methods,
        reason = "a repository new to CIHub gets an identifier; the others keep theirs"
    )]
    let ids: Vec<String> = repos.iter().map(|_| RepoId::new().to_string()).collect();
    let column = |field: fn(&cihub_scm::ScmRepo) -> &String| -> Vec<&String> {
        repos.iter().map(field).collect()
    };
    let mut tx = begin_system(pool).await.map_err(PlatformError::Store)?;
    let restored: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM repos
         WHERE connection_id = $1 AND removed_at IS NOT NULL AND external_id = ANY($2)",
    )
    .bind(connection_id.to_string())
    .bind(column(|repo| &repo.external_id))
    .fetch_all(&mut *tx)
    .await
    .map_err(PlatformError::Query)?;
    let written = sqlx::query(
        "INSERT INTO repos (id, org_id, connection_id, external_id, namespace, name,
                            default_branch)
         SELECT r.id, $2, $3, r.external_id, r.namespace, r.name, r.default_branch
         FROM unnest($1::text[], $4::text[], $5::text[], $6::text[], $7::text[])
              AS r (id, external_id, namespace, name, default_branch)
         ON CONFLICT (connection_id, external_id) DO UPDATE
         SET namespace = EXCLUDED.namespace, name = EXCLUDED.name,
             default_branch = EXCLUDED.default_branch, removed_at = NULL",
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
    let removed: Vec<String> = sqlx::query_scalar(
        "UPDATE repos SET removed_at = $3
         WHERE connection_id = $1 AND removed_at IS NULL AND external_id <> ALL($2)
         RETURNING id",
    )
    .bind(connection_id.to_string())
    .bind(column(|repo| &repo.external_id))
    .bind(now)
    .fetch_all(&mut *tx)
    .await
    .map_err(PlatformError::Query)?;

    give_to_repo_queues(&mut tx, org_id, &removed, &EventKind::AccessLost, now).await?;
    give_to_repo_queues(&mut tx, org_id, &restored, &EventKind::AccessRestored, now).await?;
    tx.commit().await.map_err(PlatformError::Query)?;
    let written = usize::try_from(written.rows_affected()).unwrap_or(usize::MAX);
    info!(
        org_id = %org_id,
        connection_id = %connection_id,
        repos = written,
        removed = removed.len(),
        restored = restored.len(),
        "repositories read"
    );

    Ok(written)
}

/// The connection `connection_id`, or `None` when it is suspended.
pub(crate) async fn active_connection(
    pool: &PgPool,
    connection_id: ConnectionId,
) -> Result<Option<ConnectionRef>, PlatformError> {
    let mut tx = begin_system(pool).await.map_err(PlatformError::Store)?;
    let external_id: Option<String> = sqlx::query_scalar(
        "SELECT external_id FROM scm_connections WHERE id = $1 AND suspended_at IS NULL",
    )
    .bind(connection_id.to_string())
    .fetch_optional(&mut *tx)
    .await
    .map_err(PlatformError::Query)?;

    tx.commit().await.map_err(PlatformError::Query)?;
    Ok(external_id.map(|external_id| ConnectionRef {
        id: connection_id,
        external_id,
    }))
}

/// When the SCM answered that the access is lost or the installation is not
/// found, requests the sync of the installation, which records it.
pub(crate) async fn sync_installation_after<T>(
    pool: &PgPool,
    connection: &ConnectionRef,
    answer: &Result<T, ScmError>,
    now: OffsetDateTime,
) -> Result<(), PlatformError> {
    if !matches!(answer, Err(ScmError::AccessLost(_) | ScmError::NotFound(_))) {
        return Ok(());
    }

    let mut tx = begin_system(pool).await.map_err(PlatformError::Store)?;
    let cause = hour_window(now);

    request_installation_sync(
        &mut tx,
        &connection.external_id,
        &cause,
        JobClass::Repair,
        now,
    )
    .await?;
    tx.commit().await.map_err(PlatformError::Query)?;
    Ok(())
}

pub fn sync_repos_executor(
    scm: Arc<dyn Scm>,
) -> impl Fn(PgPool, Job, OffsetDateTime) -> Pin<Box<dyn Future<Output = Outcome> + Send>>
+ Send
+ Sync
+ 'static {
    move |pool, job, now| {
        let scm = Arc::clone(&scm);
        Box::pin(async move { outcome_of(run_sync_repos(&pool, scm.as_ref(), &job, now).await) })
    }
}

/// The outcome of a read of a connection: a lost access or a missing
/// installation does not come back by itself, and the sync of the
/// installation records it.
pub(crate) fn outcome_of<T>(result: Result<T, PlatformError>) -> Outcome {
    match result {
        Ok(_) => Outcome::Done,
        Err(err @ PlatformError::Scm(ScmError::AccessLost(_) | ScmError::NotFound(_))) => {
            Outcome::Failed(error_chain(&err))
        }
        Err(err) => Outcome::Retry(error_chain(&err)),
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

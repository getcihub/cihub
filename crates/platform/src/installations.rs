use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use cihub_domain::{
    Actor, ConnectionId, ConnectionKind, Event, EventId, EventKind, JobClass, OrgId, QueueId,
};
use cihub_scm::{Scm, ScmInstallation};
use cihub_store::{PgPool, begin_system, deposit_fact};
use serde_json::json;
use sqlx::PgConnection;
use time::{Duration, OffsetDateTime};
use tracing::{info, warn};

use crate::connections::sync_repos_job;
use crate::jobs::hour_window;
use crate::{Job, Outcome, PlatformError, enqueue_job, error_chain, mark_dirty};

pub const SYNC_INSTALLATION: &str = "sync_installation";

pub const SYNC_INSTALLATIONS: &str = "sync_installations";

pub(crate) const SYNC_AFTER: Duration = Duration::hours(1);

pub fn sync_installation_job(external_id: &str, cause: &str, class: JobClass) -> Job {
    Job {
        idempotency_key: format!("{SYNC_INSTALLATION}:{external_id}-{cause}:1"),
        job_type: SYNC_INSTALLATION.to_owned(),
        class,
        org_id: None,
        connection: None,
        queue_id: None,
        payload: json!({ "installation": external_id }),
        attempts: 0,
    }
}

fn sync_installations_job(cause: &str) -> Job {
    Job {
        idempotency_key: format!("{SYNC_INSTALLATIONS}:{cause}:1"),
        job_type: SYNC_INSTALLATIONS.to_owned(),
        class: JobClass::Repair,
        org_id: None,
        connection: None,
        queue_id: None,
        payload: json!({}),
        attempts: 0,
    }
}

pub(crate) async fn request_installation_sync(
    conn: &mut PgConnection,
    external_id: &str,
    cause: &str,
    class: JobClass,
    now: OffsetDateTime,
) -> Result<bool, PlatformError> {
    let job = sync_installation_job(external_id, cause, class);
    enqueue_job(conn, &job, now, now).await
}

/// Requests the full sync of the day of `now`. A scheduler calls it every
/// hour: only the first request of a day creates the job.
///
/// # Errors
///
/// Postgres fails.
pub async fn request_installations_sync(
    conn: &mut PgConnection,
    now: OffsetDateTime,
) -> Result<bool, PlatformError> {
    let day = format!(
        "{:04}{:02}{:02}",
        now.year(),
        u8::from(now.month()),
        now.day()
    );
    enqueue_job(conn, &sync_installations_job(&day), now, now).await
}

/// Requests the full sync of a server that starts at `now`.
///
/// # Errors
///
/// Postgres fails.
pub async fn request_installations_sync_at_start(
    conn: &mut PgConnection,
    now: OffsetDateTime,
) -> Result<bool, PlatformError> {
    let instant = format!("{}{:02}{:02}", hour_window(now), now.minute(), now.second());
    enqueue_job(conn, &sync_installations_job(&instant), now, now).await
}

/// Reads the installation of `job` on the SCM and converges its connection.
///
/// # Errors
///
/// The job names no installation, the SCM or Postgres fails, or another
/// organization has the slug of a new installation.
pub async fn run_sync_installation(
    pool: &PgPool,
    scm: &dyn Scm,
    kind: ConnectionKind,
    job: &Job,
    now: OffsetDateTime,
) -> Result<(), PlatformError> {
    let Some(external_id) = job.payload["installation"].as_str() else {
        return Err(PlatformError::JobWithoutInstallation(
            job.idempotency_key.clone(),
        ));
    };

    let installation = scm
        .installation(external_id, job.class)
        .await
        .map_err(PlatformError::Scm)?;

    converge_installation(pool, kind, external_id, installation.as_ref(), now).await
}

/// Reads every installation of the App and converges each connection; a
/// connection whose installation the SCM no longer lists is suspended.
/// A slug already taken does not stop the others.
///
/// # Errors
///
/// The SCM or Postgres fails, or another organization has the slug of a new
/// installation.
pub async fn run_sync_installations(
    pool: &PgPool,
    scm: &dyn Scm,
    kind: ConnectionKind,
    now: OffsetDateTime,
) -> Result<(), PlatformError> {
    let installations = scm
        .installations(JobClass::Repair)
        .await
        .map_err(PlatformError::Scm)?;

    let mut slug_taken = None;

    for installation in &installations {
        let synced = converge_installation(
            pool,
            kind,
            &installation.external_id,
            Some(installation),
            now,
        )
        .await;
        keep_slug_taken(synced, &mut slug_taken)?;
    }

    let listed: Vec<&String> = installations
        .iter()
        .map(|installation| &installation.external_id)
        .collect();

    for external_id in unlisted_connections(pool, kind, &listed).await? {
        converge_installation(pool, kind, &external_id, None, now).await?;
    }

    slug_taken.map_or(Ok(()), Err)
}

/// Connects, as a sync would, each installation of `installations` that
/// CIHub does not know yet, and returns how many organizations it created. A
/// slug already taken does not stop the others: the full sync tries again.
///
/// # Errors
///
/// Postgres fails.
pub(crate) async fn connect_installations(
    pool: &PgPool,
    kind: ConnectionKind,
    installations: &[ScmInstallation],
    now: OffsetDateTime,
) -> Result<usize, PlatformError> {
    let listed: Vec<&str> = installations
        .iter()
        .map(|installation| installation.external_id.as_str())
        .collect();
    let mut tx = begin_system(pool).await.map_err(PlatformError::Store)?;
    let known: Vec<String> = sqlx::query_scalar(
        "SELECT external_id FROM scm_connections
         WHERE kind = $1::connection_kind AND external_id = ANY($2)",
    )
    .bind(kind.as_str())
    .bind(&listed)
    .fetch_all(&mut *tx)
    .await
    .map_err(PlatformError::Query)?;
    tx.commit().await.map_err(PlatformError::Query)?;

    let mut connected = 0;
    for installation in installations {
        if known.contains(&installation.external_id) || installation.is_suspended {
            continue;
        }
        let synced = converge_installation(
            pool,
            kind,
            &installation.external_id,
            Some(installation),
            now,
        )
        .await;
        if synced.is_ok() {
            connected += 1;
        }
        keep_slug_taken(synced, &mut None)?;
    }

    Ok(connected)
}

fn keep_slug_taken(
    synced: Result<(), PlatformError>,
    slug_taken: &mut Option<PlatformError>,
) -> Result<(), PlatformError> {
    match synced {
        Err(err @ PlatformError::SlugTaken { .. }) => {
            warn!(
                error = &err as &dyn std::error::Error,
                "installation not synced"
            );
            slug_taken.get_or_insert(err);
            Ok(())
        }
        other => other,
    }
}

async fn unlisted_connections(
    pool: &PgPool,
    kind: ConnectionKind,
    listed: &[&String],
) -> Result<Vec<String>, PlatformError> {
    let mut tx = begin_system(pool).await.map_err(PlatformError::Store)?;
    let unlisted = sqlx::query_scalar(
        "SELECT external_id FROM scm_connections
         WHERE kind = $1::connection_kind AND external_id <> ALL($2)
         ORDER BY external_id",
    )
    .bind(kind.as_str())
    .bind(listed)
    .fetch_all(&mut *tx)
    .await
    .map_err(PlatformError::Query)?;
    tx.commit().await.map_err(PlatformError::Query)?;
    Ok(unlisted)
}

struct KnownConnection {
    org_id: OrgId,
    id: ConnectionId,
    is_suspended: bool,
}

async fn converge_installation(
    pool: &PgPool,
    kind: ConnectionKind,
    external_id: &str,
    installation: Option<&ScmInstallation>,
    now: OffsetDateTime,
) -> Result<(), PlatformError> {
    let mut tx = begin_system(pool).await.map_err(PlatformError::Store)?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("{SYNC_INSTALLATION}:{external_id}"))
        .execute(&mut *tx)
        .await
        .map_err(PlatformError::Query)?;

    let connection = known_connection(&mut tx, kind, external_id).await?;
    let is_active = installation.is_some_and(|installation| !installation.is_suspended);

    match (connection, installation) {
        (Some(connection), _) => {
            update_connection(&mut tx, &connection, installation, now).await?;
        }
        (None, Some(installation)) if is_active => {
            onboard(&mut tx, kind, installation, now).await?;
        }
        (None, _) => {}
    }

    tx.commit().await.map_err(PlatformError::Query)
}

async fn known_connection(
    conn: &mut PgConnection,
    kind: ConnectionKind,
    external_id: &str,
) -> Result<Option<KnownConnection>, PlatformError> {
    let row: Option<(String, String, bool)> = sqlx::query_as(
        "SELECT org_id, id, suspended_at IS NOT NULL FROM scm_connections
         WHERE kind = $1::connection_kind AND external_id = $2",
    )
    .bind(kind.as_str())
    .bind(external_id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(PlatformError::Query)?;
    let Some((org_id, id, is_suspended)) = row else {
        return Ok(None);
    };

    Ok(Some(KnownConnection {
        org_id: org_id
            .parse()
            .map_err(PlatformError::read("connection", &id))?,
        id: id.parse().map_err(PlatformError::read("connection", &id))?,
        is_suspended,
    }))
}

async fn onboard(
    conn: &mut PgConnection,
    kind: ConnectionKind,
    installation: &ScmInstallation,
    now: OffsetDateTime,
) -> Result<(), PlatformError> {
    let slug = &installation.account.path;
    let is_slug_taken: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM orgs WHERE slug = $1)")
            .bind(slug)
            .fetch_one(&mut *conn)
            .await
            .map_err(PlatformError::Query)?;

    if is_slug_taken {
        return Err(PlatformError::SlugTaken {
            slug: slug.clone(),
            installation: installation.external_id.clone(),
        });
    }

    let (org_id, connection_id) = create_organization(conn, kind, installation, now).await?;
    let job = sync_repos_job(org_id, connection_id, &hour_window(now));

    enqueue_job(conn, &job, now, now).await?;

    info!(
        org_id = %org_id,
        connection_id = %connection_id,
        installation = installation.external_id,
        "organization created"
    );

    Ok(())
}

async fn create_organization(
    conn: &mut PgConnection,
    kind: ConnectionKind,
    installation: &ScmInstallation,
    now: OffsetDateTime,
) -> Result<(OrgId, ConnectionId), PlatformError> {
    #[expect(
        clippy::disallowed_methods,
        reason = "a new installation gets an organization and a connection"
    )]
    let (org_id, connection_id) = (OrgId::new(), ConnectionId::new());

    sqlx::query("INSERT INTO orgs (id, slug, created_at) VALUES ($1, $2, $3)")
        .bind(org_id.to_string())
        .bind(&installation.account.path)
        .bind(now)
        .execute(&mut *conn)
        .await
        .map_err(PlatformError::Query)?;
    sqlx::query(
        "INSERT INTO scm_connections (id, org_id, account_external_id, kind, external_id,
                                      synced_at, permissions, created_at)
         VALUES ($1, $2, $3, $4::connection_kind, $5, $6, $7, $6)",
    )
    .bind(connection_id.to_string())
    .bind(org_id.to_string())
    .bind(&installation.account.external_id)
    .bind(kind.as_str())
    .bind(&installation.external_id)
    .bind(now)
    .bind(json!(installation.permissions))
    .execute(&mut *conn)
    .await
    .map_err(PlatformError::Query)?;

    Ok((org_id, connection_id))
}

async fn update_connection(
    conn: &mut PgConnection,
    connection: &KnownConnection,
    installation: Option<&ScmInstallation>,
    now: OffsetDateTime,
) -> Result<(), PlatformError> {
    sqlx::query(
        "UPDATE scm_connections
         SET synced_at = $2, account_external_id = coalesce($3, account_external_id),
             permissions = coalesce($4, permissions)
         WHERE id = $1",
    )
    .bind(connection.id.to_string())
    .bind(now)
    .bind(installation.map(|installation| &installation.account.external_id))
    .bind(installation.map(|installation| json!(installation.permissions)))
    .execute(&mut *conn)
    .await
    .map_err(PlatformError::Query)?;

    let is_suspended = installation.is_none_or(|installation| installation.is_suspended);
    if is_suspended == connection.is_suspended {
        return Ok(());
    }

    let changed = sqlx::query(
        "UPDATE scm_connections SET suspended_at = CASE WHEN $2 THEN $3::timestamptz END
         WHERE id = $1 AND (suspended_at IS NOT NULL) <> $2",
    )
    .bind(connection.id.to_string())
    .bind(is_suspended)
    .bind(now)
    .execute(&mut *conn)
    .await
    .map_err(PlatformError::Query)?;

    if changed.rows_affected() != 1 {
        return Ok(());
    }

    let kind = if is_suspended {
        EventKind::AccessLost
    } else {
        EventKind::AccessRestored
    };

    info!(
        org_id = %connection.org_id,
        connection_id = %connection.id,
        suspended = is_suspended,
        "connection access changed"
    );

    give_to_queues(conn, connection, &kind, now).await
}

/// Deposits the fact `kind` in the inbox of each queue of `connection`, and
/// marks them dirty. A queue of a repository that the connection no longer
/// reaches keeps its lost access when the connection comes back.
async fn give_to_queues(
    conn: &mut PgConnection,
    connection: &KnownConnection,
    kind: &EventKind,
    now: OffsetDateTime,
) -> Result<(), PlatformError> {
    let queues: Vec<String> = sqlx::query_scalar(
        "SELECT q.id FROM queues q JOIN repos r ON r.id = q.repo_id
         WHERE r.connection_id = $1 AND ($2 OR r.removed_at IS NULL) ORDER BY q.id",
    )
    .bind(connection.id.to_string())
    .bind(*kind == EventKind::AccessLost)
    .fetch_all(&mut *conn)
    .await
    .map_err(PlatformError::Query)?;

    give_to(conn, connection.org_id, queues, kind, now).await
}

/// Deposits the fact `kind` in the inbox of each queue of the repositories
/// `repos` of `org_id`, and marks them dirty.
pub(crate) async fn give_to_repo_queues(
    conn: &mut PgConnection,
    org_id: OrgId,
    repos: &[String],
    kind: &EventKind,
    now: OffsetDateTime,
) -> Result<(), PlatformError> {
    let queues: Vec<String> =
        sqlx::query_scalar("SELECT id FROM queues WHERE repo_id = ANY($1) ORDER BY id")
            .bind(repos)
            .fetch_all(&mut *conn)
            .await
            .map_err(PlatformError::Query)?;

    give_to(conn, org_id, queues, kind, now).await
}

async fn give_to(
    conn: &mut PgConnection,
    org_id: OrgId,
    queues: Vec<String>,
    kind: &EventKind,
    now: OffsetDateTime,
) -> Result<(), PlatformError> {
    for queue in queues {
        let queue_id: QueueId = queue
            .parse()
            .map_err(PlatformError::read("queue", &queue))?;
        #[expect(
            clippy::disallowed_methods,
            reason = "a fact that a sync gives has a new identifier"
        )]
        let id = EventId::new();
        let fact = Event {
            id,
            org_id,
            queue_id,
            actor: Actor::System,
            occurred_at: now,
            cause_id: None,
            kind: kind.clone(),
        };

        deposit_fact(conn, &fact, now)
            .await
            .map_err(PlatformError::Store)?;

        mark_dirty(conn, queue_id).await?;
    }
    Ok(())
}

fn outcome(synced: Result<(), PlatformError>) -> Outcome {
    match synced {
        Ok(()) => Outcome::Done,
        Err(err @ (PlatformError::SlugTaken { .. } | PlatformError::JobWithoutInstallation(_))) => {
            Outcome::Failed(error_chain(&err))
        }
        Err(err) => Outcome::Retry(error_chain(&err)),
    }
}

pub fn sync_installation_executor(
    scm: Arc<dyn Scm>,
    kind: ConnectionKind,
) -> impl Fn(PgPool, Job, OffsetDateTime) -> Pin<Box<dyn Future<Output = Outcome> + Send>>
+ Send
+ Sync
+ 'static {
    move |pool, job, now| {
        let scm = Arc::clone(&scm);
        Box::pin(async move {
            outcome(run_sync_installation(&pool, scm.as_ref(), kind, &job, now).await)
        })
    }
}

pub fn sync_installations_executor(
    scm: Arc<dyn Scm>,
    kind: ConnectionKind,
) -> impl Fn(PgPool, Job, OffsetDateTime) -> Pin<Box<dyn Future<Output = Outcome> + Send>>
+ Send
+ Sync
+ 'static {
    move |pool, _job, now| {
        let scm = Arc::clone(&scm);
        Box::pin(
            async move { outcome(run_sync_installations(&pool, scm.as_ref(), kind, now).await) },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sync_names_its_installation_and_its_cause() {
        let now = OffsetDateTime::from_unix_timestamp(1_789_000_000).unwrap();
        let job = sync_installation_job("7", "d-1", JobClass::Interactive);

        assert_eq!(job.idempotency_key, "sync_installation:7-d-1:1");
        assert_eq!(job.payload, json!({ "installation": "7" }));
        assert_eq!(
            sync_installation_job("7", &hour_window(now), JobClass::Repair).idempotency_key,
            "sync_installation:7-20260910T00:1"
        );
    }
}

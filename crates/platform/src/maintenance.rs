use cihub_domain::JobClass;
use cihub_store::{begin_system, create_event_partitions};
use serde_json::json;
use sqlx::{PgConnection, PgPool};
use time::{Date, Duration, OffsetDateTime};
use tracing::info;

use crate::{Job, Outcome, PlatformError, enqueue_job, error_chain};

/// The type of the daily job that keeps the shared tables in shape
/// (`AGENTS.md` D67).
pub const MAINTENANCE: &str = "maintenance";

/// A processed delivery is deleted after this time.
pub const DELIVERY_RETENTION: Duration = Duration::days(7);

/// A done job is deleted after this time.
pub const DONE_JOB_RETENTION: Duration = Duration::days(3);

/// The maintenance job of `day`: its key admits one job per day, whatever the
/// number of schedulers that request it.
pub fn maintenance_job(day: Date) -> Job {
    Job {
        idempotency_key: format!("{MAINTENANCE}:{day}:1"),
        job_type: MAINTENANCE.to_owned(),
        class: JobClass::Repair,
        org_id: None,
        connection: None,
        queue_id: None,
        payload: json!({}),
        attempts: 0,
    }
}

/// Requests the maintenance job of the day of `now`. A scheduler calls it
/// every hour: only the first request of a day creates the job.
///
/// # Errors
///
/// Postgres fails.
pub async fn request_maintenance(
    conn: &mut PgConnection,
    now: OffsetDateTime,
) -> Result<bool, PlatformError> {
    enqueue_job(conn, &maintenance_job(now.date()), now, now).await
}

/// Creates the partitions of `events` for the month of `now` and the next,
/// then deletes the processed deliveries and the done jobs past their
/// retention. Each step can run twice without harm.
///
/// # Errors
///
/// Postgres fails.
pub async fn run_maintenance(pool: &PgPool, now: OffsetDateTime) -> Result<(), PlatformError> {
    create_event_partitions(pool, now.date())
        .await
        .map_err(PlatformError::Store)?;
    let mut tx = begin_system(pool).await.map_err(PlatformError::Store)?;
    let deliveries = sqlx::query("DELETE FROM webhook_inbox WHERE processed_at < $1")
        .bind(now - DELIVERY_RETENTION)
        .execute(&mut *tx)
        .await
        .map_err(PlatformError::Query)?
        .rows_affected();
    let jobs = sqlx::query("DELETE FROM jobs WHERE state = 'done' AND finished_at < $1")
        .bind(now - DONE_JOB_RETENTION)
        .execute(&mut *tx)
        .await
        .map_err(PlatformError::Query)?
        .rows_affected();
    tx.commit().await.map_err(PlatformError::Query)?;
    info!(deliveries, jobs, "maintenance done");
    Ok(())
}

/// The executor of [`MAINTENANCE`]: a failure is retried after the backoff.
pub async fn execute_maintenance(pool: PgPool, _job: Job, now: OffsetDateTime) -> Outcome {
    match run_maintenance(&pool, now).await {
        Ok(()) => Outcome::Done,
        Err(err) => Outcome::Retry(error_chain(&err)),
    }
}

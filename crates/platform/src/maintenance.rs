use cihub_domain::JobClass;
use cihub_store::{begin_system, create_event_partitions, next_month};
use serde_json::json;
use sqlx::{AssertSqlSafe, PgConnection, PgPool};
use time::{Date, Duration, Month, OffsetDateTime, Time};
use tracing::info;

use crate::{Job, Outcome, PlatformError, enqueue_job, error_chain};

pub const MAINTENANCE: &str = "maintenance";

pub(crate) const DELIVERY_RETENTION: Duration = Duration::days(7);

pub(crate) const DONE_JOB_RETENTION: Duration = Duration::days(3);

pub(crate) const EVENT_RETENTION: Duration = Duration::days(90);

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
/// then deletes the processed deliveries, the deliveries that stayed
/// unrouted and the done jobs past their retention, and drops the old partitions of `events` that snapshots cover.
/// Each step can run twice without harm.
///
/// # Errors
///
/// Postgres fails.
pub async fn run_maintenance(pool: &PgPool, now: OffsetDateTime) -> Result<(), PlatformError> {
    create_event_partitions(pool, now.date())
        .await
        .map_err(PlatformError::Store)?;
    let mut tx = begin_system(pool).await.map_err(PlatformError::Store)?;
    let deliveries = sqlx::query(
        "DELETE FROM webhook_inbox
         WHERE processed_at < $1 OR (routed_at IS NULL AND received_at < $1)",
    )
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
    let (dropped, kept) = drop_old_event_partitions(&mut tx, now).await?;
    tx.commit().await.map_err(PlatformError::Query)?;
    for partition in &kept {
        info!(partition, "event partition kept: events without a snapshot");
    }
    info!(
        deliveries,
        jobs,
        partitions = dropped.len(),
        "maintenance done"
    );
    Ok(())
}

fn partition_end(name: &str) -> Option<OffsetDateTime> {
    let (year, month) = name.strip_prefix("events_")?.split_once('_')?;
    let (year, month): (i32, u8) = (year.parse().ok()?, month.parse().ok()?);
    if name != format!("events_{year:04}_{month:02}") {
        return None;
    }
    let first_day = Date::from_calendar_date(year, Month::try_from(month).ok()?, 1).ok()?;
    Some(next_month(first_day).with_time(Time::MIDNIGHT).assume_utc())
}

async fn drop_old_event_partitions(
    conn: &mut PgConnection,
    now: OffsetDateTime,
) -> Result<(Vec<String>, Vec<String>), PlatformError> {
    let partitions: Vec<String> = sqlx::query_scalar(
        "SELECT c.relname::text FROM pg_inherits i
         JOIN pg_class c ON c.oid = i.inhrelid
         JOIN pg_class p ON p.oid = i.inhparent
         WHERE p.relname = 'events'
         ORDER BY 1",
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(PlatformError::Query)?;
    let (mut dropped, mut kept) = (Vec::new(), Vec::new());
    for name in partitions {
        if partition_end(&name).is_none_or(|end| end > now - EVENT_RETENTION) {
            continue;
        }
        let uncovered: bool = sqlx::query_scalar(AssertSqlSafe(format!(
            "SELECT EXISTS (
                 SELECT 1 FROM {name} e
                 WHERE e.seq > coalesce(
                     (SELECT max(s.seq) FROM queue_snapshots s WHERE s.queue_id = e.queue_id), 0))"
        )))
        .fetch_one(&mut *conn)
        .await
        .map_err(PlatformError::Query)?;
        if uncovered {
            kept.push(name);
            continue;
        }
        sqlx::query(AssertSqlSafe(format!("DROP TABLE {name}")))
            .execute(&mut *conn)
            .await
            .map_err(PlatformError::Query)?;
        dropped.push(name);
    }
    Ok((dropped, kept))
}

pub async fn execute_maintenance(pool: PgPool, _job: Job, now: OffsetDateTime) -> Outcome {
    match run_maintenance(&pool, now).await {
        Ok(()) => Outcome::Done,
        Err(err) => Outcome::Retry(error_chain(&err)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_partition_ends_with_its_month() {
        let end = |name| partition_end(name).map(|at| at.date().to_string());
        assert_eq!(end("events_2026_05"), Some("2026-06-01".into()));
        assert_eq!(end("events_2026_12"), Some("2027-01-01".into()));
        assert_eq!(end("events_2026_13"), None);
        assert_eq!(end("events_2026_5"), None);
        assert_eq!(end("events_x"), None);
        assert_eq!(end("queue_items"), None);
    }
}

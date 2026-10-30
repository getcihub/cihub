use cihub_domain::JobClass;
use cihub_store::{begin_system, create_event_partitions, next_month};
use serde_json::json;
use sqlx::{AssertSqlSafe, PgConnection, PgPool};
use time::{Date, Duration, Month, OffsetDateTime, Time};
use tracing::info;

use crate::{
    Job, Outcome, PlatformError, enqueue_job, error_chain, forget_permissions_before,
    forget_tokens_without_session,
};

pub const MAINTENANCE: &str = "maintenance";

pub(crate) const ENDED_JOB_RETENTION: Duration = Duration::days(3);

pub(crate) const EVENT_RETENTION: Duration = Duration::days(90);

/// A permission outlives the sign-out so that the next sign-in shows the
/// repositories at once; past the life of a session, a person who comes back
/// may have lost them on the SCM, and waits for their sync instead.
pub(crate) const PERMISSION_RETENTION: Duration = Duration::days(14);

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
/// then deletes the ended jobs past their retention, the expired sessions and device codes, the tokens of the people without a session, and drops the old partitions of `events` that snapshots cover.
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
    let jobs = sqlx::query(
        "DELETE FROM jobs WHERE state IN ('done', 'failed', 'dead') AND finished_at < $1",
    )
    .bind(now - ENDED_JOB_RETENTION)
    .execute(&mut *tx)
    .await
    .map_err(PlatformError::Query)?
    .rows_affected();
    let sessions = sqlx::query("DELETE FROM sessions WHERE expires_at <= $1")
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(PlatformError::Query)?
        .rows_affected();
    let device_codes = sqlx::query("DELETE FROM device_codes WHERE expires_at <= $1")
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(PlatformError::Query)?
        .rows_affected();
    let tokens = forget_tokens_without_session(&mut tx)
        .await
        .map_err(PlatformError::Store)?;
    let permissions = forget_permissions_before(&mut tx, now - PERMISSION_RETENTION)
        .await
        .map_err(PlatformError::Store)?;
    let (dropped, kept) = drop_old_event_partitions(&mut tx, now).await?;

    tx.commit().await.map_err(PlatformError::Query)?;
    for partition in &kept {
        info!(
            partition,
            reason = "events_without_snapshot",
            "event_partition.kept"
        );
    }

    info!(
        jobs,
        sessions,
        device_codes,
        tokens,
        permissions,
        partitions = dropped.len(),
        "maintenance.done"
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
                     (SELECT q.snapshot_seq FROM queues q WHERE q.id = e.queue_id), 0))"
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

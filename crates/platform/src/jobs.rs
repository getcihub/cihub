use cihub_domain::{ConnectionId, JobClass, JobState, OrgId, QueueId, ScmInstanceId};
use serde_json::Value;
use sqlx::PgConnection;
use time::{Duration, OffsetDateTime};

use crate::PlatformError;
use crate::wake::notify;

pub(crate) const JOB_CHANNEL: &str = "cihub_jobs";

pub const JOB_LOCK: Duration = Duration::seconds(60);

pub const JOBS_PER_CONNECTION: i64 = 100;

pub const MAX_ATTEMPTS: i32 = 11;

/// The wait before the next attempt: 2, 4, 8… seconds, at most 5 minutes.
pub fn backoff(attempts: i32) -> Duration {
    let exponent = attempts.clamp(1, 9).unsigned_abs();
    Duration::seconds(2_i64.pow(exponent)).min(Duration::minutes(5))
}

pub(crate) fn hour_window(now: OffsetDateTime) -> String {
    format!(
        "{:04}{:02}{:02}T{:02}",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour()
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Job {
    pub idempotency_key: String,
    pub job_type: String,
    pub class: JobClass,
    pub org_id: Option<OrgId>,
    pub connection: Option<(ScmInstanceId, ConnectionId)>,
    pub queue_id: Option<QueueId>,
    pub payload: Value,
    /// Attempts started, this one included once claimed.
    pub attempts: i32,
}

/// Requests `job`, to run from `run_after`, and returns `true`, or returns
/// `false` when a job with the same key exists: a second request changes
/// nothing. A new job notifies the workers when the transaction commits.
///
/// # Errors
///
/// Postgres refuses the row: another organization than the context, or a
/// connection or queue of another organization.
pub async fn enqueue_job(
    conn: &mut PgConnection,
    job: &Job,
    run_after: OffsetDateTime,
    now: OffsetDateTime,
) -> Result<bool, PlatformError> {
    let (instance, connection) = job.connection.unzip();
    let done = sqlx::query(
        "INSERT INTO jobs (idempotency_key, type, class, org_id, scm_instance_id, connection_id,
                           queue_id, payload, run_after, created_at)
         VALUES ($1, $2, $3::job_class, $4, $5, $6, $7, $8, $9, $10)
         ON CONFLICT (idempotency_key) DO NOTHING",
    )
    .bind(&job.idempotency_key)
    .bind(&job.job_type)
    .bind(job.class.as_str())
    .bind(job.org_id.map(|id| id.to_string()))
    .bind(instance.map(|id| id.to_string()))
    .bind(connection.map(|id| id.to_string()))
    .bind(job.queue_id.map(|id| id.to_string()))
    .bind(&job.payload)
    .bind(run_after)
    .bind(now)
    .execute(&mut *conn)
    .await
    .map_err(PlatformError::Query)?;
    let enqueued = done.rows_affected() == 1;
    if enqueued {
        notify(conn, JOB_CHANNEL).await?;
    }
    Ok(enqueued)
}

type JobRow = (
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Value,
    i32,
);

/// Gives `worker` at most `limit` jobs that are due at `now`, highest class
/// first, until `now + JOB_LOCK`. A job whose worker stopped is due again when
/// its lock expires. A connection gets no more jobs once it has
/// [`JOBS_PER_CONNECTION`] in progress, nor while it is paused
/// ([`record_budgets`](crate::record_budgets)). Runs in the system context: a worker
/// serves every organization.
///
/// # Errors
///
/// Postgres fails, or a row does not match the vocabulary of `cihub-domain`.
pub async fn claim_jobs(
    conn: &mut PgConnection,
    worker: &str,
    now: OffsetDateTime,
    limit: i64,
) -> Result<Vec<Job>, PlatformError> {
    let rows: Vec<JobRow> = sqlx::query_as(
        "WITH in_progress AS (
             SELECT connection_id, count(*) AS n FROM jobs
             WHERE state = 'running' AND locked_until > $2 AND connection_id IS NOT NULL
             GROUP BY connection_id
         ), candidates AS (
             SELECT idempotency_key, connection_id, class, run_after FROM jobs
             WHERE ((state = 'queued' AND run_after <= $2)
                    OR (state = 'running' AND locked_until <= $2))
               AND NOT EXISTS (
                   SELECT 1 FROM scm_connections s
                   WHERE s.id = jobs.connection_id AND s.paused_until > $2
               )
             ORDER BY class, run_after, idempotency_key
             LIMIT $4 * 10
         ), eligible AS (
             SELECT c.idempotency_key FROM (
                 SELECT idempotency_key, connection_id, class, run_after,
                        row_number() OVER (
                            PARTITION BY connection_id ORDER BY class, run_after, idempotency_key
                        ) AS rank
                 FROM candidates
             ) c LEFT JOIN in_progress p USING (connection_id)
             WHERE c.connection_id IS NULL OR c.rank + coalesce(p.n, 0) <= $5
         ), chosen AS (
             -- The state is checked again on the locked row: a concurrent
             -- worker can claim a candidate after the snapshot of this query.
             SELECT idempotency_key FROM jobs
             WHERE idempotency_key IN (SELECT idempotency_key FROM eligible)
               AND ((state = 'queued' AND run_after <= $2)
                    OR (state = 'running' AND locked_until <= $2))
             ORDER BY class, run_after, idempotency_key
             LIMIT $4
             FOR UPDATE SKIP LOCKED
         ), claimed AS (
             UPDATE jobs SET state = 'running', locked_by = $1, locked_until = $3,
                             attempts = jobs.attempts + 1
             FROM chosen WHERE jobs.idempotency_key = chosen.idempotency_key
             RETURNING jobs.*
         )
         SELECT idempotency_key, type, class::text, org_id, scm_instance_id, connection_id,
                queue_id, payload, attempts
         FROM claimed ORDER BY class, run_after, idempotency_key",
    )
    .bind(worker)
    .bind(now)
    .bind(now + JOB_LOCK)
    .bind(limit)
    .bind(JOBS_PER_CONNECTION)
    .fetch_all(conn)
    .await
    .map_err(PlatformError::Query)?;
    rows.into_iter().map(decode).collect()
}

fn decode(
    (key, job_type, class, org_id, instance, connection, queue_id, payload, attempts): JobRow,
) -> Result<Job, PlatformError> {
    let connection = match (instance, connection) {
        (Some(instance), Some(connection)) => Some((
            instance.parse().map_err(PlatformError::read("job", &key))?,
            connection
                .parse()
                .map_err(PlatformError::read("job", &key))?,
        )),
        _ => None,
    };
    Ok(Job {
        job_type,
        class: class.parse().map_err(PlatformError::read("job", &key))?,
        org_id: org_id
            .map(|id| id.parse())
            .transpose()
            .map_err(PlatformError::read("job", &key))?,
        connection,
        queue_id: queue_id
            .map(|id| id.parse())
            .transpose()
            .map_err(PlatformError::read("job", &key))?,
        payload,
        attempts,
        idempotency_key: key.clone(),
    })
}

/// Marks the job `key` as done at `now`, and returns `true`, while `worker`
/// still holds it. Returns `false` when its lock expired and another worker
/// took it back: that worker finishes it.
///
/// # Errors
///
/// Postgres fails.
pub async fn complete_job(
    conn: &mut PgConnection,
    key: &str,
    worker: &str,
    now: OffsetDateTime,
) -> Result<bool, PlatformError> {
    let done = sqlx::query(
        "UPDATE jobs SET state = 'done', locked_by = NULL, locked_until = NULL, finished_at = $3
         WHERE idempotency_key = $1 AND state = 'running' AND locked_by = $2",
    )
    .bind(key)
    .bind(worker)
    .bind(now)
    .execute(conn)
    .await
    .map_err(PlatformError::Query)?;
    Ok(done.rows_affected() == 1)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Retry {
    Later,
    Never,
}

/// Records that `worker` failed the job `key` at `now`, with `error`, and
/// returns the new state of the job: queued again after its backoff, `dead`
/// after [`MAX_ATTEMPTS`], or `failed` when a retry cannot fix it. Returns
/// `None`, and changes nothing, when `worker` no longer holds the job.
///
/// # Errors
///
/// Postgres fails.
pub async fn fail_job(
    conn: &mut PgConnection,
    key: &str,
    worker: &str,
    now: OffsetDateTime,
    error: &str,
    retry: Retry,
) -> Result<Option<JobState>, PlatformError> {
    let attempts: Option<i32> = sqlx::query_scalar(
        "SELECT attempts FROM jobs
         WHERE idempotency_key = $1 AND state = 'running' AND locked_by = $2
         FOR UPDATE",
    )
    .bind(key)
    .bind(worker)
    .fetch_optional(&mut *conn)
    .await
    .map_err(PlatformError::Query)?;
    let Some(attempts) = attempts else {
        return Ok(None);
    };
    let state = match retry {
        Retry::Never => JobState::Failed,
        Retry::Later if attempts >= MAX_ATTEMPTS => JobState::Dead,
        Retry::Later => JobState::Queued,
    };
    sqlx::query(
        "UPDATE jobs SET state = $2::job_state, run_after = $3, last_error = $4,
                         locked_by = NULL, locked_until = NULL,
                         finished_at = CASE WHEN $2::job_state = 'queued' THEN NULL ELSE $5 END
         WHERE idempotency_key = $1",
    )
    .bind(key)
    .bind(state.as_str())
    .bind(now + backoff(attempts))
    .bind(error)
    .bind(now)
    .execute(conn)
    .await
    .map_err(PlatformError::Query)?;
    Ok(Some(state))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_up_to_five_minutes() {
        let waits: Vec<i64> = (1..=10).map(|n| backoff(n).whole_seconds()).collect();
        assert_eq!(waits, [2, 4, 8, 16, 32, 64, 128, 256, 300, 300]);
    }
}

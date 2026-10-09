use cihub_domain::QueueId;
use sqlx::PgConnection;
use time::{Duration, OffsetDateTime};

use crate::PlatformError;

pub const LEASE_TTL: Duration = Duration::seconds(15);

/// Takes or renews the leases of at most `limit` queues that have work at
/// `now` (dirty, or a tick due), and returns them. A queue whose lease another
/// owner holds is skipped until the lease expires. Runs in the system context:
/// the scheduler serves every organization.
///
/// # Errors
///
/// Postgres fails, or a queue identifier is not a `QueueId`.
pub async fn acquire_leases(
    conn: &mut PgConnection,
    owner: &str,
    now: OffsetDateTime,
    limit: i64,
) -> Result<Vec<QueueId>, PlatformError> {
    let ids: Vec<String> = sqlx::query_scalar(
        "WITH leased AS (
             UPDATE queues SET lease_owner = $1, lease_expires_at = $3
             WHERE id IN (
                 SELECT id FROM queues
                 WHERE (dirty OR next_tick_at <= $2)
                   AND (lease_owner IS NULL OR lease_owner = $1 OR lease_expires_at <= $2)
                 ORDER BY dirty DESC, next_tick_at NULLS LAST, id
                 LIMIT $4
                 FOR UPDATE SKIP LOCKED)
             RETURNING id, dirty, next_tick_at)
         SELECT id FROM leased ORDER BY dirty DESC, next_tick_at NULLS LAST, id",
    )
    .bind(owner)
    .bind(now)
    .bind(now + LEASE_TTL)
    .bind(limit)
    .fetch_all(conn)
    .await
    .map_err(PlatformError::Query)?;

    ids.into_iter()
        .map(|id| id.parse().map_err(PlatformError::read("queue", &id)))
        .collect()
}

/// Gives back every lease of `owner`, at shutdown, so that another replica
/// does not wait for them to expire. Returns the number of leases.
///
/// # Errors
///
/// Postgres fails.
pub async fn release_leases(conn: &mut PgConnection, owner: &str) -> Result<u64, PlatformError> {
    let done = sqlx::query(
        "UPDATE queues SET lease_owner = NULL, lease_expires_at = NULL WHERE lease_owner = $1",
    )
    .bind(owner)
    .execute(conn)
    .await
    .map_err(PlatformError::Query)?;
    Ok(done.rows_affected())
}

/// Takes the lock of `queue` until the end of the transaction of `conn`, or
/// returns `false` when another transaction holds it. A reconciliation runs
/// only with this lock, even when its lease was taken back meanwhile.
///
/// # Errors
///
/// Postgres fails.
pub async fn lock_queue(conn: &mut PgConnection, queue: QueueId) -> Result<bool, PlatformError> {
    // Two queues share a lock only if their 64-bit hashes collide; they then
    // wait for each other, which is slow but safe.
    sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(queue.to_string())
        .fetch_one(conn)
        .await
        .map_err(PlatformError::Query)
}

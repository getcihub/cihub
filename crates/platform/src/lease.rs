use cihub_domain::QueueId;
use sqlx::PgConnection;
use time::{Duration, OffsetDateTime};

use crate::PlatformError;

/// A lease that its owner does not renew goes to another replica after this
/// time (`DECISIONS.md` D54).
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
    // Two owners that insert the same queue at the same time conflict on the
    // key: the `WHERE` of `DO UPDATE` reads the row of the winner, so the
    // other one gets nothing.
    let ids: Vec<String> = sqlx::query_scalar(
        "INSERT INTO queue_leases (queue_id, org_id, owner, expires_at)
         SELECT q.id, q.org_id, $1, $3
         FROM queues q LEFT JOIN queue_leases l ON l.queue_id = q.id
         WHERE (q.dirty OR q.next_tick_at <= $2)
           AND (l.queue_id IS NULL OR l.owner = $1 OR l.expires_at <= $2)
         ORDER BY q.dirty DESC, q.next_tick_at NULLS LAST, q.id
         LIMIT $4
         ON CONFLICT (queue_id) DO UPDATE SET owner = excluded.owner, expires_at = excluded.expires_at
             WHERE queue_leases.owner = excluded.owner OR queue_leases.expires_at <= $2
         RETURNING queue_id",
    )
    .bind(owner)
    .bind(now)
    .bind(now + LEASE_TTL)
    .bind(limit)
    .fetch_all(conn)
    .await
    .map_err(PlatformError::Query)?;
    ids.into_iter()
        .map(|id| {
            id.parse()
                .map_err(|source| PlatformError::ReadQueue { id, source })
        })
        .collect()
}

/// Gives back every lease of `owner`, at shutdown, so that another replica
/// does not wait for them to expire. Returns the number of leases.
///
/// # Errors
///
/// Postgres fails.
pub async fn release_leases(conn: &mut PgConnection, owner: &str) -> Result<u64, PlatformError> {
    let done = sqlx::query("DELETE FROM queue_leases WHERE owner = $1")
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

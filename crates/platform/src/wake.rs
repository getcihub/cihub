use std::error::Error;
use std::time::Duration;

use cihub_domain::QueueId;
use cihub_store::{Postgres, Transaction, begin_system};
use sqlx::postgres::PgListener;
use sqlx::{PgConnection, PgPool};
use time::OffsetDateTime;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::{PlatformError, lock_queue};

pub(crate) const QUEUE_CHANNEL: &str = "cihub_queue";

pub(crate) async fn notify(conn: &mut PgConnection, channel: &str) -> Result<(), PlatformError> {
    sqlx::query("SELECT pg_notify($1, '')")
        .bind(channel)
        .execute(conn)
        .await
        .map_err(PlatformError::Query)?;
    Ok(())
}

pub(crate) async fn wake_for_delivery(
    conn: &mut PgConnection,
    queue: Option<QueueId>,
) -> Result<(), PlatformError> {
    match queue {
        Some(queue) => mark_dirty(conn, queue).await,
        None => notify(conn, QUEUE_CHANNEL).await,
    }
}

pub const TICK: Duration = Duration::from_secs(30);

/// Marks `queue` as having work, and notifies the schedulers when the
/// transaction of `conn` commits. Every write that a queue must react to calls
/// it in the same transaction: a delivery, a job result, a request of the API.
///
/// # Errors
///
/// Postgres fails.
pub async fn mark_dirty(conn: &mut PgConnection, queue: QueueId) -> Result<(), PlatformError> {
    sqlx::query(
        "WITH marked AS (UPDATE queues SET dirty = true WHERE id = $1)
         SELECT pg_notify($2, $1)",
    )
    .bind(queue.to_string())
    .bind(QUEUE_CHANNEL)
    .execute(conn)
    .await
    .map_err(PlatformError::Query)?;
    Ok(())
}

/// Opens the transaction that reconciles `queue`, in the system context, or
/// returns `None` when another transaction reconciles it. The queue is no
/// longer dirty from the start of the transaction: a write that marks it dirty
/// meanwhile waits for the commit, then marks it again, so no work is lost.
///
/// # Errors
///
/// Postgres fails.
pub async fn begin_reconciliation(
    pool: &PgPool,
    queue: QueueId,
) -> Result<Option<Transaction<'static, Postgres>>, PlatformError> {
    let mut tx = begin_system(pool).await.map_err(PlatformError::Store)?;
    if !lock_queue(&mut tx, queue).await? {
        return Ok(None);
    }
    sqlx::query("UPDATE queues SET dirty = false WHERE id = $1")
        .bind(queue.to_string())
        .execute(&mut *tx)
        .await
        .map_err(PlatformError::Query)?;
    Ok(Some(tx))
}

/// Sets when `queue` is reconciled again without a change: `None` waits for a
/// change.
///
/// # Errors
///
/// Postgres fails.
pub async fn schedule_tick(
    conn: &mut PgConnection,
    queue: QueueId,
    at: Option<OffsetDateTime>,
) -> Result<(), PlatformError> {
    sqlx::query("UPDATE queues SET next_tick_at = $2 WHERE id = $1")
        .bind(queue.to_string())
        .bind(at)
        .execute(conn)
        .await
        .map_err(PlatformError::Query)?;
    Ok(())
}

pub const MIN_WAIT: Duration = Duration::from_secs(1);

/// How long a scheduler waits for a notification after a pass: until the next
/// tick of a queue that is not due yet, between [`MIN_WAIT`] and [`TICK`], so
/// that the `wake_at` of a plan is served on time and not at the next
/// [`TICK`].
///
/// # Errors
///
/// Postgres fails.
pub async fn until_next_tick(
    conn: &mut PgConnection,
    now: OffsetDateTime,
) -> Result<Duration, PlatformError> {
    let next: Option<OffsetDateTime> =
        sqlx::query_scalar("SELECT min(next_tick_at) FROM queues WHERE next_tick_at > $1")
            .bind(now)
            .fetch_one(conn)
            .await
            .map_err(PlatformError::Query)?;
    Ok(next.map_or(TICK, |at| (at - now).unsigned_abs().clamp(MIN_WAIT, TICK)))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wake {
    Notified,
    Tick,
    Cancelled,
}

pub struct QueueWakeup {
    listener: PgListener,
}

impl QueueWakeup {
    /// # Errors
    ///
    /// Postgres cannot be reached.
    pub async fn listen(pool: &PgPool) -> Result<Self, PlatformError> {
        Self::listen_to(pool, QUEUE_CHANNEL).await
    }

    pub(crate) async fn listen_to(pool: &PgPool, channel: &str) -> Result<Self, PlatformError> {
        let mut listener = PgListener::connect_with(pool)
            .await
            .map_err(PlatformError::Query)?;
        listener
            .listen(channel)
            .await
            .map_err(PlatformError::Query)?;
        Ok(Self { listener })
    }

    /// Waits for a notification, for `tick`, or for `cancel`, whichever comes
    /// first. The notifications received during a scan wait in the buffer of
    /// the listener, so the next call returns at once.
    ///
    /// # Errors
    ///
    /// The connection is lost and cannot be opened again; the caller waits
    /// for the tick before it tries again.
    pub async fn wait(
        &mut self,
        tick: Duration,
        cancel: &CancellationToken,
    ) -> Result<Wake, PlatformError> {
        self.recv(tick, cancel).await.map_err(PlatformError::Query)
    }

    /// Like [`wait`](Self::wait), but a lost connection is logged with the
    /// name of the `notifications`, then waits for `tick` or `cancel` before
    /// the caller looks for work again.
    pub async fn wait_or_pause(
        &mut self,
        tick: Duration,
        cancel: &CancellationToken,
        notifications: &'static str,
    ) {
        if let Err(err) = self.recv(tick, cancel).await {
            warn!(
                error = &err as &dyn Error,
                notifications, "notifications.lost"
            );
            tokio::select! {
                () = cancel.cancelled() => {}
                () = tokio::time::sleep(tick) => {}
            }
        }
    }

    async fn recv(
        &mut self,
        tick: Duration,
        cancel: &CancellationToken,
    ) -> Result<Wake, sqlx::Error> {
        tokio::select! {
            () = cancel.cancelled() => Ok(Wake::Cancelled),
            received = self.listener.recv() => received.map(|_| Wake::Notified),
            () = tokio::time::sleep(tick) => Ok(Wake::Tick),
        }
    }
}

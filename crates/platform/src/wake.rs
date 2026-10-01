use std::time::Duration;

use cihub_domain::QueueId;
use cihub_store::{Postgres, Transaction, begin_system};
use sqlx::postgres::PgListener;
use sqlx::{PgConnection, PgPool};
use time::OffsetDateTime;
use tokio_util::sync::CancellationToken;

use crate::{PlatformError, lock_queue};

/// The channel that says a queue has work; the payload is its identifier, or
/// empty for a delivery of a connection without a queue.
pub const QUEUE_CHANNEL: &str = "cihub_queue";

/// Notifies the schedulers, when the transaction of `conn` commits, that a
/// delivery of a connection waits without a queue.
///
/// # Errors
///
/// Postgres fails.
pub(crate) async fn notify_connection_delivery(
    conn: &mut PgConnection,
) -> Result<(), PlatformError> {
    sqlx::query("SELECT pg_notify($1, '')")
        .bind(QUEUE_CHANNEL)
        .execute(conn)
        .await
        .map_err(PlatformError::Query)?;
    Ok(())
}

/// A scheduler looks for work at least this often, even without a
/// notification: a notification sent while its connection was down is lost.
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

/// Why a scheduler woke up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wake {
    /// A queue was marked dirty.
    Notified,
    /// [`TICK`] passed without a notification.
    Tick,
    /// The process stops.
    Cancelled,
}

/// The notifications of [`QUEUE_CHANNEL`], on a connection of its own.
pub struct QueueWakeup {
    listener: PgListener,
}

impl QueueWakeup {
    /// Listens to [`QUEUE_CHANNEL`] on a new connection of `pool`.
    ///
    /// # Errors
    ///
    /// Postgres cannot be reached.
    pub async fn listen(pool: &PgPool) -> Result<Self, PlatformError> {
        let mut listener = PgListener::connect_with(pool)
            .await
            .map_err(PlatformError::Query)?;
        listener
            .listen(QUEUE_CHANNEL)
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
        tokio::select! {
            () = cancel.cancelled() => Ok(Wake::Cancelled),
            received = self.listener.recv() => {
                received.map(|_| Wake::Notified).map_err(PlatformError::Query)
            }
            () = tokio::time::sleep(tick) => Ok(Wake::Tick),
        }
    }
}

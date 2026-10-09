use cihub_domain::{ConnectionId, OrgId, QueueId};
use serde_json::Value;
use sqlx::PgConnection;
use time::OffsetDateTime;

use crate::PlatformError;
use crate::wake::{notify, wake_for_delivery};

pub(crate) const INBOX_CHANNEL: &str = "cihub_inbox";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Delivery {
    pub delivery_id: String,
    pub org_id: Option<OrgId>,
    pub connection_id: Option<ConnectionId>,
    pub queue_id: Option<QueueId>,
    pub event: String,
    pub payload: Value,
    pub received_at: OffsetDateTime,
}

/// Stores `delivery` and returns `true`, or returns `false` when the inbox
/// already has it: a copy of a delivery changes nothing. A delivery with its
/// organization is routed; a new delivery of a queue marks the queue dirty in
/// the same transaction, and a new delivery of a connection without a queue
/// wakes the schedulers. A new delivery without organization waits for the
/// routing loop, which [`INBOX_CHANNEL`] wakes.
///
/// # Errors
///
/// Postgres refuses the row: another organization than the context, or a
/// connection or queue of another organization.
pub async fn store_delivery(
    conn: &mut PgConnection,
    delivery: &Delivery,
) -> Result<bool, PlatformError> {
    let done = sqlx::query(
        "INSERT INTO webhook_inbox (delivery_id, org_id, connection_id, queue_id, event, payload,
                                    received_at, routed_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, CASE WHEN $2 IS NULL THEN NULL ELSE $7 END)
         ON CONFLICT (delivery_id) DO NOTHING",
    )
    .bind(&delivery.delivery_id)
    .bind(delivery.org_id.map(|id| id.to_string()))
    .bind(delivery.connection_id.map(|id| id.to_string()))
    .bind(delivery.queue_id.map(|id| id.to_string()))
    .bind(&delivery.event)
    .bind(&delivery.payload)
    .bind(delivery.received_at)
    .execute(&mut *conn)
    .await
    .map_err(PlatformError::Query)?;
    let stored = done.rows_affected() == 1;
    match (stored, delivery.org_id) {
        (true, Some(_)) => wake_for_delivery(conn, delivery.queue_id).await?,
        (true, None) => notify(conn, INBOX_CHANNEL).await?,
        (false, _) => {}
    }
    Ok(stored)
}

pub(crate) type DeliveryRow = (
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    Value,
    OffsetDateTime,
);

/// The deliveries of `queue` not processed yet, at most `limit`, in the order
/// of reception. The caller holds the lock of the queue.
///
/// # Errors
///
/// Postgres fails, or a row does not match the identifiers of `cihub-domain`.
pub async fn pending_deliveries(
    conn: &mut PgConnection,
    queue: QueueId,
    limit: i64,
) -> Result<Vec<Delivery>, PlatformError> {
    let rows: Vec<DeliveryRow> = sqlx::query_as(
        "SELECT delivery_id, org_id, connection_id, queue_id, event, payload, received_at
         FROM webhook_inbox
         WHERE queue_id = $1 AND processed_at IS NULL
         ORDER BY received_at, delivery_id
         LIMIT $2",
    )
    .bind(queue.to_string())
    .bind(limit)
    .fetch_all(conn)
    .await
    .map_err(PlatformError::Query)?;
    rows.into_iter().map(decode).collect()
}

pub(crate) fn decode(
    (delivery_id, org_id, connection_id, queue_id, event, payload, received_at): DeliveryRow,
) -> Result<Delivery, PlatformError> {
    let read = || PlatformError::read("delivery", &delivery_id);
    Ok(Delivery {
        org_id: org_id.map(|id| id.parse()).transpose().map_err(read())?,
        connection_id: connection_id
            .map(|id| id.parse())
            .transpose()
            .map_err(read())?,
        queue_id: queue_id.map(|id| id.parse()).transpose().map_err(read())?,
        delivery_id: delivery_id.clone(),
        event,
        payload,
        received_at,
    })
}

/// Marks a delivery as processed at `now`, with the reason when the processing
/// failed. The caller does it in the transaction that writes the facts of the
/// delivery, so that a crash neither loses nor repeats them.
///
/// # Errors
///
/// Postgres fails.
pub async fn mark_processed(
    conn: &mut PgConnection,
    delivery_id: &str,
    now: OffsetDateTime,
    error: Option<&str>,
) -> Result<(), PlatformError> {
    sqlx::query("UPDATE webhook_inbox SET processed_at = $2, error = $3 WHERE delivery_id = $1")
        .bind(delivery_id)
        .bind(now)
        .bind(error)
        .execute(conn)
        .await
        .map_err(PlatformError::Query)?;
    Ok(())
}

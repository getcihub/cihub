use cihub_domain::{ConnectionId, OrgId, QueueId, ScmInstanceId};
use serde_json::Value;
use sqlx::PgConnection;
use time::OffsetDateTime;

use crate::PlatformError;
use crate::wake::{notify, wake_for_delivery};

/// The channel that says a delivery waits for its routing.
pub(crate) const INBOX_CHANNEL: &str = "cihub_inbox";

/// A webhook delivery as the ingestion receives it: stored as is, before any
/// decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Delivery {
    pub scm_instance_id: ScmInstanceId,
    /// The identifier that the SCM gives to the delivery; a copy has the same.
    pub delivery_id: String,
    /// `None` while the ingestion cannot route the delivery.
    pub org_id: Option<OrgId>,
    pub connection_id: Option<ConnectionId>,
    pub queue_id: Option<QueueId>,
    /// The kind of event, as the SCM names it (`pull_request`).
    pub event: String,
    pub action: Option<String>,
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
        "INSERT INTO webhook_inbox (scm_instance_id, delivery_id, org_id, connection_id, queue_id,
                                    event, action, payload, received_at, routed_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, CASE WHEN $3 IS NULL THEN NULL ELSE $9 END)
         ON CONFLICT (scm_instance_id, delivery_id) DO NOTHING",
    )
    .bind(delivery.scm_instance_id.to_string())
    .bind(&delivery.delivery_id)
    .bind(delivery.org_id.map(|id| id.to_string()))
    .bind(delivery.connection_id.map(|id| id.to_string()))
    .bind(delivery.queue_id.map(|id| id.to_string()))
    .bind(&delivery.event)
    .bind(&delivery.action)
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
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    Option<String>,
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
        "SELECT scm_instance_id, delivery_id, org_id, connection_id, queue_id, event, action,
                payload, received_at
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
    (
        scm_instance_id,
        delivery_id,
        org_id,
        connection_id,
        queue_id,
        event,
        action,
        payload,
        received_at,
    ): DeliveryRow,
) -> Result<Delivery, PlatformError> {
    let read = || PlatformError::read("delivery", &delivery_id);
    Ok(Delivery {
        scm_instance_id: scm_instance_id.parse().map_err(read())?,
        org_id: org_id.map(|id| id.parse()).transpose().map_err(read())?,
        connection_id: connection_id
            .map(|id| id.parse())
            .transpose()
            .map_err(read())?,
        queue_id: queue_id.map(|id| id.parse()).transpose().map_err(read())?,
        delivery_id: delivery_id.clone(),
        event,
        action,
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
    scm_instance_id: ScmInstanceId,
    delivery_id: &str,
    now: OffsetDateTime,
    error: Option<&str>,
) -> Result<(), PlatformError> {
    sqlx::query(
        "UPDATE webhook_inbox SET processed_at = $3, error = $4
         WHERE scm_instance_id = $1 AND delivery_id = $2",
    )
    .bind(scm_instance_id.to_string())
    .bind(delivery_id)
    .bind(now)
    .bind(error)
    .execute(conn)
    .await
    .map_err(PlatformError::Query)?;
    Ok(())
}

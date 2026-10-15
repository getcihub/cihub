use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use cihub_domain::{Event, OrgId, QueueId};
use cihub_store::{begin_org, begin_system, load_event_page};
use sqlx::{PgConnection, PgPool};
use tokio::sync::broadcast::error::TryRecvError;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::{LoopError, PlatformError, QueueNewsFeed};

/// The events that a subscription reads in one transaction.
const EVENTS_PER_DELIVERY: i64 = 500;

/// A change heard wakes the subscriptions at once; this reads the logs again
/// when a change is not heard.
pub const SUBSCRIPTIONS_POLL: Duration = Duration::from_secs(30);

pub type SubscriptionFuture<'a> = Pin<Box<dyn Future<Output = Result<(), LoopError>> + Send + 'a>>;

/// Writes the effect of the events of one queue, in the transaction of its
/// cursor.
pub type SubscriptionHandler = Box<
    dyn for<'a> Fn(&'a mut PgConnection, &'a [(i64, Event)]) -> SubscriptionFuture<'a>
        + Send
        + Sync,
>;

/// A reader of the logs of the queues, `events` being the outbox. It receives
/// the events of one queue in order, after its cursor, in the transaction of
/// the organization that advances the cursor: what it writes there, it writes
/// once.
pub struct Subscription {
    /// The key of its cursors: renaming it reads every log again.
    pub name: &'static str,
    pub handle: SubscriptionHandler,
}

/// Delivers the logs to `subscriptions` until `cancel`: at each change heard on
/// `cihub_queue_changed`, and every [`SUBSCRIPTIONS_POLL`].
///
/// # Errors
///
/// The changes of the queues cannot be heard.
pub async fn run_subscriptions(
    pool: PgPool,
    subscriptions: Vec<Subscription>,
    cancel: CancellationToken,
) -> Result<(), PlatformError> {
    let feed = QueueNewsFeed::start(pool.clone(), cancel.clone()).await?;
    let mut news = feed.subscribe();
    let mut every = tokio::time::interval(SUBSCRIPTIONS_POLL);

    loop {
        tokio::select! {
            () = cancel.cancelled() => return Ok(()),
            _ = every.tick() => {}
            _ = news.recv() => {}
        }
        // The changes heard meanwhile are read by this pass.
        while !matches!(
            news.try_recv(),
            Err(TryRecvError::Empty | TryRecvError::Closed)
        ) {}

        for subscription in &subscriptions {
            if let Err(err) = deliver_all(&pool, subscription).await {
                warn!(
                    error = &err as &dyn Error,
                    subscription = subscription.name,
                    "log not delivered"
                );
            }
        }
    }
}

/// Delivers to `subscription` the events of every queue after its cursor. A
/// queue whose events the subscription refuses keeps its cursor, and the
/// next queues still get theirs.
///
/// # Errors
///
/// Postgres fails, or the subscription fails on a queue: the first error.
pub async fn deliver_all(pool: &PgPool, subscription: &Subscription) -> Result<(), PlatformError> {
    let mut tx = begin_system(pool).await.map_err(PlatformError::Store)?;
    // The last event of each queue, read backwards on `events_queue_id_seq`:
    // an `EXISTS` after the cursor let Postgres read the whole log.
    let behind: Vec<(String, String)> = sqlx::query_as(
        "SELECT q.org_id, q.id FROM queues q
         CROSS JOIN LATERAL (SELECT max(e.seq) AS seq FROM events e WHERE e.queue_id = q.id) last
         LEFT JOIN log_cursors c ON c.queue_id = q.id AND c.subscription = $1
         WHERE last.seq > coalesce(c.seq, 0)
         ORDER BY q.id",
    )
    .bind(subscription.name)
    .fetch_all(&mut *tx)
    .await
    .map_err(PlatformError::Query)?;
    tx.commit().await.map_err(PlatformError::Query)?;

    let mut first_error = None;
    for (org, queue) in behind {
        let org: OrgId = org
            .parse()
            .map_err(PlatformError::read("organization", &org))?;
        let queue: QueueId = queue
            .parse()
            .map_err(PlatformError::read("queue", &queue))?;
        if let Err(err) = deliver_queue(pool, subscription, org, queue).await {
            warn!(
                error = &err as &dyn Error,
                subscription = subscription.name,
                queue_id = %queue,
                "log of a queue not delivered"
            );
            first_error.get_or_insert(err);
        }
    }
    first_error.map_or(Ok(()), Err)
}

async fn deliver_queue(
    pool: &PgPool,
    subscription: &Subscription,
    org: OrgId,
    queue: QueueId,
) -> Result<(), PlatformError> {
    while deliver(pool, subscription, org, queue).await? {}
    Ok(())
}

/// Delivers to `subscription` the next events of `queue`, and returns whether
/// some are left. Another replica that delivers the same queue makes it return
/// `false`.
async fn deliver(
    pool: &PgPool,
    subscription: &Subscription,
    org: OrgId,
    queue: QueueId,
) -> Result<bool, PlatformError> {
    let mut tx = begin_org(pool, org).await.map_err(PlatformError::Store)?;
    sqlx::query(
        "INSERT INTO log_cursors (org_id, queue_id, subscription, seq) VALUES ($1, $2, $3, 0)
         ON CONFLICT DO NOTHING",
    )
    .bind(org.to_string())
    .bind(queue.to_string())
    .bind(subscription.name)
    .execute(&mut *tx)
    .await
    .map_err(PlatformError::Query)?;
    let cursor: Option<i64> = sqlx::query_scalar(
        "SELECT seq FROM log_cursors WHERE queue_id = $1 AND subscription = $2
         FOR UPDATE SKIP LOCKED",
    )
    .bind(queue.to_string())
    .bind(subscription.name)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PlatformError::Query)?;
    let Some(cursor) = cursor else {
        return Ok(false);
    };

    let events = load_event_page(&mut tx, queue, cursor, EVENTS_PER_DELIVERY)
        .await
        .map_err(PlatformError::Store)?;
    let Some(&(last, _)) = events.last() else {
        return Ok(false);
    };

    (subscription.handle)(&mut tx, &events)
        .await
        .map_err(|source| PlatformError::Subscription {
            name: subscription.name,
            source,
        })?;
    sqlx::query("UPDATE log_cursors SET seq = $3 WHERE queue_id = $1 AND subscription = $2")
        .bind(queue.to_string())
        .bind(subscription.name)
        .bind(last)
        .execute(&mut *tx)
        .await
        .map_err(PlatformError::Query)?;
    tx.commit().await.map_err(PlatformError::Query)?;

    debug!(
        subscription = subscription.name,
        queue_id = %queue,
        seq = last,
        "log delivered"
    );
    Ok(i64::try_from(events.len()).is_ok_and(|count| count == EVENTS_PER_DELIVERY))
}

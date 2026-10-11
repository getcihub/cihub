use std::error::Error;
use std::time::Duration;

use cihub_domain::{OrgId, QueueId};
use sqlx::postgres::PgListener;
use sqlx::{PgConnection, PgPool};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::PlatformError;

/// Not `cihub_queue`, which says that a queue has work: this one says that
/// what its pages show changed, once the transaction that changed it commits.
const QUEUE_CHANGED_CHANNEL: &str = "cihub_queue_changed";

/// The changes that a page may lag behind before it reads everything again.
const NEWS_KEPT: usize = 1024;

const LISTEN_AGAIN_AFTER: Duration = Duration::from_secs(1);

/// A queue whose log, or what its pages show, changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueueChange {
    pub org_id: OrgId,
    pub queue_id: QueueId,
    /// The sequence number of the last event of its log; 0 before the first.
    pub seq: i64,
}

/// What the pages of an instance hear about the queues.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueNews {
    Changed(QueueChange),
    /// The listener lost changes: a page reads everything again.
    Lost,
}

/// Tells the pages, once the transaction of `conn` commits, that `queue`
/// changed.
///
/// # Errors
///
/// Postgres fails.
pub async fn notify_queue_changed(
    conn: &mut PgConnection,
    org: OrgId,
    queue: QueueId,
) -> Result<(), PlatformError> {
    sqlx::query(
        "SELECT pg_notify($1, $2 || ' ' || $3 || ' '
                || coalesce((SELECT max(seq) FROM events WHERE queue_id = $3), 0))",
    )
    .bind(QUEUE_CHANGED_CHANNEL)
    .bind(org.to_string())
    .bind(queue.to_string())
    .execute(conn)
    .await
    .map_err(PlatformError::Query)?;
    Ok(())
}

/// The changes of the queues of every organization, which one listener of
/// the process hears for all the pages that it serves.
#[derive(Clone, Debug)]
pub struct QueueNewsFeed {
    news: broadcast::Sender<QueueNews>,
    cancel: CancellationToken,
}

impl QueueNewsFeed {
    /// Listens to the changes until `cancel`.
    pub fn start(pool: PgPool, cancel: CancellationToken) -> Self {
        let (news, _) = broadcast::channel(NEWS_KEPT);
        tokio::spawn(listen(pool, news.clone(), cancel.clone()));
        Self { news, cancel }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<QueueNews> {
        self.news.subscribe()
    }

    /// Cancelled when the server stops: the streams of the pages end.
    pub fn stopped(&self) -> &CancellationToken {
        &self.cancel
    }
}

async fn listen(pool: PgPool, news: broadcast::Sender<QueueNews>, cancel: CancellationToken) {
    while !cancel.is_cancelled() {
        if let Err(err) = listen_until_lost(&pool, &news, &cancel).await {
            warn!(
                error = &err as &dyn Error,
                "changes of the queues not heard"
            );
        }

        let _ = news.send(QueueNews::Lost);
        tokio::select! {
            () = cancel.cancelled() => {}
            () = tokio::time::sleep(LISTEN_AGAIN_AFTER) => {}
        }
    }
}

/// Passes the changes on until `cancel`, or until the connection drops,
/// when the changes of the meantime are lost.
async fn listen_until_lost(
    pool: &PgPool,
    news: &broadcast::Sender<QueueNews>,
    cancel: &CancellationToken,
) -> Result<(), sqlx::Error> {
    let mut listener = PgListener::connect_with(pool).await?;
    listener.listen(QUEUE_CHANGED_CHANNEL).await?;

    loop {
        let received = tokio::select! {
            () = cancel.cancelled() => return Ok(()),
            received = listener.try_recv() => received?,
        };
        let Some(notification) = received else {
            return Ok(());
        };
        if let Some(change) = change_of(notification.payload()) {
            let _ = news.send(QueueNews::Changed(change));
        }
    }
}

/// `<org> <queue> <seq>`, as `notify_queue_changed` writes it.
fn change_of(payload: &str) -> Option<QueueChange> {
    let mut parts = payload.split(' ');
    let change = QueueChange {
        org_id: parts.next()?.parse().ok()?,
        queue_id: parts.next()?.parse().ok()?,
        seq: parts.next()?.parse().ok()?,
    };
    parts.next().is_none().then_some(change)
}

#[cfg(test)]
mod tests {
    use ulid::Ulid;

    use super::*;

    #[test]
    fn a_change_reads_back_as_it_was_written() {
        let org = OrgId::from_ulid(Ulid(1));
        let queue = QueueId::from_ulid(Ulid(2));

        assert_eq!(
            change_of(&format!("{org} {queue} 42")),
            Some(QueueChange {
                org_id: org,
                queue_id: queue,
                seq: 42
            })
        );
        assert_eq!(change_of(&format!("{org} {queue}")), None);
        assert_eq!(change_of("not a change"), None);
    }
}

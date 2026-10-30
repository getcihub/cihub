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
/// changed; `seq` is the last sequence number of its log, read from the log
/// when the caller does not know it.
///
/// # Errors
///
/// Postgres fails.
pub async fn notify_queue_changed(
    conn: &mut PgConnection,
    org: OrgId,
    queue: QueueId,
    seq: Option<i64>,
) -> Result<(), PlatformError> {
    sqlx::query(
        "SELECT pg_notify($1, $2 || ' ' || $3 || ' '
                || coalesce($4, (SELECT max(seq) FROM events WHERE queue_id = $3), 0))",
    )
    .bind(QUEUE_CHANGED_CHANNEL)
    .bind(org.to_string())
    .bind(queue.to_string())
    .bind(seq)
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
    /// Listens to the changes until `cancel`; returns once it listens, so that
    /// no change of a page opened after it is lost.
    ///
    /// # Errors
    ///
    /// Postgres cannot be reached.
    pub async fn start(pool: PgPool, cancel: CancellationToken) -> Result<Self, PlatformError> {
        let listener = listening(&pool).await.map_err(PlatformError::Query)?;
        let (news, _) = broadcast::channel(NEWS_KEPT);
        tokio::spawn(listen(pool, listener, news.clone(), cancel.clone()));

        Ok(Self { news, cancel })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<QueueNews> {
        self.news.subscribe()
    }

    /// Cancelled when the server stops: the streams of the pages end.
    pub fn stopped(&self) -> &CancellationToken {
        &self.cancel
    }
}

async fn listening(pool: &PgPool) -> Result<PgListener, sqlx::Error> {
    let mut listener = PgListener::connect_with(pool).await?;
    listener.listen(QUEUE_CHANGED_CHANNEL).await?;
    Ok(listener)
}

/// Passes the changes on until `cancel`; listens again after a failure, and
/// tells the pages that the changes of the meantime are lost once it listens.
async fn listen(
    pool: PgPool,
    mut listener: PgListener,
    news: broadcast::Sender<QueueNews>,
    cancel: CancellationToken,
) {
    loop {
        let Err(err) = pass_on(&mut listener, &news, &cancel).await else {
            return;
        };
        warn!(error = &err as &dyn Error, "queue_changes.not_heard");

        let Some(again) = listen_again(&pool, &cancel).await else {
            return;
        };
        listener = again;
        let _ = news.send(QueueNews::Lost);
    }
}

/// A new listener, tried every second until `cancel`.
async fn listen_again(pool: &PgPool, cancel: &CancellationToken) -> Option<PgListener> {
    loop {
        tokio::select! {
            () = cancel.cancelled() => return None,
            () = tokio::time::sleep(LISTEN_AGAIN_AFTER) => {}
        }
        match listening(pool).await {
            Ok(listener) => return Some(listener),
            Err(err) => warn!(error = &err as &dyn Error, "queue_changes.not_heard"),
        }
    }
}

/// Passes the changes on until `cancel`, or until the listener fails. The
/// listener connects again by itself after a lost connection: the pages read
/// everything again, as they may have missed changes.
async fn pass_on(
    listener: &mut PgListener,
    news: &broadcast::Sender<QueueNews>,
    cancel: &CancellationToken,
) -> Result<(), sqlx::Error> {
    loop {
        let received = tokio::select! {
            () = cancel.cancelled() => return Ok(()),
            received = listener.try_recv() => received?,
        };
        let heard = if let Some(notification) = received {
            change_of(notification.payload()).map(QueueNews::Changed)
        } else {
            warn!("queue_changes.connection_lost");
            Some(QueueNews::Lost)
        };
        if let Some(heard) = heard {
            let _ = news.send(heard);
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

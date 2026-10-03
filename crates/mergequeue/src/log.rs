use cihub_domain::{Event, OrgId, QueueId, QueueMode};
use cihub_mq_core::{Outcome, Snapshot, apply};
use cihub_store::{append_events, load_events};
use sqlx::PgConnection;
use time::{Duration, OffsetDateTime};

use crate::projection::project;
use crate::{MergeQueueError, parse_id};

/// The log keeps a snapshot of its queue after every this many events.
pub const SNAPSHOT_EVERY: i64 = 500;

/// The log also keeps a snapshot when its oldest event after the last
/// snapshot is this old, so that a snapshot covers every event before the
/// partitions of `events` are dropped (`DECISIONS.md` D106).
pub(crate) const COVER_AFTER: Duration = Duration::days(30);

/// The state of a queue after the last event of its log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueLog {
    pub snapshot: Snapshot,
    /// The `seq` of the last event; 0 for an empty log.
    pub seq: i64,
    /// The time of the oldest event after the last snapshot.
    pub uncovered_since: Option<OffsetDateTime>,
}

impl QueueLog {
    /// Reads the last snapshot of `queue`, then applies the events after it.
    /// The caller holds the lock of the queue.
    ///
    /// # Errors
    ///
    /// Postgres fails, the queue does not exist, or the log does not fit its
    /// snapshot.
    pub async fn load(conn: &mut PgConnection, queue: QueueId) -> Result<Self, MergeQueueError> {
        let last: Option<(i64, serde_json::Value)> = sqlx::query_as(
            "SELECT seq, payload FROM queue_snapshots WHERE queue_id = $1
             ORDER BY seq DESC LIMIT 1",
        )
        .bind(queue.to_string())
        .fetch_optional(&mut *conn)
        .await
        .map_err(MergeQueueError::Query)?;
        let mut log = match last {
            Some((seq, payload)) => Self {
                snapshot: serde_json::from_value(payload).map_err(MergeQueueError::Snapshot)?,
                seq,
                uncovered_since: None,
            },
            None => Self {
                snapshot: empty(conn, queue).await?,
                seq: 0,
                uncovered_since: None,
            },
        };
        for (seq, event) in load_events(conn, queue, log.seq)
            .await
            .map_err(MergeQueueError::Store)?
        {
            if seq != log.seq + 1 {
                return Err(MergeQueueError::LogGap {
                    queue,
                    after: log.seq,
                });
            }
            log.replay(&event)?;
            log.seq = seq;
            log.uncovered_since.get_or_insert(event.occurred_at);
        }
        Ok(log)
    }

    /// Keeps a snapshot when the oldest event after the last one is older
    /// than [`COVER_AFTER`] at `now`.
    ///
    /// # Errors
    ///
    /// Postgres fails, or the snapshot cannot be serialized.
    pub async fn cover(
        &mut self,
        conn: &mut PgConnection,
        now: OffsetDateTime,
    ) -> Result<(), MergeQueueError> {
        if self
            .uncovered_since
            .is_some_and(|at| at <= now - COVER_AFTER)
        {
            self.keep_snapshot(conn, now).await?;
        }
        Ok(())
    }

    /// Applies `events` in order and writes those that change the state, with
    /// their projections, and a snapshot every [`SNAPSHOT_EVERY`] events.
    /// Returns the events written: a fact that changes nothing is not.
    /// The caller holds the lock of the queue.
    ///
    /// # Errors
    ///
    /// A decision does not fit the snapshot, or Postgres fails; the caller
    /// rolls back.
    pub async fn write(
        &mut self,
        conn: &mut PgConnection,
        events: Vec<Event>,
        now: OffsetDateTime,
    ) -> Result<Vec<Event>, MergeQueueError> {
        let mut written = Vec::new();
        for event in events {
            let outcome =
                apply(&mut self.snapshot, &event).map_err(|source| MergeQueueError::Apply {
                    event_id: event.id,
                    source,
                })?;
            if outcome == Outcome::Ignored {
                continue;
            }
            let seqs = append_events(conn, std::slice::from_ref(&event))
                .await
                .map_err(MergeQueueError::Store)?;
            self.seq = seqs.first().copied().unwrap_or(self.seq + 1);
            self.uncovered_since.get_or_insert(event.occurred_at);
            self.after_applied(conn, &event, now).await?;
            written.push(event);
        }
        Ok(written)
    }

    fn replay(&mut self, event: &Event) -> Result<(), MergeQueueError> {
        match apply(&mut self.snapshot, event) {
            Ok(Outcome::Applied) => Ok(()),
            Ok(Outcome::Ignored) => Err(MergeQueueError::ReplayIgnored(event.id)),
            Err(source) => Err(MergeQueueError::Apply {
                event_id: event.id,
                source,
            }),
        }
    }

    /// Projects the event `self.seq`, already applied, and keeps a snapshot
    /// when its turn comes.
    async fn after_applied(
        &mut self,
        conn: &mut PgConnection,
        event: &Event,
        now: OffsetDateTime,
    ) -> Result<(), MergeQueueError> {
        project(conn, self.seq, event, &self.snapshot).await?;
        if self.seq % SNAPSHOT_EVERY == 0 {
            self.keep_snapshot(conn, now).await?;
        }
        Ok(())
    }

    async fn keep_snapshot(
        &mut self,
        conn: &mut PgConnection,
        now: OffsetDateTime,
    ) -> Result<(), MergeQueueError> {
        let payload = serde_json::to_value(&self.snapshot).map_err(MergeQueueError::Snapshot)?;
        sqlx::query(
            "INSERT INTO queue_snapshots (org_id, queue_id, seq, payload, created_at)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(self.snapshot.org_id.to_string())
        .bind(self.snapshot.queue_id.to_string())
        .bind(self.seq)
        .bind(payload)
        .bind(now)
        .execute(conn)
        .await
        .map_err(MergeQueueError::Query)?;
        self.uncovered_since = None;
        Ok(())
    }
}

/// The state of `queue` before its first event. A queue starts in mode
/// `single`: a mode comes only from the fact `queue.settings_changed`, so
/// that the replay of the log does not depend on the settings of today.
async fn empty(conn: &mut PgConnection, queue: QueueId) -> Result<Snapshot, MergeQueueError> {
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT org_id, target_branch FROM queues WHERE id = $1")
            .bind(queue.to_string())
            .fetch_optional(conn)
            .await
            .map_err(MergeQueueError::Query)?;
    let (org_id, target_branch) = row.ok_or(MergeQueueError::UnknownQueue(queue))?;
    let org_id: OrgId = parse_id(&org_id, queue)?;
    Ok(Snapshot::new(
        org_id,
        queue,
        target_branch,
        QueueMode::Single,
    ))
}

/// Replays the whole log of `queue`: deletes its projections and snapshots,
/// then applies and projects each event again, as when it was written. The
/// projections are then exactly those that the log describes (principle I);
/// the rows of `check_observations` stay, because the replay writes the same
/// ones and the late reports are not in the log (`DECISIONS.md` D118). Once the
/// partitions of its old events are dropped, a log is no longer whole, and
/// only its snapshot holds its state (`DECISIONS.md` D106). The caller holds the
/// lock of the queue.
///
/// # Errors
///
/// Postgres fails, the log is not whole, or the log does not replay: an event
/// that the replay refuses or ignores.
pub async fn rebuild(
    conn: &mut PgConnection,
    queue: QueueId,
    now: OffsetDateTime,
) -> Result<QueueLog, MergeQueueError> {
    let (first, snapshots): (Option<i64>, i64) = sqlx::query_as(
        "SELECT (SELECT min(seq) FROM events WHERE queue_id = $1),
                (SELECT count(*) FROM queue_snapshots WHERE queue_id = $1)",
    )
    .bind(queue.to_string())
    .fetch_one(&mut *conn)
    .await
    .map_err(MergeQueueError::Query)?;
    if first.is_some_and(|seq| seq != 1) || (first.is_none() && snapshots > 0) {
        return Err(MergeQueueError::LogGap { queue, after: 0 });
    }
    for statement in [
        "DELETE FROM test_runs WHERE queue_id = $1",
        "DELETE FROM test_groups WHERE queue_id = $1",
        "DELETE FROM queue_items WHERE queue_id = $1",
        "DELETE FROM queue_snapshots WHERE queue_id = $1",
        "UPDATE queues SET state = 'running', mode = 'single' WHERE id = $1",
    ] {
        sqlx::query(statement)
            .bind(queue.to_string())
            .execute(&mut *conn)
            .await
            .map_err(MergeQueueError::Query)?;
    }
    let mut log = QueueLog {
        snapshot: empty(conn, queue).await?,
        seq: 0,
        uncovered_since: None,
    };
    for (seq, event) in load_events(conn, queue, 0)
        .await
        .map_err(MergeQueueError::Store)?
    {
        log.replay(&event)?;
        log.seq = seq;
        log.uncovered_since.get_or_insert(event.occurred_at);
        log.after_applied(conn, &event, now).await?;
    }
    Ok(log)
}

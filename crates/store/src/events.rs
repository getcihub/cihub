use std::error::Error;

use cihub_domain::{Actor, Event, EventId, EventKind, OrgId, QueueId, UserId};
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool};
use time::{Date, Month, OffsetDateTime};

use crate::StoreError;

/// Appends `events` to the logs of their queues, each after the last event of
/// its queue, and returns the `seq` of each. The last snapshot counts too: once
/// the partitions of the old events are dropped, `seq` goes on from it. The
/// caller holds the lock of each queue: two writers on the same queue would
/// take the same `seq`.
///
/// # Errors
///
/// An event cannot be serialized, or Postgres refuses a row (no partition for
/// its month, another organization than the context).
pub async fn append_events(
    conn: &mut PgConnection,
    events: &[Event],
) -> Result<Vec<i64>, StoreError> {
    let mut seqs = Vec::with_capacity(events.len());
    for event in events {
        let (payload, actor_user_id) = columns(event)?;
        let subjects = event.kind.subjects();
        let seq: i64 = sqlx::query_scalar(
            "INSERT INTO events (id, org_id, queue_id, seq, type, actor_kind, actor_user_id,
                                 cause_id, item_id, run_id, payload, occurred_at)
             SELECT $1, $2, $3,
                    greatest(
                        (SELECT coalesce(max(seq), 0) FROM events WHERE queue_id = $3),
                        (SELECT coalesce(snapshot_seq, 0) FROM queues WHERE id = $3)
                    ) + 1,
                    $4::event_type, $5::actor_kind, $6, $7, $8, $9, $10, $11
             RETURNING seq",
        )
        .bind(event.id.to_string())
        .bind(event.org_id.to_string())
        .bind(event.queue_id.to_string())
        .bind(event.kind.event_type().as_str())
        .bind(event.actor.kind().as_str())
        .bind(actor_user_id)
        .bind(event.cause_id.map(|id| id.to_string()))
        .bind(subjects.item_id.map(|id| id.to_string()))
        .bind(subjects.run_id.map(|id| id.to_string()))
        .bind(payload)
        .bind(event.occurred_at)
        .fetch_one(&mut *conn)
        .await
        .map_err(StoreError::Query)?;
        seqs.push(seq);
    }
    Ok(seqs)
}

/// # Errors
///
/// The columns do not match an event of `cihub-domain`.
pub fn event_kind(kind: &str, payload: &Value) -> Result<EventKind, serde_json::Error> {
    serde_json::from_value(json!({ "type": kind, "payload": payload }))
}

fn columns(event: &Event) -> Result<(Value, Option<String>), StoreError> {
    let serialized = serde_json::to_value(&event.kind).map_err(StoreError::WriteEvent)?;
    let payload = serialized.get("payload").cloned().unwrap_or(Value::Null);
    let actor_user_id = match &event.actor {
        Actor::User { user_id } => Some(user_id.to_string()),
        Actor::System | Actor::Bot => None,
    };
    Ok((payload, actor_user_id))
}

type EventRow = (
    i64,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Value,
    OffsetDateTime,
);

/// # Errors
///
/// Postgres fails, or a row does not match the events of `cihub-domain`.
pub async fn load_events(
    conn: &mut PgConnection,
    queue: QueueId,
    after: i64,
) -> Result<Vec<(i64, Event)>, StoreError> {
    let rows: Vec<EventRow> = sqlx::query_as(
        "SELECT seq, id, org_id, queue_id, type::text, actor_kind::text, actor_user_id, cause_id,
                payload, occurred_at
         FROM events WHERE queue_id = $1 AND seq > $2 ORDER BY seq",
    )
    .bind(queue.to_string())
    .bind(after)
    .fetch_all(conn)
    .await
    .map_err(StoreError::Query)?;
    rows.into_iter()
        .map(|row| {
            let id = row.1.clone();
            decode(row).map_err(StoreError::read("event", &id))
        })
        .collect()
}

fn decode(
    (seq, id, org_id, queue_id, kind, actor_kind, actor_user_id, cause_id, payload, occurred_at): EventRow,
) -> Result<(i64, Event), Box<dyn Error + Send + Sync>> {
    let actor = match (actor_kind.as_str(), actor_user_id) {
        ("system", None) => Actor::System,
        ("bot", None) => Actor::Bot,
        ("user", Some(user_id)) => Actor::User {
            user_id: user_id.parse::<UserId>()?,
        },
        (other, _) => return Err(format!("unknown actor `{other}`").into()),
    };
    let kind = event_kind(&kind, &payload)?;
    let event = Event {
        id: id.parse::<EventId>()?,
        org_id: org_id.parse::<OrgId>()?,
        queue_id: queue_id.parse::<QueueId>()?,
        actor,
        occurred_at,
        cause_id: cause_id.map(|id| id.parse::<EventId>()).transpose()?,
        kind,
    };
    Ok((seq, event))
}

/// Deposits the fact `event` in the inbox of its queue, and
/// returns `false` when the inbox still has it: a job that runs twice
/// deposits the same fact once. The scheduler applies it later under the lock
/// of the queue; the caller marks the queue dirty.
///
/// # Errors
///
/// `event` is a decision, it cannot be serialized, or Postgres refuses the row.
pub async fn deposit_fact(
    conn: &mut PgConnection,
    event: &Event,
    now: OffsetDateTime,
) -> Result<bool, StoreError> {
    if event.kind.event_type().is_decision() {
        return Err(StoreError::DecisionDeposited(event.id.to_string()));
    }
    let (payload, actor_user_id) = columns(event)?;
    let done = sqlx::query(
        "INSERT INTO queue_inbox (id, org_id, queue_id, actor_kind, actor_user_id, type, payload,
                                  received_at)
         VALUES ($1, $2, $3, $4::actor_kind, $5, $6::event_type, $7, $8)
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(event.id.to_string())
    .bind(event.org_id.to_string())
    .bind(event.queue_id.to_string())
    .bind(event.actor.kind().as_str())
    .bind(actor_user_id)
    .bind(event.kind.event_type().as_str())
    .bind(payload)
    .bind(now)
    .execute(conn)
    .await
    .map_err(StoreError::Query)?;
    Ok(done.rows_affected() == 1)
}

/// The facts of the inbox of `queue`, at most `limit`, in the order of their
/// deposits. The time of a fact is the time it came.
///
/// # Errors
///
/// Postgres fails, or a row does not match the events of `cihub-domain`.
pub async fn pending_facts(
    conn: &mut PgConnection,
    queue: QueueId,
    limit: i64,
) -> Result<Vec<Event>, StoreError> {
    let rows: Vec<EventRow> = sqlx::query_as(
        "SELECT 0::bigint, id, org_id, queue_id, type::text, actor_kind::text, actor_user_id,
                NULL::text, payload, received_at
         FROM queue_inbox
         WHERE queue_id = $1
         ORDER BY arrival
         LIMIT $2",
    )
    .bind(queue.to_string())
    .bind(limit)
    .fetch_all(conn)
    .await
    .map_err(StoreError::Query)?;
    rows.into_iter()
        .map(|row| {
            let id = row.1.clone();
            let (_, event) = decode(row).map_err(StoreError::read("event", &id))?;
            Ok(event)
        })
        .collect()
}

/// Deletes the facts `ids` from the inbox once applied,
/// in the transaction that writes them to the log.
///
/// # Errors
///
/// Postgres fails.
pub async fn delete_facts(conn: &mut PgConnection, ids: &[EventId]) -> Result<(), StoreError> {
    let ids: Vec<String> = ids.iter().map(ToString::to_string).collect();
    sqlx::query("DELETE FROM queue_inbox WHERE id = ANY($1)")
        .bind(ids)
        .execute(conn)
        .await
        .map_err(StoreError::Query)?;
    Ok(())
}

/// Creates the partitions of `events` for the month of `today` and the next
/// month, when they are missing.
///
/// # Errors
///
/// Postgres refuses to create a partition.
pub async fn create_event_partitions(pool: &PgPool, today: Date) -> Result<(), StoreError> {
    for month in [today, next_month(today)] {
        sqlx::query("SELECT cihub_create_events_partition($1)")
            .bind(month)
            .execute(pool)
            .await
            .map_err(StoreError::Query)?;
    }
    Ok(())
}

pub fn next_month(date: Date) -> Date {
    let (year, month) = match date.month() {
        Month::December => (date.year() + 1, Month::January),
        month => (date.year(), month.next()),
    };
    Date::from_calendar_date(year, month, 1).unwrap_or(date)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_month_rolls_over_the_year() {
        let date = |y, m, d| Date::from_calendar_date(y, m, d).unwrap();
        assert_eq!(
            next_month(date(2026, Month::September, 29)),
            date(2026, Month::October, 1)
        );
        assert_eq!(
            next_month(date(2026, Month::December, 31)),
            date(2027, Month::January, 1)
        );
    }
}

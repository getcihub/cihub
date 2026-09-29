use std::error::Error;

use cihub_domain::{Actor, Event, EventId, EventKind, OrgId, QueueId, UserId};
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool};
use time::{Date, Month, OffsetDateTime};

use crate::StoreError;

/// Appends `events` to the logs of their queues, each after the last event of
/// its queue. The caller holds the lock of each queue: two writers on the same
/// queue would take the same `seq`.
///
/// # Errors
///
/// An event cannot be serialized, or Postgres refuses a row (no partition for
/// its month, another organization than the context).
pub async fn append_events(conn: &mut PgConnection, events: &[Event]) -> Result<(), StoreError> {
    for event in events {
        let serialized = serde_json::to_value(&event.kind).map_err(StoreError::WriteEvent)?;
        let payload = serialized.get("payload").cloned().unwrap_or(Value::Null);
        let subjects = event.kind.subjects();
        let actor_user_id = match &event.actor {
            Actor::User { user_id } => Some(user_id.to_string()),
            Actor::System | Actor::Bot => None,
        };
        sqlx::query(
            "INSERT INTO events (id, org_id, queue_id, seq, type, actor_kind, actor_user_id,
                                 cause_id, item_id, group_id, run_id, payload, occurred_at)
             SELECT $1, $2, $3, coalesce(max(seq), 0) + 1, $4::event_type, $5::actor_kind, $6,
                    $7, $8, $9, $10, $11, $12
             FROM events WHERE queue_id = $3",
        )
        .bind(event.id.to_string())
        .bind(event.org_id.to_string())
        .bind(event.queue_id.to_string())
        .bind(event.kind.event_type().as_str())
        .bind(event.actor.kind().as_str())
        .bind(actor_user_id)
        .bind(event.cause_id.map(|id| id.to_string()))
        .bind(subjects.item_id.map(|id| id.to_string()))
        .bind(subjects.group_id.map(|id| id.to_string()))
        .bind(subjects.run_id.map(|id| id.to_string()))
        .bind(payload)
        .bind(event.occurred_at)
        .execute(&mut *conn)
        .await
        .map_err(StoreError::Query)?;
    }
    Ok(())
}

type EventRow = (
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

/// The log of `queue`, in order.
///
/// # Errors
///
/// Postgres fails, or a row does not match the events of `cihub-domain`.
pub async fn load_events(
    conn: &mut PgConnection,
    queue: QueueId,
) -> Result<Vec<Event>, StoreError> {
    let rows: Vec<EventRow> = sqlx::query_as(
        "SELECT id, org_id, queue_id, type::text, actor_kind::text, actor_user_id, cause_id,
                payload, occurred_at
         FROM events WHERE queue_id = $1 ORDER BY seq",
    )
    .bind(queue.to_string())
    .fetch_all(conn)
    .await
    .map_err(StoreError::Query)?;
    rows.into_iter()
        .map(|row| {
            let id = row.0.clone();
            decode(row).map_err(|source| StoreError::ReadEvent { id, source })
        })
        .collect()
}

fn decode(
    (id, org_id, queue_id, kind, actor_kind, actor_user_id, cause_id, payload, occurred_at): EventRow,
) -> Result<Event, Box<dyn Error + Send + Sync>> {
    let actor = match (actor_kind.as_str(), actor_user_id) {
        ("system", None) => Actor::System,
        ("bot", None) => Actor::Bot,
        ("user", Some(user_id)) => Actor::User {
            user_id: user_id.parse::<UserId>()?,
        },
        (other, _) => return Err(format!("unknown actor `{other}`").into()),
    };
    let kind: EventKind = serde_json::from_value(json!({ "type": kind, "payload": payload }))?;
    Ok(Event {
        id: id.parse::<EventId>()?,
        org_id: org_id.parse::<OrgId>()?,
        queue_id: queue_id.parse::<QueueId>()?,
        actor,
        occurred_at,
        cause_id: cause_id.map(|id| id.parse::<EventId>()).transpose()?,
        kind,
    })
}

/// Creates the partitions of `events` for the month of `today` and the next
/// month, when they are missing (`AGENTS.md` D36).
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

/// The first day of the month after `date`.
fn next_month(date: Date) -> Date {
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

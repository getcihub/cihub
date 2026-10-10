use std::error::Error;
use std::time::Duration;

use cihub_domain::{ConnectionId, JobClass, OrgId, QueueId, RunId, run_of_test_branch};
use cihub_scm::{ScmDelivery, ScmEvent};
use cihub_store::{PgPool, begin_system};
use serde_json::Value;
use sqlx::PgConnection;
use time::OffsetDateTime;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::connections::request_repo_sync;
use crate::inbox::{DeliveryRow, INBOX_CHANNEL, decode};
use crate::installations::{SYNC_AFTER, request_installation_sync};
use crate::jobs::hour_window;
use crate::wake::wake_for_delivery;
use crate::{Delivery, PlatformError, QueueWakeup};

pub const ROUTE_POLL: Duration = Duration::from_secs(5);

pub(crate) const ROUTES_PER_PASS: i64 = 100;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Route {
    pub org_id: Option<OrgId>,
    pub connection_id: Option<ConnectionId>,
    pub queue_id: Option<QueueId>,
    pub processed: bool,
    pub synced_at: Option<OffsetDateTime>,
}

fn processed_route() -> Route {
    Route {
        processed: true,
        ..Route::default()
    }
}

pub(crate) async fn unrouted_deliveries(
    conn: &mut PgConnection,
    limit: i64,
) -> Result<Vec<Delivery>, PlatformError> {
    let rows: Vec<DeliveryRow> = sqlx::query_as(
        "SELECT delivery_id, org_id, connection_id, queue_id, event, payload, received_at
         FROM webhook_inbox
         WHERE connection_id IS NULL
         ORDER BY received_at, delivery_id
         LIMIT $1
         FOR UPDATE SKIP LOCKED",
    )
    .bind(limit)
    .fetch_all(conn)
    .await
    .map_err(PlatformError::Query)?;
    rows.into_iter().map(decode).collect()
}

/// Where `delivery` goes: its connection when CIHub knows it, and its queue
/// when one is sure: a PR by its base branch, a push by its branch, a check
/// by the test branch that names its run. A delivery
/// without a known connection, and a change of the installation of a
/// connection, are processed at once: the sync of the installation reads the
/// SCM again, and the router deletes the delivery.
///
/// # Errors
///
/// Postgres fails, or a row does not match the identifiers of `cihub-domain`.
pub async fn find_route(
    conn: &mut PgConnection,
    delivery: &ScmDelivery,
) -> Result<Route, PlatformError> {
    if delivery.event == ScmEvent::Ignored {
        return Ok(processed_route());
    }

    let Some(external_id) = &delivery.connection_external_id else {
        return Ok(processed_route());
    };

    let connection: Option<(String, String, OffsetDateTime)> =
        sqlx::query_as("SELECT org_id, id, synced_at FROM scm_connections WHERE external_id = $1")
            .bind(external_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(PlatformError::Query)?;
    let Some((org_id, connection_id, synced_at)) = connection else {
        return Ok(processed_route());
    };

    let queue_id: Option<String> = match &delivery.event {
        ScmEvent::PrChanged {
            repo_external_id,
            base_branch: branch,
            ..
        }
        | ScmEvent::Pushed {
            repo_external_id,
            branch,
            ..
        } => queue_of_branch(conn, &connection_id, repo_external_id, branch).await?,
        ScmEvent::CheckChanged {
            repo_external_id,
            sha,
            branches,
            ..
        } => {
            let run_id = branches
                .iter()
                .find_map(|branch| run_of_test_branch(branch));
            queue_of_run(conn, &connection_id, repo_external_id, run_id, sha).await?
        }
        _ => None,
    };

    Ok(Route {
        org_id: Some(
            org_id
                .parse()
                .map_err(PlatformError::read("delivery", &org_id))?,
        ),
        connection_id: Some(
            connection_id
                .parse()
                .map_err(PlatformError::read("delivery", &connection_id))?,
        ),
        queue_id: queue_id
            .map(|id| id.parse().map_err(PlatformError::read("delivery", &id)))
            .transpose()?,
        processed: delivery.event == ScmEvent::ConnectionChanged,
        synced_at: Some(synced_at),
    })
}

fn installation_sync(
    delivery: &ScmDelivery,
    route: &Route,
    delivery_id: &str,
    now: OffsetDateTime,
) -> Option<(String, JobClass)> {
    if delivery.event == ScmEvent::Ignored {
        return None;
    }
    if delivery.event == ScmEvent::ConnectionChanged || route.connection_id.is_none() {
        return Some((delivery_id.to_owned(), JobClass::Interactive));
    }

    let synced_at = route.synced_at?;
    (synced_at < now - SYNC_AFTER).then(|| (hour_window(now), JobClass::Repair))
}

async fn queue_of_branch(
    conn: &mut PgConnection,
    connection_id: &str,
    repo_external_id: &str,
    branch: &str,
) -> Result<Option<String>, PlatformError> {
    sqlx::query_scalar(
        "SELECT min(q.id) FROM queues q JOIN repos r ON r.id = q.repo_id
         WHERE r.connection_id = $1 AND r.external_id = $2 AND q.target_branch = $3
         HAVING count(*) = 1",
    )
    .bind(connection_id)
    .bind(repo_external_id)
    .bind(branch)
    .fetch_optional(conn)
    .await
    .map_err(PlatformError::Query)
}

async fn queue_of_run(
    conn: &mut PgConnection,
    connection_id: &str,
    repo_external_id: &str,
    run_id: Option<RunId>,
    sha: &str,
) -> Result<Option<String>, PlatformError> {
    sqlx::query_scalar(
        "SELECT t.queue_id FROM test_runs t
         JOIN queues q ON q.id = t.queue_id
         JOIN repos r ON r.id = q.repo_id
         WHERE (t.id = $1 OR t.test_sha = $2) AND r.connection_id = $3 AND r.external_id = $4
         ORDER BY t.id = $1 DESC, t.started_at DESC
         LIMIT 1",
    )
    .bind(run_id.map(|id| id.to_string()))
    .bind(sha)
    .bind(connection_id)
    .bind(repo_external_id)
    .fetch_optional(conn)
    .await
    .map_err(PlatformError::Query)
}

pub(crate) async fn route_delivery(
    conn: &mut PgConnection,
    delivery_id: &str,
    route: Route,
) -> Result<bool, PlatformError> {
    if route.processed {
        let done = sqlx::query(
            "DELETE FROM webhook_inbox WHERE delivery_id = $1 AND connection_id IS NULL",
        )
        .bind(delivery_id)
        .execute(&mut *conn)
        .await
        .map_err(PlatformError::Query)?;
        return Ok(done.rows_affected() == 1);
    }

    let done = sqlx::query(
        "UPDATE webhook_inbox
         SET org_id = $2, connection_id = $3, queue_id = $4
         WHERE delivery_id = $1 AND connection_id IS NULL",
    )
    .bind(delivery_id)
    .bind(route.org_id.map(|id| id.to_string()))
    .bind(route.connection_id.map(|id| id.to_string()))
    .bind(route.queue_id.map(|id| id.to_string()))
    .execute(&mut *conn)
    .await
    .map_err(PlatformError::Query)?;
    let routed = done.rows_affected() == 1;

    if routed {
        wake_for_delivery(conn, route.queue_id).await?;
    }

    Ok(routed)
}

/// Routes the deliveries that wait, [`ROUTES_PER_PASS`] per transaction,
/// then waits for [`INBOX_CHANNEL`] or `idle_wait`, until cancellation. `translate`
/// reads a delivery of the SCM; a delivery that it cannot
/// read is deleted, with a warning.
///
/// # Errors
///
/// Postgres cannot be reached at the start, to listen to [`INBOX_CHANNEL`].
pub async fn run_router<T, E>(
    pool: PgPool,
    translate: T,
    clock: impl Fn() -> OffsetDateTime,
    idle_wait: Duration,
    cancel: CancellationToken,
) -> Result<(), PlatformError>
where
    T: Fn(&str, &Value) -> Result<ScmDelivery, E>,
    E: Error + 'static,
{
    let mut wakeup = QueueWakeup::listen_to(&pool, INBOX_CHANNEL).await?;
    while !cancel.is_cancelled() {
        match route_pass(&pool, &translate, clock()).await {
            Ok(routed) if routed == ROUTES_PER_PASS => continue,
            Ok(_) => {}
            Err(err) => warn!(error = &err as &dyn Error, "deliveries not routed"),
        }
        wakeup
            .wait_or_pause(idle_wait, &cancel, "inbox notifications lost")
            .await;
    }
    Ok(())
}

async fn route_pass<T, E>(
    pool: &PgPool,
    translate: &T,
    now: OffsetDateTime,
) -> Result<i64, PlatformError>
where
    T: Fn(&str, &Value) -> Result<ScmDelivery, E>,
    E: Error + 'static,
{
    let mut tx = begin_system(pool).await.map_err(PlatformError::Store)?;
    let deliveries = unrouted_deliveries(&mut tx, ROUTES_PER_PASS).await?;
    let count = i64::try_from(deliveries.len()).unwrap_or(i64::MAX);

    for delivery in deliveries {
        let delivery_id = delivery.delivery_id.as_str();
        let read = match translate(&delivery.event, &delivery.payload) {
            Ok(read) => Some(read),
            Err(err) => {
                warn!(
                    delivery_id,
                    error = &err as &dyn Error,
                    "delivery not translated"
                );
                None
            }
        };
        let route = match &read {
            Some(read) => find_route(&mut tx, read).await?,
            None => processed_route(),
        };

        let routed = route_delivery(&mut tx, delivery_id, route).await?;

        if let (true, Some(read)) = (routed, &read) {
            request_reads(&mut tx, read, route, delivery_id, now).await?;
        }

        info!(
            delivery_id,
            org_id = route.org_id.map(tracing::field::display),
            queue_id = route.queue_id.map(tracing::field::display),
            processed = route.processed,
            "delivery routed"
        );
    }

    tx.commit().await.map_err(PlatformError::Query)?;
    Ok(count)
}

async fn request_reads(
    conn: &mut PgConnection,
    delivery: &ScmDelivery,
    route: Route,
    delivery_id: &str,
    now: OffsetDateTime,
) -> Result<(), PlatformError> {
    let repos_changed = matches!(delivery.event, ScmEvent::ReposChanged { .. });
    if let (true, Some(org_id), Some(connection)) =
        (repos_changed, route.org_id, route.connection_id)
    {
        request_repo_sync(conn, org_id, connection, delivery_id, now).await?;
    }

    let Some(external_id) = &delivery.connection_external_id else {
        return Ok(());
    };

    let Some((cause, class)) = installation_sync(delivery, &route, delivery_id, now) else {
        return Ok(());
    };

    request_installation_sync(conn, external_id, &cause, class, now).await?;
    Ok(())
}

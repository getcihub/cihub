use std::error::Error;
use std::time::Duration;

use cihub_domain::{ConnectionId, OrgId, QueueId, RunId, ScmInstanceId, run_of_test_branch};
use cihub_scm::{ScmDelivery, ScmEvent};
use cihub_store::{PgPool, begin_system};
use serde_json::Value;
use sqlx::PgConnection;
use sqlx::postgres::PgListener;
use time::OffsetDateTime;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::wake::notify_connection_delivery;
use crate::{INBOX_CHANNEL, PlatformError, mark_dirty};

/// Without a notification, the routing loop looks for deliveries this often.
pub const ROUTE_POLL: Duration = Duration::from_secs(5);

/// A pass routes at most this many deliveries in one transaction.
pub const ROUTES_PER_PASS: i64 = 100;

/// Where a delivery goes (`AGENTS.md` D85, D86).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Route {
    /// `None` while the connection is unknown: only the system sees the
    /// delivery.
    pub org_id: Option<OrgId>,
    pub connection_id: Option<ConnectionId>,
    /// `Some` only when one queue is sure; otherwise the queues of the
    /// connection read the delivery (lot 5).
    pub queue_id: Option<QueueId>,
    /// Nothing is left to do: CIHub does not use the delivery, or cannot read
    /// it.
    pub processed: bool,
}

/// A delivery that waits for its routing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnroutedDelivery {
    pub scm_instance_id: ScmInstanceId,
    pub delivery_id: String,
    pub event: String,
    pub payload: Value,
}

/// The deliveries not routed yet, at most `limit`, in the order of reception.
/// Another transaction that routes at the same time skips them.
///
/// # Errors
///
/// Postgres fails, or a row does not match the identifiers of `cihub-domain`.
pub async fn unrouted_deliveries(
    conn: &mut PgConnection,
    limit: i64,
) -> Result<Vec<UnroutedDelivery>, PlatformError> {
    let rows: Vec<(String, String, String, Value)> = sqlx::query_as(
        "SELECT scm_instance_id, delivery_id, event, payload FROM webhook_inbox
         WHERE routed_at IS NULL
         ORDER BY received_at, delivery_id
         LIMIT $1
         FOR UPDATE SKIP LOCKED",
    )
    .bind(limit)
    .fetch_all(conn)
    .await
    .map_err(PlatformError::Query)?;
    rows.into_iter()
        .map(|(instance, delivery_id, event, payload)| {
            let scm_instance_id =
                instance
                    .parse()
                    .map_err(|source| PlatformError::ReadDelivery {
                        id: delivery_id.clone(),
                        source,
                    })?;
            Ok(UnroutedDelivery {
                scm_instance_id,
                delivery_id,
                event,
                payload,
            })
        })
        .collect()
}

/// Where `delivery` goes: its connection when CIHub knows it, and its queue
/// when one is sure: a PR by its base branch, a push by its branch, a check
/// by the test branch that names its run (`AGENTS.md` D85).
///
/// # Errors
///
/// Postgres fails, or a row does not match the identifiers of `cihub-domain`.
pub async fn find_route(
    conn: &mut PgConnection,
    scm_instance_id: ScmInstanceId,
    delivery: &ScmDelivery,
) -> Result<Route, PlatformError> {
    if delivery.event == ScmEvent::Ignored {
        return Ok(Route {
            processed: true,
            ..Route::default()
        });
    }
    let Some(external_id) = &delivery.connection_external_id else {
        return Ok(Route::default());
    };
    let connection: Option<(String, String)> = sqlx::query_as(
        "SELECT org_id, id FROM scm_connections WHERE scm_instance_id = $1 AND external_id = $2",
    )
    .bind(scm_instance_id.to_string())
    .bind(external_id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(PlatformError::Query)?;
    let Some((org_id, connection_id)) = connection else {
        return Ok(Route::default());
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
        } => queue_of_branch(conn, scm_instance_id, repo_external_id, branch).await?,
        ScmEvent::CheckChanged {
            repo_external_id,
            sha,
            branches,
            ..
        } => {
            let run_id = branches
                .iter()
                .find_map(|branch| run_of_test_branch(branch));
            queue_of_run(conn, scm_instance_id, repo_external_id, run_id, sha).await?
        }
        _ => None,
    };
    let read = |id: String| {
        move |source| PlatformError::ReadDelivery {
            id: id.clone(),
            source,
        }
    };
    Ok(Route {
        org_id: Some(org_id.parse().map_err(read(org_id.clone()))?),
        connection_id: Some(connection_id.parse().map_err(read(connection_id.clone()))?),
        queue_id: queue_id
            .map(|id| id.parse().map_err(read(id.clone())))
            .transpose()?,
        processed: false,
    })
}

/// The queue of the branch `branch` of a repository, when the branch has
/// only one (`AGENTS.md` D135).
async fn queue_of_branch(
    conn: &mut PgConnection,
    scm_instance_id: ScmInstanceId,
    repo_external_id: &str,
    branch: &str,
) -> Result<Option<String>, PlatformError> {
    sqlx::query_scalar(
        "SELECT min(q.id) FROM queues q JOIN repos r ON r.id = q.repo_id
         WHERE r.scm_instance_id = $1 AND r.external_id = $2 AND q.target_branch = $3
         HAVING count(*) = 1",
    )
    .bind(scm_instance_id.to_string())
    .bind(repo_external_id)
    .bind(branch)
    .fetch_optional(conn)
    .await
    .map_err(PlatformError::Query)
}

/// The queue of the run that a check on a repository reports on: the run
/// named by the test branch, or, once the branch is gone, the last run whose
/// test SHA is `sha` (a late report, `AGENTS.md` D118).
async fn queue_of_run(
    conn: &mut PgConnection,
    scm_instance_id: ScmInstanceId,
    repo_external_id: &str,
    run_id: Option<RunId>,
    sha: &str,
) -> Result<Option<String>, PlatformError> {
    sqlx::query_scalar(
        "SELECT t.queue_id FROM test_runs t
         JOIN queues q ON q.id = t.queue_id
         JOIN repos r ON r.id = q.repo_id
         WHERE (t.id = $1 OR t.test_sha = $2) AND r.scm_instance_id = $3 AND r.external_id = $4
         ORDER BY t.id = $1 DESC, t.started_at DESC
         LIMIT 1",
    )
    .bind(run_id.map(|id| id.to_string()))
    .bind(sha)
    .bind(scm_instance_id.to_string())
    .bind(repo_external_id)
    .fetch_optional(conn)
    .await
    .map_err(PlatformError::Query)
}

/// Writes `route` on a delivery not routed yet, and marks its queue dirty,
/// or wakes the schedulers for a delivery of a connection without a queue.
/// Returns `false` when the delivery was already routed.
///
/// # Errors
///
/// Postgres fails.
pub async fn route_delivery(
    conn: &mut PgConnection,
    scm_instance_id: ScmInstanceId,
    delivery_id: &str,
    route: Route,
    now: OffsetDateTime,
) -> Result<bool, PlatformError> {
    let done = sqlx::query(
        "UPDATE webhook_inbox
         SET org_id = $3, connection_id = $4, queue_id = $5, routed_at = $6,
             processed_at = CASE WHEN $7 THEN $6 END
         WHERE scm_instance_id = $1 AND delivery_id = $2 AND routed_at IS NULL",
    )
    .bind(scm_instance_id.to_string())
    .bind(delivery_id)
    .bind(route.org_id.map(|id| id.to_string()))
    .bind(route.connection_id.map(|id| id.to_string()))
    .bind(route.queue_id.map(|id| id.to_string()))
    .bind(now)
    .bind(route.processed)
    .execute(&mut *conn)
    .await
    .map_err(PlatformError::Query)?;
    let routed = done.rows_affected() == 1;
    match route {
        _ if !routed || route.processed => {}
        Route {
            queue_id: Some(queue),
            ..
        } => mark_dirty(conn, queue).await?,
        Route {
            org_id: Some(_), ..
        } => notify_connection_delivery(conn).await?,
        Route { .. } => {}
    }
    Ok(routed)
}

/// Routes the deliveries that wait, [`ROUTES_PER_PASS`] per transaction,
/// then waits for [`INBOX_CHANNEL`] or `idle_wait`, until cancellation. `translate`
/// reads a delivery of the SCM (`AGENTS.md` D84); a delivery that it cannot
/// read is routed as processed, with a warning.
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
    let mut listener = PgListener::connect_with(&pool)
        .await
        .map_err(PlatformError::Query)?;
    listener
        .listen(INBOX_CHANNEL)
        .await
        .map_err(PlatformError::Query)?;
    while !cancel.is_cancelled() {
        match route_pass(&pool, &translate, clock()).await {
            Ok(routed) if routed == ROUTES_PER_PASS => continue,
            Ok(_) => {}
            Err(err) => warn!(error = &err as &dyn Error, "deliveries not routed"),
        }
        tokio::select! {
            () = cancel.cancelled() => {}
            received = listener.recv() => {
                if let Err(err) = received {
                    warn!(error = &err as &dyn Error, "inbox notifications lost");
                    tokio::select! {
                        () = cancel.cancelled() => {}
                        () = tokio::time::sleep(idle_wait) => {}
                    }
                }
            }
            () = tokio::time::sleep(idle_wait) => {}
        }
    }
    Ok(())
}

/// Routes one batch of deliveries in one transaction, and returns its size.
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
        let route = match translate(&delivery.event, &delivery.payload) {
            Ok(read) => find_route(&mut tx, delivery.scm_instance_id, &read).await?,
            Err(err) => {
                warn!(
                    delivery_id,
                    error = &err as &dyn Error,
                    "delivery not translated"
                );
                Route {
                    processed: true,
                    ..Route::default()
                }
            }
        };
        route_delivery(&mut tx, delivery.scm_instance_id, delivery_id, route, now).await?;
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

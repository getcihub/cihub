use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use cihub_domain::{ConnectionId, ConnectionMode};
use cihub_store::begin_system;
use sqlx::{PgConnection, PgPool};
use time::OffsetDateTime;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::PlatformError;

/// A process writes the budgets of its connections at most this often, except
/// a pause or a change of mode (`DECISIONS.md` D77).
pub const BUDGET_WRITE_EVERY: Duration = Duration::from_secs(5);

/// The budget of a connection as a process sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionBudget {
    pub mode: ConnectionMode,
    /// Why the connection left `normal`, for the operator.
    pub mode_reason: Option<String>,
    /// The rate limit with the smallest share left, when the SCM gave one.
    pub rate_remaining: Option<u32>,
    pub rate_limit: Option<u32>,
    pub rate_reset_at: Option<OffsetDateTime>,
    /// The jobs of the connection wait until then (`DECISIONS.md` D70).
    pub paused_until: Option<OffsetDateTime>,
    pub consecutive_failures: u32,
}

/// Writes `budgets` on their connections. A pause shorter than the one in
/// the table does not shorten it: another process may have seen a longer
/// one. Runs in the system context: a process serves every organization.
///
/// # Errors
///
/// Postgres fails.
pub async fn record_budgets(
    conn: &mut PgConnection,
    budgets: &[(ConnectionId, ConnectionBudget)],
) -> Result<(), PlatformError> {
    let column = |n: Option<u32>| n.map(|n| i32::try_from(n).unwrap_or(i32::MAX));
    for (connection, budget) in budgets {
        sqlx::query(
            "UPDATE scm_connections
             SET mode = $2::connection_mode, mode_reason = $3, rate_remaining = $4,
                 rate_limit = $5, rate_reset_at = $6,
                 paused_until = greatest(paused_until, $7), consecutive_failures = $8
             WHERE id = $1",
        )
        .bind(connection.to_string())
        .bind(budget.mode.as_str())
        .bind(&budget.mode_reason)
        .bind(column(budget.rate_remaining))
        .bind(column(budget.rate_limit))
        .bind(budget.rate_reset_at)
        .bind(budget.paused_until)
        .bind(column(Some(budget.consecutive_failures)))
        .execute(&mut *conn)
        .await
        .map_err(PlatformError::Query)?;
    }
    Ok(())
}

/// Writes the budgets that `changed` returns every [`BUDGET_WRITE_EVERY`],
/// and at once when `urgent` is notified, until cancellation. A write that
/// fails is tried again with the next one.
pub async fn run_budget_writer(
    pool: PgPool,
    changed: impl Fn() -> Vec<(ConnectionId, ConnectionBudget)>,
    urgent: Arc<Notify>,
    cancel: CancellationToken,
) {
    let mut every = tokio::time::interval(BUDGET_WRITE_EVERY);
    let mut pending = HashMap::new();
    loop {
        tokio::select! {
            () = cancel.cancelled() => return,
            _ = every.tick() => {}
            () = urgent.notified() => {}
        }
        pending.extend(changed());
        if pending.is_empty() {
            continue;
        }
        let budgets: Vec<_> = pending.iter().map(|(id, b)| (*id, b.clone())).collect();
        match write(&pool, &budgets).await {
            Ok(()) => pending.clear(),
            Err(err) => warn!(error = &err as &dyn Error, "budgets not recorded"),
        }
    }
}

async fn write(
    pool: &PgPool,
    budgets: &[(ConnectionId, ConnectionBudget)],
) -> Result<(), PlatformError> {
    let mut tx = begin_system(pool).await.map_err(PlatformError::Store)?;
    record_budgets(&mut tx, budgets).await?;
    tx.commit().await.map_err(PlatformError::Query)
}

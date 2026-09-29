//! `cihub-server`: reads the `CIHUB_*` configuration, starts the logs, checks
//! that the database has every embedded migration, serves `/healthz` and
//! `/readyz` on the internal address, and runs the roles of the process until
//! `SIGTERM` or `SIGINT` (`AGENTS.md` D8). `cihub-server migrate` applies the
//! missing migrations and stops (`AGENTS.md` D14). It holds no queue logic: a
//! role runs loops whose code lives in the library crates.

mod config;
mod health;

use std::collections::BTreeSet;
use std::net::SocketAddr;

use anyhow::{Context, anyhow, bail};
use cihub_store::{MIGRATOR, PgPool};
use tokio::net::TcpListener;
use tokio::signal::unix::{SignalKind, signal};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, info, info_span};

use crate::config::{Config, Role};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let migrate_only = match std::env::args().nth(1).as_deref() {
        None => false,
        Some("migrate") => true,
        Some(other) => bail!("unknown command `{other}`: expected no command or `migrate`"),
    };
    let Config {
        roles,
        internal_addr,
        database_url,
        logs,
    } = Config::from_env()?;
    logs.init()?;
    let pool = cihub_store::connect(&database_url).await?;
    if migrate_only {
        migrate(&pool).await
    } else {
        serve(roles, internal_addr, pool).await
    }
}

async fn migrate(pool: &PgPool) -> anyhow::Result<()> {
    let applied = cihub_store::migrate(&MIGRATOR, pool).await?;
    for version in &applied {
        info!(version, "migration applied");
    }
    info!(applied = applied.len(), "migrations done");
    Ok(())
}

async fn serve(
    roles: BTreeSet<Role>,
    internal_addr: SocketAddr,
    pool: PgPool,
) -> anyhow::Result<()> {
    let missing = cihub_store::missing_migrations(&MIGRATOR, &pool).await?;
    if !missing.is_empty() {
        bail!("the database misses the migrations {missing:?}: run `cihub-server migrate`");
    }
    let listener = TcpListener::bind(internal_addr)
        .await
        .with_context(|| format!("cannot listen on {internal_addr}"))?;
    info!(internal_addr = %listener.local_addr()?, "server started");

    let cancel = CancellationToken::new();
    let mut tasks = JoinSet::new();
    tasks.spawn(health::serve(listener, roles.clone(), pool, cancel.clone()));
    for role in &roles {
        let span = info_span!("role", role = role.as_str());
        tasks.spawn(run_role(cancel.clone()).instrument(span));
    }
    let result = supervise(tasks, &cancel).await;
    info!("server stopped");
    result
}

/// Waits for cancellation: the loops of each role arrive with their lot.
async fn run_role(cancel: CancellationToken) -> anyhow::Result<()> {
    info!("role started");
    cancel.cancelled().await;
    info!("role stopped");
    Ok(())
}

/// Cancels every task on `SIGTERM`, on `SIGINT`, or when one task stops on its
/// own, then waits for all of them. Returns the first error.
async fn supervise(
    mut tasks: JoinSet<anyhow::Result<()>>,
    cancel: &CancellationToken,
) -> anyhow::Result<()> {
    let mut terminate = signal(SignalKind::terminate()).context("cannot watch SIGTERM")?;
    let mut interrupt = signal(SignalKind::interrupt()).context("cannot watch SIGINT")?;
    let mut first_error = None;
    loop {
        tokio::select! {
            _ = terminate.recv(), if !cancel.is_cancelled() => {
                info!(signal = "SIGTERM", "shutdown requested");
                cancel.cancel();
            }
            _ = interrupt.recv(), if !cancel.is_cancelled() => {
                info!(signal = "SIGINT", "shutdown requested");
                cancel.cancel();
            }
            joined = tasks.join_next() => {
                let Some(joined) = joined else { break };
                let result = joined.map_err(anyhow::Error::from).and_then(|result| result);
                let result = if cancel.is_cancelled() {
                    result
                } else {
                    cancel.cancel();
                    result.and(Err(anyhow!("a task stopped before shutdown")))
                };
                if let Err(err) = result {
                    first_error.get_or_insert(err);
                }
            }
        }
    }
    first_error.map_or(Ok(()), Err)
}

//! `cihub-server`: reads the `CIHUB_*` configuration, starts the logs, serves
//! `/healthz` and `/readyz` on the internal address, and runs the roles of the
//! process until `SIGTERM` or `SIGINT` (`AGENTS.md` D8). It holds no queue
//! logic: a role runs loops whose code lives in the library crates.

mod config;
mod health;

use anyhow::{Context, anyhow};
use tokio::net::TcpListener;
use tokio::signal::unix::{SignalKind, signal};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, info, info_span};

use crate::config::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::from_env()?;
    config.logs.init()?;
    let listener = TcpListener::bind(config.internal_addr)
        .await
        .with_context(|| format!("cannot listen on {}", config.internal_addr))?;
    info!(internal_addr = %listener.local_addr()?, "server started");

    let cancel = CancellationToken::new();
    let mut tasks = JoinSet::new();
    tasks.spawn(health::serve(
        listener,
        config.roles.clone(),
        cancel.clone(),
    ));
    for role in &config.roles {
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

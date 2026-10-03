//! `cihub server start [<role>...]`: serves `/healthz` and `/readyz` on the
//! internal address, and runs the given roles in one process until `SIGTERM`
//! or `SIGINT` (`DECISIONS.md` D168). The roles of one process share only the
//! Postgres pool.

use std::collections::BTreeSet;
use std::net::SocketAddr;

use anyhow::{Context, anyhow};
use cihub_store::PgPool;
use tokio::net::TcpListener;
use tokio::signal::unix::{SignalKind, signal};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, info, info_span};

use super::{Github, Role, health, notifier, scheduler, web, worker};

/// What the roles of a process share.
#[derive(Clone)]
pub(super) struct Shared {
    pub pool: PgPool,
    /// A name unique to this process, which owns its jobs and leases.
    pub process: String,
    pub http_addr: SocketAddr,
    pub github: Github,
    pub cancel: CancellationToken,
}

pub(super) async fn run(
    roles: BTreeSet<Role>,
    internal_addr: SocketAddr,
    http_addr: SocketAddr,
    pool: PgPool,
    github: Github,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(internal_addr)
        .await
        .with_context(|| format!("cannot listen on {internal_addr}"))?;
    info!(internal_addr = %listener.local_addr()?, "server started");

    let cancel = CancellationToken::new();
    let mut tasks = JoinSet::new();
    tasks.spawn(health::serve(
        listener,
        roles.clone(),
        pool.clone(),
        cancel.clone(),
    ));
    let shared = Shared {
        pool,
        process: process_name(),
        http_addr,
        github,
        cancel: cancel.clone(),
    };
    for role in &roles {
        let span = info_span!("role", role = role.as_str());
        tasks.spawn(run_role(*role, shared.clone()).instrument(span));
    }
    let result = supervise(tasks, &cancel).await;
    info!("server stopped");
    result
}

/// Runs the loops of `role` until cancellation.
async fn run_role(role: Role, shared: Shared) -> anyhow::Result<()> {
    info!("role started");
    match role {
        Role::Web => web::run(shared).await?,
        Role::Scheduler => scheduler::run(shared).await?,
        Role::Worker => worker::run(shared).await?,
        Role::Notifier => notifier::run(shared).await,
    }
    info!("role stopped");
    Ok(())
}

fn process_name() -> String {
    #[expect(
        clippy::disallowed_methods,
        reason = "the one place where the server draws a random name"
    )]
    let id = ulid::Ulid::generate();
    format!("cihub-{}", id.to_string().to_lowercase())
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

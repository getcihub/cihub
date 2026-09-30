//! `cihub-server`: reads the `CIHUB_*` configuration, starts the logs, checks
//! that the database has every embedded migration, serves `/healthz` and
//! `/readyz` on the internal address, and runs the roles of the process until
//! `SIGTERM` or `SIGINT` (`AGENTS.md` D8). `cihub-server migrate` applies the
//! missing migrations and stops (`AGENTS.md` D14). It holds no queue logic: a
//! role runs loops whose code lives in the library crates.

mod config;
mod health;

use std::collections::BTreeSet;
use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, anyhow, bail};
use cihub_platform::{Executors, JOB_POLL, MAINTENANCE, execute_maintenance, request_maintenance};
use cihub_store::{MIGRATOR, PgPool, begin_system};
use time::OffsetDateTime;
use tokio::net::TcpListener;
use tokio::signal::unix::{SignalKind, signal};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, info, info_span, warn};

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
    cihub_store::check_role(&pool).await?;
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
    cihub_store::create_event_partitions(pool, now().date()).await?;
    info!("event partitions ready");
    Ok(())
}

/// The clock of the process: the library crates receive the time from here.
fn now() -> OffsetDateTime {
    #[expect(
        clippy::disallowed_methods,
        reason = "the one place where the server reads the clock"
    )]
    OffsetDateTime::now_utc()
}

/// A name unique to this process, which owns its jobs and leases.
fn process_name() -> String {
    #[expect(
        clippy::disallowed_methods,
        reason = "the one place where the server draws a random name"
    )]
    let id = ulid::Ulid::generate();
    format!("cihub-{}", id.to_string().to_lowercase())
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
    tasks.spawn(health::serve(
        listener,
        roles.clone(),
        pool.clone(),
        cancel.clone(),
    ));
    let process = process_name();
    for role in &roles {
        let span = info_span!("role", role = role.as_str());
        tasks
            .spawn(run_role(*role, pool.clone(), process.clone(), cancel.clone()).instrument(span));
    }
    let result = supervise(tasks, &cancel).await;
    info!("server stopped");
    result
}

/// Runs the loops of `role` until cancellation. The other loops of each role
/// arrive with their lot.
async fn run_role(
    role: Role,
    pool: PgPool,
    process: String,
    cancel: CancellationToken,
) -> anyhow::Result<()> {
    info!("role started");
    match role {
        Role::Scheduler => request_maintenance_hourly(&pool, &cancel).await,
        Role::Worker => {
            let mut executors = Executors::default();
            executors.register(MAINTENANCE, execute_maintenance);
            cihub_platform::run_worker(pool, process, Arc::new(executors), now, JOB_POLL, cancel)
                .await?;
        }
        Role::Api | Role::Notifier => cancel.cancelled().await,
    }
    info!("role stopped");
    Ok(())
}

/// Requests the maintenance job of the day at the start, then every hour
/// (`AGENTS.md` D67). Each replica does it; the key of the day admits one job.
async fn request_maintenance_hourly(pool: &PgPool, cancel: &CancellationToken) {
    let mut every_hour = tokio::time::interval(Duration::from_hours(1));
    loop {
        tokio::select! {
            () = cancel.cancelled() => return,
            _ = every_hour.tick() => {
                if let Err(err) = request_maintenance_now(pool).await {
                    warn!(error = &*err as &dyn Error, "maintenance not requested");
                }
            }
        }
    }
}

async fn request_maintenance_now(pool: &PgPool) -> anyhow::Result<()> {
    let mut tx = begin_system(pool).await?;
    if request_maintenance(&mut tx, now()).await? {
        info!("maintenance requested");
    }
    tx.commit().await?;
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

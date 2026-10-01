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
use cihub_domain::{ConnectionId, ScmInstanceId, ScmKind};
use cihub_github::{GithubApp, GithubConfig};
use cihub_platform::{
    ConnectionBudget, Executors, JOB_POLL, MAINTENANCE, ROUTE_POLL, SWEEP_DELIVERIES, SWEEP_EVERY,
    execute_maintenance, request_maintenance, request_sweep, run_budget_writer, run_router,
    sweep_executor,
};
use cihub_scm::Scm;
use cihub_store::{MIGRATOR, PgPool, begin_system, ensure_scm_instance};
use cihub_web::GithubWebhooks;
use secrecy::SecretString;
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
        http_addr,
        database_url,
        logs,
        github,
    } = Config::from_env()?;
    logs.init()?;
    let pool = cihub_store::connect(&database_url).await?;
    cihub_store::check_role(&pool).await?;
    if migrate_only {
        migrate(&pool).await
    } else {
        // `migrate` does not talk to GitHub; the server cannot run without it.
        let Some(github) = github else {
            bail!(
                "GitHub is not configured: set CIHUB_GITHUB_APP_ID, CIHUB_GITHUB_PRIVATE_KEY or \
                 CIHUB_GITHUB_PRIVATE_KEY_FILE, and CIHUB_GITHUB_WEBHOOK_SECRET (docs/GITHUB.md)"
            );
        };
        serve(roles, internal_addr, http_addr, pool, github).await
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

/// What the roles of a process share.
#[derive(Clone)]
struct Shared {
    pool: PgPool,
    process: String,
    http_addr: SocketAddr,
    github: Github,
    cancel: CancellationToken,
}

/// The GitHub App of the instance, once started.
#[derive(Clone)]
struct Github {
    app: Arc<GithubApp>,
    scm_instance_id: ScmInstanceId,
    webhook_secret: SecretString,
}

async fn serve(
    roles: BTreeSet<Role>,
    internal_addr: SocketAddr,
    http_addr: SocketAddr,
    pool: PgPool,
    github: config::GithubApp,
) -> anyhow::Result<()> {
    let missing = cihub_store::missing_migrations(&MIGRATOR, &pool).await?;
    if !missing.is_empty() {
        bail!("the database misses the migrations {missing:?}: run `cihub-server migrate`");
    }
    let github = start_github(&pool, &github).await?;
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

/// Runs the loops of `role` until cancellation. The other loops of each role
/// arrive with their lot.
async fn run_role(role: Role, shared: Shared) -> anyhow::Result<()> {
    let Shared {
        pool,
        process,
        http_addr,
        github,
        cancel,
    } = shared;
    info!("role started");
    match role {
        Role::Web => {
            let listener = TcpListener::bind(http_addr)
                .await
                .with_context(|| format!("cannot listen on {http_addr}"))?;
            info!(http_addr = %listener.local_addr()?, "public address served");
            let webhooks = GithubWebhooks {
                scm_instance_id: github.scm_instance_id,
                secret: github.webhook_secret,
            };
            let router = cihub_web::router(pool.clone(), now, webhooks);
            let serve = async {
                cihub_web::serve(listener, router, cancel.clone())
                    .await
                    .context("public address server failed")
            };
            let route = async {
                run_router(
                    pool,
                    cihub_github::translate,
                    now,
                    ROUTE_POLL,
                    cancel.clone(),
                )
                .await
                .context("the routing of the deliveries cannot start")
            };
            tokio::try_join!(serve, route)?;
        }
        Role::Scheduler => {
            let maintenance = request_maintenance_hourly(&pool, &cancel);
            let sweep = request_sweep_regularly(&pool, github.scm_instance_id, &cancel);
            tokio::join!(maintenance, sweep);
        }
        Role::Worker => {
            let mut executors = Executors::default();
            executors.register(MAINTENANCE, execute_maintenance);
            let scm: Arc<dyn Scm> = github.app.clone();
            executors.register(
                SWEEP_DELIVERIES,
                sweep_executor(scm, github.scm_instance_id),
            );
            let worker = cihub_platform::run_worker(
                pool.clone(),
                process,
                Arc::new(executors),
                now,
                JOB_POLL,
                cancel.clone(),
            );
            let app = github.app;
            let urgent = app.budget_urgent();
            let changed = move || connection_budgets(&app);
            let writer = run_budget_writer(pool, changed, urgent, cancel);
            tokio::join!(worker, writer).0?;
        }
        Role::Notifier => cancel.cancelled().await,
    }
    info!("role stopped");
    Ok(())
}

/// Starts the client of the GitHub App, and creates the row of its SCM
/// instance when it is missing (`AGENTS.md` D79).
async fn start_github(pool: &PgPool, app: &config::GithubApp) -> anyhow::Result<Github> {
    let url = app.url.as_str();
    let config = GithubConfig {
        url: url.to_owned(),
        app_id: app.app_id.clone(),
        private_key: app.private_key.clone(),
    };
    let client = GithubApp::new(&config, Arc::new(now)).context("the GitHub App cannot start")?;
    #[expect(
        clippy::disallowed_methods,
        reason = "the identifier of the SCM instance, used only when it is new"
    )]
    let candidate = ScmInstanceId::new();
    let mut tx = begin_system(pool).await?;
    let scm_instance_id = ensure_scm_instance(&mut tx, candidate, ScmKind::Github, url).await?;
    tx.commit().await?;
    info!(url, app_id = app.app_id, "GitHub App configured");
    Ok(Github {
        app: Arc::new(client),
        scm_instance_id,
        webhook_secret: app.webhook_secret.clone(),
    })
}

/// The budgets that changed in the GitHub App, as `scm_connections` keeps
/// them: the rate limit with the smallest share left (`AGENTS.md` D77).
fn connection_budgets(app: &GithubApp) -> Vec<(ConnectionId, ConnectionBudget)> {
    app.take_changed_budgets()
        .into_iter()
        .map(|(connection, budget)| {
            let rate = budget.lowest();
            let recorded = ConnectionBudget {
                mode: budget.mode,
                mode_reason: budget.mode_reason,
                rate_remaining: rate.map(|rate| rate.remaining),
                rate_limit: rate.map(|rate| rate.limit),
                rate_reset_at: rate.map(|rate| rate.reset_at),
                paused_until: budget.paused_until,
                consecutive_failures: budget.failures,
            };
            (connection, recorded)
        })
        .collect()
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

/// Requests the sweep of the deliveries of `instance` at the start, then
/// every [`SWEEP_EVERY`] (`AGENTS.md` D88). Each replica does it; the key of
/// the window admits one job.
async fn request_sweep_regularly(
    pool: &PgPool,
    instance: ScmInstanceId,
    cancel: &CancellationToken,
) {
    let mut every = tokio::time::interval(SWEEP_EVERY);
    loop {
        tokio::select! {
            () = cancel.cancelled() => return,
            _ = every.tick() => {
                if let Err(err) = request_sweep_now(pool, instance).await {
                    warn!(error = &*err as &dyn Error, "sweep not requested");
                }
            }
        }
    }
}

async fn request_sweep_now(pool: &PgPool, instance: ScmInstanceId) -> anyhow::Result<()> {
    let mut tx = begin_system(pool).await?;
    if request_sweep(&mut tx, instance, now()).await? {
        info!("sweep of the deliveries requested");
    }
    tx.commit().await?;
    Ok(())
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

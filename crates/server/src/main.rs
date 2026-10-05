//! `cihub-server`: reads the `CIHUB_*` configuration, starts the logs, checks
//! that the database has every embedded migration, serves `/healthz` and
//! `/readyz` on the internal address, and runs the four roles in one process
//! until `SIGTERM` or `SIGINT` (`DECISIONS.md` D8, D168). `cihub-server
//! migrate` applies the missing migrations and stops (`DECISIONS.md` D14).
//! `cihub-server admin bootstrap --installation <id>` starts an organization
//! from an installation of the GitHub App, shows its API token once, and stops
//! (`DECISIONS.md` D154).
//! It holds no queue logic: a role runs loops whose code lives in the library
//! crates.

mod config;
mod health;

use std::error::Error;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, anyhow, bail};
use cihub_domain::{ConnectionId, ConnectionKind, ScmInstanceId, ScmKind};
use cihub_github::GithubApp;
use cihub_mergequeue::Translate;
use cihub_platform::{
    Bootstrapped, ConnectionBudget, Executors, JOB_POLL, MAINTENANCE, ROUTE_POLL, SWEEP_DELIVERIES,
    SWEEP_EVERY, SYNC_REPOS, execute_maintenance, request_maintenance, request_repo_syncs,
    request_sweep, run_budget_writer, run_router, sweep_executor, sync_repos_executor,
};
use cihub_scm::Scm;
use cihub_store::{MIGRATOR, PgPool, Postgres, Transaction, begin_system, ensure_scm_instance};
use cihub_web::GithubWebhooks;
use secrecy::{ExposeSecret, SecretString};
use time::OffsetDateTime;
use tokio::net::TcpListener;
use tokio::signal::unix::{SignalKind, signal};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, info, info_span, warn};

use crate::config::Config;

/// What the process does, from its arguments.
enum Command {
    Serve,
    Migrate,
    /// Starts an organization from the installation of the GitHub App.
    Bootstrap {
        installation: String,
    },
}

impl Command {
    fn parse(args: &[String]) -> anyhow::Result<Self> {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        match args.as_slice() {
            [] => Ok(Self::Serve),
            ["migrate"] => Ok(Self::Migrate),
            ["admin", "bootstrap", "--installation", installation] => Ok(Self::Bootstrap {
                installation: (*installation).to_owned(),
            }),
            _ => bail!(
                "unknown command `{}`: expected no command, `migrate`, or \
                 `admin bootstrap --installation <id>`",
                args.join(" ")
            ),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = Command::parse(&args)?;
    let Config {
        internal_addr,
        http_addr,
        database_url,
        logs,
        github,
    } = Config::from_env()?;
    logs.init()?;
    let pool = cihub_store::connect(&database_url).await?;
    cihub_store::check_role(&pool).await?;
    match command {
        Command::Migrate => migrate(&pool).await,
        Command::Bootstrap { installation } => {
            let github = start_github(&pool, github).await?;
            bootstrap(&pool, &github, &installation).await
        }
        Command::Serve => {
            let github = start_github(&pool, github).await?;
            serve(internal_addr, http_addr, pool, github).await
        }
    }
}

/// Checks that the database is migrated, then starts the client of the
/// GitHub App and creates the row of its SCM instance when it is missing
/// (`DECISIONS.md` D79). `migrate` does not talk to GitHub; the other
/// commands cannot run without it.
async fn start_github(pool: &PgPool, github: Option<config::GithubApp>) -> anyhow::Result<Github> {
    let Some(github) = github else {
        bail!(
            "GitHub is not configured: set CIHUB_GITHUB_APP_ID, CIHUB_GITHUB_PRIVATE_KEY or \
             CIHUB_GITHUB_PRIVATE_KEY_FILE, and CIHUB_GITHUB_WEBHOOK_SECRET (docs/GITHUB.md)"
        );
    };
    let missing = cihub_store::missing_migrations(&MIGRATOR, pool).await?;
    if !missing.is_empty() {
        bail!("the database misses the migrations {missing:?}: run `cihub-server migrate`");
    }
    let config::GithubApp {
        config,
        webhook_secret,
    } = github;
    let client = GithubApp::new(&config, Arc::new(now)).context("the GitHub App cannot start")?;
    #[expect(
        clippy::disallowed_methods,
        reason = "the identifier of the SCM instance, used only when it is new"
    )]
    let candidate = ScmInstanceId::new();
    let mut tx = begin_system(pool).await?;
    let scm_instance_id =
        ensure_scm_instance(&mut tx, candidate, ScmKind::Github, &config.url).await?;
    tx.commit().await?;
    info!(
        url = config.url,
        app_id = config.app_id,
        "GitHub App configured"
    );
    Ok(Github {
        app: Arc::new(client),
        scm_instance_id,
        webhook_secret,
    })
}

/// Starts the organization of `installation`, and shows its API token once
/// (`DECISIONS.md` D154, D155).
async fn bootstrap(pool: &PgPool, github: &Github, installation: &str) -> anyhow::Result<()> {
    let Bootstrapped {
        org_id,
        connection_id,
        created,
        token,
    } = cihub_platform::bootstrap(
        pool,
        github.app.as_ref(),
        github.scm_instance_id,
        ConnectionKind::GithubApp,
        installation,
        now(),
    )
    .await
    .with_context(|| format!("the installation {installation} cannot start an organization"))?;
    info!(%org_id, %connection_id, created, "organization bootstrapped");
    #[expect(
        clippy::print_stdout,
        reason = "the one place that shows the secret of the token, once"
    )]
    {
        println!("organization: {org_id}");
        println!("API token (shown once, keep it in a secret store):");
        println!("{}", token.expose_secret());
    }
    Ok(())
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

/// A role of the process: a part of its loops, named in the logs
/// (`DECISIONS.md` D91, D168).
#[derive(Clone, Copy)]
enum Role {
    /// HTTP server of the public address: API, webhooks, dashboard.
    Web,
    /// Reconciliation loop of the queues.
    Scheduler,
    /// Execution of the jobs of the modules.
    Worker,
    /// Delivery of the outbox.
    Notifier,
}

impl Role {
    const ALL: [Self; 4] = [Self::Web, Self::Scheduler, Self::Worker, Self::Notifier];

    fn as_str(self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Scheduler => "scheduler",
            Self::Worker => "worker",
            Self::Notifier => "notifier",
        }
    }
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
    tasks.spawn(health::serve(listener, pool.clone(), cancel.clone()));
    let shared = Shared {
        pool,
        process: process_name(),
        http_addr,
        github,
        cancel: cancel.clone(),
    };
    for role in Role::ALL {
        let span = info_span!("role", role = role.as_str());
        tasks.spawn(run_role(role, shared.clone()).instrument(span));
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
            let modules = vec![merge_queue(&pool, &github, &process).api];
            let webhooks = GithubWebhooks {
                scm_instance_id: github.scm_instance_id,
                secret: github.webhook_secret,
            };
            let router = cihub_web::router(pool.clone(), now, webhooks, modules);
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
            let module = merge_queue(&pool, &github, &process);
            let queues = (module.scheduler)(cancel.clone());
            let maintenance = request_hourly(&pool, &cancel);
            let sweep = request_sweep_regularly(&pool, github.scm_instance_id, &cancel);
            let (queues, (), ()) = tokio::join!(queues, maintenance, sweep);
            queues.map_err(|err| anyhow!(err).context("the scheduler of the queues stopped"))?;
        }
        Role::Worker => {
            let mut executors = Executors::default();
            executors.register(MAINTENANCE, execute_maintenance);
            (merge_queue(&pool, &github, &process).executors)(&mut executors);
            let scm: Arc<dyn Scm> = github.app.clone();
            executors.register(
                SWEEP_DELIVERIES,
                sweep_executor(Arc::clone(&scm), github.scm_instance_id),
            );
            executors.register(SYNC_REPOS, sync_repos_executor(scm));
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

/// The merge queue module, on the GitHub App of the instance.
fn merge_queue(pool: &PgPool, github: &Github, process: &str) -> cihub_platform::Module {
    let translate: Translate = Arc::new(|event, payload| {
        cihub_github::translate(event, payload)
            .map_err(|err| Box::new(err) as Box<dyn Error + Send + Sync>)
    });
    let scm: Arc<dyn Scm> = github.app.clone();
    cihub_mergequeue::module(
        pool.clone(),
        scm,
        translate,
        process.to_owned(),
        Arc::new(now),
    )
}

/// The budgets that changed in the GitHub App, as `scm_connections` keeps
/// them: the rate limit with the smallest share left (`DECISIONS.md` D77).
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

/// Runs `request` at the start, then every `every`, until `cancel`. Each
/// replica does it: the key of the period admits one job, so a copy changes
/// nothing.
async fn repeat_every<Fut: Future<Output = ()>>(
    every: Duration,
    cancel: &CancellationToken,
    mut request: impl FnMut() -> Fut,
) {
    let mut interval = tokio::time::interval(every);
    loop {
        tokio::select! {
            () = cancel.cancelled() => return,
            _ = interval.tick() => request().await,
        }
    }
}

/// The work of one system transaction; boxed, because a closure cannot name
/// the future that borrows its argument.
type InTransaction<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Runs `request` in one system transaction, and commits it.
async fn in_system_transaction<T, E>(
    pool: &PgPool,
    request: impl for<'a> FnOnce(
        &'a mut Transaction<'static, Postgres>,
    ) -> InTransaction<'a, Result<T, E>>,
) -> anyhow::Result<T>
where
    anyhow::Error: From<E>,
{
    let mut tx = begin_system(pool).await?;
    let result = request(&mut tx).await?;
    tx.commit().await?;
    Ok(result)
}

/// Requests the maintenance job of the day (`DECISIONS.md` D67) and the read of
/// the repositories of each connection (D155), every hour.
async fn request_hourly(pool: &PgPool, cancel: &CancellationToken) {
    repeat_every(Duration::from_hours(1), cancel, || async move {
        if let Err(err) = request_maintenance_now(pool).await {
            warn!(error = &*err as &dyn Error, "maintenance not requested");
        }
        if let Err(err) = request_repo_syncs_now(pool).await {
            warn!(
                error = &*err as &dyn Error,
                "reads of the repositories not requested"
            );
        }
    })
    .await;
}

async fn request_maintenance_now(pool: &PgPool) -> anyhow::Result<()> {
    let requested =
        in_system_transaction(pool, |tx| Box::pin(request_maintenance(tx, now()))).await?;
    if requested {
        info!("maintenance requested");
    }
    Ok(())
}

async fn request_repo_syncs_now(pool: &PgPool) -> anyhow::Result<()> {
    let requested =
        in_system_transaction(pool, |tx| Box::pin(request_repo_syncs(tx, now()))).await?;
    if requested > 0 {
        info!(requested, "reads of the repositories requested");
    }
    Ok(())
}

/// Requests the sweep of the deliveries of `instance` every [`SWEEP_EVERY`]
/// (`DECISIONS.md` D88).
async fn request_sweep_regularly(
    pool: &PgPool,
    instance: ScmInstanceId,
    cancel: &CancellationToken,
) {
    repeat_every(SWEEP_EVERY, cancel, || async move {
        if let Err(err) = request_sweep_now(pool, instance).await {
            warn!(error = &*err as &dyn Error, "sweep not requested");
        }
    })
    .await;
}

async fn request_sweep_now(pool: &PgPool, instance: ScmInstanceId) -> anyhow::Result<()> {
    let requested =
        in_system_transaction(pool, |tx| Box::pin(request_sweep(tx, instance, now()))).await?;
    if requested {
        info!("sweep of the deliveries requested");
    }
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

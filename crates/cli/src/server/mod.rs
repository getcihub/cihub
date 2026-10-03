//! `cihub server`: starts and administers an instance, with a direct access to
//! Postgres (`DECISIONS.md` D168, D169). One file per command (`start`,
//! `migrate`, `bootstrap`) and one per role (`web`, `scheduler`, `worker`,
//! `notifier`). A role holds no queue logic: it starts loops whose code lives
//! in the library crates.

mod bootstrap;
mod config;
mod health;
mod migrate;
mod notifier;
mod scheduler;
mod start;
mod web;
mod worker;

use std::collections::BTreeSet;
use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, bail};
use cihub_domain::{ScmInstanceId, ScmKind};
use cihub_github::GithubApp;
use cihub_mergequeue::Translate;
use cihub_scm::Scm;
use cihub_store::{MIGRATOR, PgPool, begin_system, ensure_scm_instance};
use clap::{Args, Subcommand, ValueEnum};
use secrecy::SecretString;
use serde::Serialize;
use time::OffsetDateTime;
use tracing::info;

use self::config::Config;

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub(crate) struct ServerArgs {
    /// The configuration file; without it, the `CIHUB_*` variables and the
    /// defaults only (`docs/DEPLOYMENT.md`).
    #[arg(long, global = true, value_name = "FILE")]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: ServerCommand,
}

#[derive(Subcommand)]
enum ServerCommand {
    /// Starts the given roles in one process; the four without argument.
    Start {
        #[arg(value_name = "ROLE")]
        roles: Vec<Role>,
    },
    /// Applies the migrations, creates the partitions, then stops.
    Migrate,
    /// Creates the organization of an installation of the GitHub App and
    /// shows a `bootstrap` token once.
    Bootstrap {
        /// The identifier of the installation.
        #[arg(long)]
        installation: String,
    },
}

/// A role of the process (`DECISIONS.md` D91, D168).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// The public address: API, webhooks, dashboard.
    Web,
    /// The reconciliation loop of the queues.
    Scheduler,
    /// The execution of the jobs of the modules.
    Worker,
    /// The delivery of the outbox.
    Notifier,
}

impl Role {
    const ALL: [Self; 4] = [Self::Web, Self::Scheduler, Self::Worker, Self::Notifier];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Scheduler => "scheduler",
            Self::Worker => "worker",
            Self::Notifier => "notifier",
        }
    }
}

pub(crate) async fn run(args: ServerArgs) -> anyhow::Result<()> {
    let Config {
        internal_addr,
        http_addr,
        database_url,
        logs,
        github,
    } = Config::load(args.config.as_deref())?;
    logs.init()?;
    let pool = cihub_store::connect(&database_url).await?;
    cihub_store::check_role(&pool).await?;
    match args.command {
        ServerCommand::Migrate => migrate::run(&pool).await,
        ServerCommand::Bootstrap { installation } => {
            let github = start_github(&pool, github).await?;
            bootstrap::run(&pool, &github, &installation).await
        }
        ServerCommand::Start { roles } => {
            let roles: BTreeSet<Role> = if roles.is_empty() {
                Role::ALL.into_iter().collect()
            } else {
                roles.into_iter().collect()
            };
            let github = start_github(&pool, github).await?;
            start::run(roles, internal_addr, http_addr, pool, github).await
        }
    }
}

/// The GitHub App of the instance, once started.
#[derive(Clone)]
struct Github {
    app: Arc<GithubApp>,
    scm_instance_id: ScmInstanceId,
    webhook_secret: SecretString,
}

/// Checks that the database is migrated, then starts the client of the
/// GitHub App and creates the row of its SCM instance when it is missing
/// (`DECISIONS.md` D79). `migrate` does not talk to GitHub; the other
/// commands cannot run without it.
async fn start_github(pool: &PgPool, github: Option<config::GithubApp>) -> anyhow::Result<Github> {
    let Some(github) = github else {
        bail!(
            "GitHub is not configured: set the keys github.app_id, github.private_key or \
             github.private_key_file, and github.webhook_secret, or their CIHUB_GITHUB_* \
             variables (docs/GITHUB.md)"
        );
    };
    let missing = cihub_store::missing_migrations(&MIGRATOR, pool).await?;
    if !missing.is_empty() {
        bail!("the database misses the migrations {missing:?}: run `cihub server migrate`");
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

/// The clock of the process: the library crates receive the time from here.
fn now() -> OffsetDateTime {
    #[expect(
        clippy::disallowed_methods,
        reason = "the one place where the server reads the clock"
    )]
    OffsetDateTime::now_utc()
}

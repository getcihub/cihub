//! `cihub server`: starts and administers an instance, with a direct access to
//! Postgres (`DECISIONS.md` D168, D169, D248). One file per command (`start`,
//! `migrate`, `token`) and one per role (`web`, `scheduler`, `worker`,
//! `notifier`). A role holds no queue logic: it starts loops whose code lives
//! in the library crates.

mod config;
mod health;
mod migrate;
mod notifier;
mod scheduler;
mod start;
mod token;
mod web;
mod worker;

use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, bail};
use cihub_github::GithubApp;
use cihub_mergequeue::Translate;
use cihub_scm::Scm;
use cihub_store::{MIGRATOR, PgPool};
use clap::{Args, Subcommand};
use secrecy::SecretString;
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
    /// Starts the four roles in one process.
    Start,
    /// Applies the migrations, creates the partitions, then stops.
    Migrate,
    /// Manages the API tokens of an organization.
    #[command(subcommand)]
    Token(TokenCommand),
}

#[derive(Subcommand)]
enum TokenCommand {
    /// Creates an API token with every scope and shows it once.
    Create {
        /// The slug of the organization.
        #[arg(long)]
        org: String,
    },
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
        ServerCommand::Token(TokenCommand::Create { org }) => token::create(&pool, &org).await,
        ServerCommand::Start => {
            let github = start_github(&pool, github).await?;
            start::run(internal_addr, http_addr, pool, github).await
        }
    }
}

#[derive(Clone)]
struct Github {
    app: Arc<GithubApp>,
    webhook_secret: SecretString,
}

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

    info!(
        url = config.url,
        app_id = config.app_id,
        "GitHub App configured"
    );

    Ok(Github {
        app: Arc::new(client),
        webhook_secret,
    })
}

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

fn now() -> OffsetDateTime {
    #[expect(
        clippy::disallowed_methods,
        reason = "the one place where the server reads the clock"
    )]
    OffsetDateTime::now_utc()
}

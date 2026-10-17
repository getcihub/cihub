//! `cihub-server`: starts and administers a CIHub instance, with a direct
//! access to Postgres. One file per command (`start`, `migrate`, `token`) and
//! one per role (`web`, `scheduler`, `worker`, `notifier`). A role holds no
//! queue logic: it starts loops whose code lives in the library crates. The
//! command line of the users is `cihub`, in `cihub-cli`.

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
use cihub_store::{DatabaseCipher, MIGRATOR, PgPool};
use clap::{Parser, Subcommand};
use secrecy::SecretString;
use time::OffsetDateTime;
use tracing::info;

use self::config::Config;
use self::start::Shared;

#[derive(Parser)]
#[command(
    name = "cihub-server",
    version,
    about = "Starts and administers a CIHub instance",
    arg_required_else_help = true
)]
struct Cli {
    /// The configuration file; without it, the `CIHUB_*` variables and the
    /// defaults only (`specs/DEPLOYMENT.md`).
    #[arg(long, global = true, value_name = "FILE")]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let Config {
        internal_addr,
        http_addr,
        public_url,
        database_url,
        database_secret,
        logs,
        github,
    } = Config::load(cli.config.as_deref())?;

    logs.init()?;
    let pool = cihub_store::connect(&database_url).await?;
    cihub_store::check_role(&pool).await?;
    match cli.command {
        Command::Migrate => migrate::run(&pool).await,
        Command::Token(TokenCommand::Create { org }) => token::create(&pool, &org).await,
        Command::Start => {
            let github = start_github(&pool, github, database_secret.as_ref()).await?;
            start::run(internal_addr, http_addr, public_url, pool, github).await
        }
    }
}

#[derive(Clone)]
struct Github {
    app: Arc<GithubApp>,
    webhook_secret: SecretString,
    /// Seals the tokens of the people; `None` when nobody signs in.
    cipher: Option<Arc<DatabaseCipher>>,
}

async fn start_github(
    pool: &PgPool,
    github: Option<config::GithubApp>,
    database_secret: Option<&SecretString>,
) -> anyhow::Result<Github> {
    let Some(github) = github else {
        bail!(
            "GitHub is not configured: set the keys github.app_id, github.private_key or \
             github.private_key_file, and github.webhook_secret, or their CIHUB_GITHUB_* \
             variables (specs/GITHUB.md)"
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

    info!(
        url = config.url,
        app_id = config.app_id,
        "GitHub App configured"
    );

    let cipher = database_secret
        .map(DatabaseCipher::new)
        .transpose()
        .context("the database secret cannot seal the tokens")?;

    Ok(Github {
        app: Arc::new(client),
        webhook_secret,
        cipher: cipher.map(Arc::new),
    })
}

/// A new module also adds its `OpenAPI` document to `examples/openapi.rs`,
/// which writes the document of the dashboard. Its `public_url` is the address
/// of the dashboard, which the statuses and the comments of the PRs link to.
fn merge_queue(shared: &Shared) -> cihub_platform::Module {
    let translate: Translate = Arc::new(|event, payload| {
        cihub_github::translate(event, payload)
            .map_err(|err| Box::new(err) as Box<dyn Error + Send + Sync>)
    });
    let scm: Arc<dyn Scm> = shared.github.app.clone();
    cihub_mergequeue::module(
        shared.pool.clone(),
        scm,
        translate,
        shared.process.clone(),
        Arc::new(now),
        shared.public_url.clone(),
    )
}

fn now() -> OffsetDateTime {
    #[expect(
        clippy::disallowed_methods,
        reason = "the one place where the server reads the clock"
    )]
    OffsetDateTime::now_utc()
}

//! `cihub`: the command line of CIHub, for a person or a CI. `cihub auth`
//! signs the command line in to an instance; `cihub queue` drives the merge
//! queue through the public API. It holds no server code and no queue
//! logic, and never reaches Postgres: `cihub-server` starts and administers an
//! instance.

mod auth;
mod credentials;
mod queue;
mod repo;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "cihub",
    version,
    about = "CIHub, the merge queue",
    arg_required_else_help = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Signs the command line in to an instance, and shows its sessions.
    Auth(auth::AuthArgs),
    /// The merge queue: PRs and queues.
    Queue(queue::QueueArgs),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut out = std::io::stdout();
    match Cli::parse().command {
        Command::Auth(args) => auth::run(&mut out, args).await,
        Command::Queue(args) => queue::run(&mut out, args).await,
    }
}

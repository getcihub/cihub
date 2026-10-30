//! `cihub`: the command line of CIHub, for a person or a CI. `cihub auth`
//! signs the command line in to an instance; `cihub queue` drives the merge
//! queue through the public API. It holds no server code and no queue logic,
//! and never reaches Postgres or the SCM: `cihub-server` starts and
//! administers an instance.

mod auth;
mod credentials;
mod output;
mod queue;
mod repo;

use std::io::IsTerminal;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "cihub",
    version,
    about = "CIHub merge queue",
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
    let is_no_color = std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty());
    let style = output::Style::new(out.is_terminal(), is_no_color);
    match Cli::parse().command {
        Command::Auth(args) => auth::run(&mut out, style, args).await,
        Command::Queue(args) => queue::run(&mut out, style, args).await,
    }
}

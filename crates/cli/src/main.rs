//! `cihub`: the command line of CIHub, for a person or a CI. `cihub login`
//! keeps the address of an instance and an API token; `cihub queue` drives the
//! merge queue through the public API. It holds no server code and no queue
//! logic, and never reaches Postgres: `cihub-server` starts and administers an
//! instance.

mod credentials;
mod login;
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
    /// Keeps the address of the instance and an API token.
    Login(login::LoginArgs),
    /// The merge queue: PRs and queues.
    Queue(queue::QueueArgs),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut out = std::io::stdout();
    match Cli::parse().command {
        Command::Login(args) => login::run(&mut out, args),
        Command::Queue(args) => queue::run(&mut out, args).await,
    }
}

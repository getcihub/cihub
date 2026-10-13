//! `cihub`: the one binary of CIHub. It defines
//! every command, one file per command, and starts the components. `cihub
//! login` and `cihub queue` talk to the public API with a token; `cihub
//! server` starts and administers an instance, with a direct access to
//! Postgres. A command talks to one of the two, never both. It holds no queue
//! logic: the code of the components lives in the library crates.

mod credentials;
mod login;
mod queue;
mod repo;
mod server;

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
    /// Starts and administers an instance (direct access to Postgres).
    Server(server::ServerArgs),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut out = std::io::stdout();
    match Cli::parse().command {
        Command::Login(args) => login::run(&mut out, args),
        Command::Queue(args) => queue::run(&mut out, args).await,
        Command::Server(args) => server::run(args).await,
    }
}

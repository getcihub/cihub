//! `cihub`: the command line of CIHub (`docs/API.md`, `DECISIONS.md` D161). A
//! thin client of the public API, distributed alone and without server code:
//! it finds the repository from the remote `origin` of git, and the queue
//! from `--queue` or from the only queue of the repository.

mod credentials;
mod repo;

use std::io::Write;

use anyhow::{Context, bail};
use cihub_api_types::{CreateQueue, PriorityInput, Queue, RequestedQueueState, SubmitPr};
use cihub_client::Client;
use clap::{Parser, Subcommand};
use secrecy::SecretString;

use crate::credentials::Credentials;

#[derive(Parser)]
#[command(name = "cihub", version, about = "The command line of CIHub")]
struct Cli {
    /// The queue of the repository, by its name; needed when the repository
    /// has more than one queue.
    #[arg(long, global = true)]
    queue: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Keeps the address of the instance and an API token.
    Login {
        /// The address of the instance, such as `https://cihub.example.com`.
        #[arg(long)]
        url: String,
        /// The API token; read from the standard input without it.
        #[arg(long)]
        token: Option<String>,
    },
    /// Submits a PR to the queue.
    Merge {
        pr: u64,
        /// `urgent`, `high`, `medium`, `low`, or a value from 0 to 255.
        #[arg(short, long)]
        priority: Option<String>,
        #[arg(long)]
        no_batch: bool,
    },
    /// Takes the PR out of the queue.
    Cancel { pr: u64 },
    /// Tests the PR again.
    Restart { pr: u64 },
    /// The state of a PR, or of the queue without one.
    Status { pr: Option<u64> },
    /// Changes the queue.
    #[command(subcommand)]
    Queue(QueueCommand),
}

#[derive(Subcommand)]
enum QueueCommand {
    /// Creates a queue on a branch of the repository.
    Create {
        branch: String,
        /// The target branch by default.
        #[arg(long)]
        name: Option<String>,
    },
    /// Stops the tests and the merges; the queue keeps its order.
    Pause,
    /// Starts the queue again.
    Resume,
    /// Finishes the items in the queue and refuses new ones.
    Drain,
    /// The merge graph of the queue.
    Graph,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let mut out = std::io::stdout().lock();
    if let Command::Login { url, token } = &cli.command {
        return login(&mut out, url, token.clone()).await;
    }
    let credentials = Credentials::load()?;
    let client = Client::new(&credentials.url, SecretString::from(credentials.token))?;
    let (owner, name) = repo::from_git()?;
    match cli.command {
        Command::Login { .. } => Ok(()),
        Command::Queue(QueueCommand::Create {
            branch,
            name: queue_name,
        }) => {
            let request = CreateQueue {
                target_branch: branch,
                name: queue_name,
            };
            let queue = client.create_queue(&owner, &name, &request).await?;
            writeln!(
                out,
                "queue `{}` created on `{}`",
                queue.name, queue.target_branch
            )?;
            Ok(())
        }
        command => {
            let queue = find_queue(&client, &owner, &name, cli.queue.as_deref()).await?;
            run(&mut out, &client, &queue, command).await
        }
    }
}

async fn login(out: &mut impl Write, url: &str, token: Option<String>) -> anyhow::Result<()> {
    let token = if let Some(token) = token {
        token
    } else {
        writeln!(out, "API token:")?;
        let mut line = String::new();
        std::io::stdin().read_line(&mut line)?;
        line.trim().to_owned()
    };
    let client = Client::new(url, SecretString::from(token.clone()))?;
    let repos = client
        .repos()
        .await
        .context("the token does not open the API")?;
    let path = Credentials {
        url: url.to_owned(),
        token,
    }
    .save()?;
    writeln!(
        out,
        "logged in: {} repositories, credentials in {}",
        repos.repos.len(),
        path.display()
    )?;
    Ok(())
}

/// The queue named `wanted`, or the only queue of the repository.
async fn find_queue(
    client: &Client,
    owner: &str,
    name: &str,
    wanted: Option<&str>,
) -> anyhow::Result<Queue> {
    let queues = client.queues(owner, name).await?.queues;
    pick_queue(queues, wanted, &format!("{owner}/{name}"))
}

fn pick_queue(queues: Vec<Queue>, wanted: Option<&str>, repo: &str) -> anyhow::Result<Queue> {
    let names: Vec<String> = queues.iter().map(|q| q.name.clone()).collect();
    if let Some(wanted) = wanted {
        return queues
            .into_iter()
            .find(|q| q.name == wanted)
            .with_context(|| format!("{repo} has no queue `{wanted}`: {}", names.join(", ")));
    }
    let mut queues = queues;
    match queues.len() {
        0 => bail!("{repo} has no queue: create one with `cihub queue create <branch>`"),
        1 => Ok(queues.remove(0)),
        _ => bail!(
            "{repo} has more than one queue: choose one with `--queue` ({})",
            names.join(", ")
        ),
    }
}

async fn run(
    out: &mut impl Write,
    client: &Client,
    queue: &Queue,
    command: Command,
) -> anyhow::Result<()> {
    let id = &queue.id;
    match command {
        Command::Merge {
            pr,
            priority,
            no_batch,
        } => {
            let priority = priority.map(|text| match text.parse::<u8>() {
                Ok(value) => PriorityInput::Value(value),
                Err(_) => PriorityInput::Name(text),
            });
            let request = SubmitPr {
                pr_number: pr,
                priority,
                no_batch,
                direct: false,
            };
            client.submit(id, &request).await?;
            writeln!(out, "#{pr} submitted to `{}`", queue.name)?;
        }
        Command::Cancel { pr } => {
            let item = client.pr(id, pr, false).await?.item;
            client.cancel(&item.id).await?;
            writeln!(out, "#{pr}: cancel asked")?;
        }
        Command::Restart { pr } => {
            let item = client.pr(id, pr, false).await?.item;
            client.restart(&item.id).await?;
            writeln!(out, "#{pr}: restart asked")?;
        }
        Command::Status { pr: Some(pr) } => {
            let item = client.pr(id, pr, false).await?.item;
            let position = item
                .position
                .map(|p| format!(", position {p}"))
                .unwrap_or_default();
            writeln!(
                out,
                "#{pr}: {} ({}){position}",
                item.state, item.state_reason
            )?;
            for reason in &item.block_reasons {
                writeln!(out, "  blocked: {}", serde_json::to_string(reason)?)?;
            }
        }
        Command::Status { pr: None } => {
            writeln!(
                out,
                "queue `{}` on `{}`: {}",
                queue.name, queue.target_branch, queue.state
            )?;
            if let Some(error) = &queue.config_error {
                writeln!(out, "  configuration file not valid: {error}")?;
            }
            for item in client.items(id, 100).await?.items {
                if item.is_current && !item.state.is_terminal() {
                    writeln!(
                        out,
                        "  #{} {} ({})",
                        item.pr_number, item.state, item.state_reason
                    )?;
                }
            }
        }
        Command::Queue(control) => queue_command(out, client, queue, control).await?,
        Command::Login { .. } => {}
    }
    Ok(())
}

async fn queue_command(
    out: &mut impl Write,
    client: &Client,
    queue: &Queue,
    command: QueueCommand,
) -> anyhow::Result<()> {
    let state = match command {
        QueueCommand::Pause => RequestedQueueState::Paused,
        QueueCommand::Resume => RequestedQueueState::Running,
        QueueCommand::Drain => RequestedQueueState::Draining,
        QueueCommand::Graph => {
            let graph = client.graph(&queue.id).await?;
            let pr = |item_id: &str| {
                graph
                    .nodes
                    .iter()
                    .find(|n| n.item_id == item_id)
                    .map_or_else(|| item_id.to_owned(), |n| format!("#{}", n.pr_number))
            };
            for node in &graph.nodes {
                writeln!(out, "#{} {}", node.pr_number, node.state)?;
            }
            for edge in &graph.edges {
                writeln!(out, "{} -> {}", pr(&edge.from), pr(&edge.to))?;
            }
            return Ok(());
        }
        QueueCommand::Create { .. } => return Ok(()),
    };
    client.set_queue_state(&queue.id, state).await?;
    writeln!(
        out,
        "queue `{}`: {} asked",
        queue.name,
        serde_json::to_string(&state)?
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn queue(name: &str) -> Queue {
        serde_json::from_value(serde_json::json!({
            "id": format!("queue-{name}"), "repo_id": "r", "name": name, "target_branch": name,
            "state": "running", "mode": "single", "config_sha": null, "config_error": null,
        }))
        .unwrap()
    }

    #[test]
    fn the_queue_is_the_named_one_or_the_only_one() {
        let only = pick_queue(vec![queue("main")], None, "acme/app").unwrap();
        assert_eq!(only.name, "main");
        let two = || vec![queue("main"), queue("hotfix")];
        assert_eq!(
            pick_queue(two(), Some("hotfix"), "acme/app").unwrap().name,
            "hotfix"
        );
        let error = pick_queue(two(), None, "acme/app").unwrap_err().to_string();
        assert!(error.contains("--queue"), "{error}");
        assert!(pick_queue(two(), Some("x"), "acme/app").is_err());
        assert!(pick_queue(Vec::new(), None, "acme/app").is_err());
    }
}

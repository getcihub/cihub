//! `cihub queue`: the merge queue, through the public API (`specs/API.md`).
//! The repository comes from `--repo`, else from the remote `origin` of git,
//! and its owner is the slug of the organization; the queue comes from
//! `--queue`, else from the only queue of the repository.

use std::io::Write;

use anyhow::{Context, bail};
use cihub_api_types::{PriorityInput, Queue, RequestedQueueState, SubmitPr};
use cihub_client::{Client, QueuePath};
use clap::{Args, Subcommand};
use secrecy::SecretString;

use crate::credentials::Credentials;
use crate::repo;

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub(crate) struct QueueArgs {
    /// The repository, as `owner/name`; the remote `origin` of git by default.
    #[arg(long, global = true, value_name = "OWNER/NAME", value_parser = repo::from_arg)]
    repo: Option<(String, String)>,
    /// The queue of the repository, by its name; needed when the repository
    /// has more than one queue.
    #[arg(long, global = true)]
    queue: Option<String>,
    #[command(subcommand)]
    command: QueueCommand,
}

#[derive(Subcommand)]
enum QueueCommand {
    /// Submits a PR.
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
    /// The state of the queue, or of a PR.
    Status { pr: Option<u64> },
    /// The merge graph of the queue.
    Graph,
    /// Stops the tests and the merges; the queue keeps its order.
    Pause,
    /// Starts the queue again.
    Resume,
    /// Finishes the items in the queue and refuses new ones.
    Drain,
}

pub(crate) async fn run(out: &mut impl Write, args: QueueArgs) -> anyhow::Result<()> {
    let credentials = Credentials::load()?;
    let client = Client::new(&credentials.url, SecretString::from(credentials.token))?;
    let (owner, name) = match args.repo {
        Some(repo) => repo,
        None => repo::from_git()?,
    };
    let queues = client.queues(&owner, &name).await?.queues;
    let queue = pick_queue(queues, args.queue.as_deref(), &format!("{owner}/{name}"))?;
    let path = QueuePath {
        org: &owner,
        repo: &name,
        queue: &queue.name,
    };

    match args.command {
        QueueCommand::Merge {
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
            client.submit(path, &request).await?;
            writeln!(out, "#{pr} submitted to `{}`", queue.name)?;
        }
        QueueCommand::Cancel { pr } => {
            client.cancel(path, pr).await?;
            writeln!(out, "#{pr}: cancel asked")?;
        }
        QueueCommand::Restart { pr } => {
            client.restart(path, pr).await?;
            writeln!(out, "#{pr}: restart asked")?;
        }
        QueueCommand::Status { pr } => status(out, &client, path, &queue, pr).await?,
        QueueCommand::Graph => graph(out, &client, path).await?,
        QueueCommand::Pause => set_state(out, &client, path, RequestedQueueState::Paused).await?,
        QueueCommand::Resume => {
            set_state(out, &client, path, RequestedQueueState::Running).await?;
        }
        QueueCommand::Drain => {
            set_state(out, &client, path, RequestedQueueState::Draining).await?;
        }
    }

    Ok(())
}

async fn status(
    out: &mut impl Write,
    client: &Client,
    path: QueuePath<'_>,
    queue: &Queue,
    pr: Option<u64>,
) -> anyhow::Result<()> {
    let Some(pr) = pr else {
        writeln!(
            out,
            "queue `{}` on `{}`: {}",
            queue.name, queue.target_branch, queue.state
        )?;
        if let Some(error) = &queue.config_error {
            writeln!(out, "  configuration file not valid: {error}")?;
        }
        for item in client.items(path, 100).await?.items {
            if item.is_current && !item.state.is_terminal() {
                writeln!(
                    out,
                    "  #{} {} ({})",
                    item.pr_number, item.state, item.state_description
                )?;
            }
        }
        return Ok(());
    };

    let item = client.pr(path, pr, false).await?.item;
    let position = item
        .position
        .map(|p| format!(", position {p}"))
        .unwrap_or_default();

    writeln!(
        out,
        "#{pr}: {} ({}){position}",
        item.state, item.state_description
    )?;

    for reason in &item.block_reasons {
        writeln!(out, "  blocked: {}", serde_json::to_string(reason)?)?;
    }

    Ok(())
}

async fn graph(out: &mut impl Write, client: &Client, path: QueuePath<'_>) -> anyhow::Result<()> {
    let graph = client.graph(path).await?;
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

    Ok(())
}

async fn set_state(
    out: &mut impl Write,
    client: &Client,
    path: QueuePath<'_>,
    state: RequestedQueueState,
) -> anyhow::Result<()> {
    client.set_queue_state(path, state).await?;
    writeln!(
        out,
        "queue `{}`: {} asked",
        path.queue,
        serde_json::to_string(&state)?
    )?;
    Ok(())
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
        0 => bail!(
            "{repo} has no queue yet: comment `/cihub merge` on a PR of its default branch to create one"
        ),
        1 => Ok(queues.remove(0)),
        _ => bail!(
            "{repo} has more than one queue: choose one with `--queue` ({})",
            names.join(", ")
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn queue(name: &str) -> Queue {
        serde_json::from_value(serde_json::json!({
            "id": format!("queue-{name}"), "repo_id": "r", "name": name, "target_branch": name,
            "state": "running", "mode": "single", "state_changed_at": "2026-10-01T09:00:00Z",
            "state_changed_by": null, "strategy": "pipelined", "target_head_sha": null,
            "access": "ok", "has_config_file": false, "config_sha": null, "config_error": null,
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
        assert!(error.contains("main, hotfix"), "{error}");
        assert!(pick_queue(two(), Some("x"), "acme/app").is_err());
        assert!(pick_queue(Vec::new(), None, "acme/app").is_err());
    }
}

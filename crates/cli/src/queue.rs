//! `cihub queue`: the merge queue, through the public API (`specs/API.md`).
//! The repository comes from `--repo`, else from the remote `origin` of git,
//! and its owner is the slug of the organization. A PR goes to the queue of
//! its base branch; the other commands take the queue of `--branch`, else
//! the queue of the default branch of the repository.

use std::io::Write;

use anyhow::{Context, bail};
use cihub_api_types::{PriorityInput, Queue, RequestedQueueState, SubmitPr};
use cihub_client::{Client, QueuePath, RepoPath};
use cihub_domain::ItemState;
use clap::{Args, Subcommand};

use crate::credentials::{Credentials, address};
use crate::repo;

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub(crate) struct QueueArgs {
    /// The repository, as `owner/name`; the remote `origin` of git by default.
    #[arg(long, global = true, value_name = "OWNER/NAME", value_parser = repo::from_arg)]
    repo: Option<(String, String)>,
    /// The target branch of the queue of `status`, `graph`, `pause`, `resume`
    /// and `drain`; the default branch of the repository without it. The
    /// commands on a PR ignore it: a PR is in the queue of its base branch.
    #[arg(long, global = true)]
    branch: Option<String>,
    #[command(subcommand)]
    command: QueueCommand,
}

#[derive(Subcommand)]
enum QueueCommand {
    /// Submits a PR to the queue of its base branch.
    Merge {
        pr: u64,
        /// `urgent`, `high`, `medium`, `low`, or a value from 0 to 255.
        #[arg(short, long)]
        priority: Option<String>,
        #[arg(long)]
        no_batch: bool,
        /// A profile of the queue; the `conditions` of the profiles decide
        /// without it.
        #[arg(long)]
        profile: Option<String>,
    },
    /// Takes the PR out of its queue.
    Cancel { pr: u64 },
    /// Tests the PR again, in its queue.
    Restart { pr: u64 },
    #[command(flatten)]
    OnQueue(QueueAction),
}

/// A command on one queue: the one of `--branch`, else the one of the
/// default branch of the repository.
#[derive(Subcommand)]
enum QueueAction {
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
    let host = credentials.host(None)?;
    let Some((token, _)) = credentials.token(&host)? else {
        bail!(
            "not signed in to {host}: run `cihub auth login --hostname {host}`, or set CIHUB_TOKEN"
        );
    };
    let client = Client::new(&address(&host), token)?;
    let (owner, name) = match args.repo {
        Some(repo) => repo,
        None => repo::from_git()?,
    };
    let repo = RepoPath {
        org: &owner,
        repo: &name,
    };

    match args.command {
        QueueCommand::Merge {
            pr,
            priority,
            no_batch,
            profile,
        } => {
            let priority = priority.map(|text| match text.parse::<u8>() {
                Ok(value) => PriorityInput::Value(value),
                Err(_) => PriorityInput::Name(text),
            });
            let request = SubmitPr {
                priority,
                no_batch,
                profile,
                direct: false,
            };
            client.submit(repo, pr, &request).await?;
            writeln!(out, "#{pr} submitted")?;
        }
        QueueCommand::Cancel { pr } => {
            client.cancel(repo, pr).await?;
            writeln!(out, "#{pr}: cancel asked")?;
        }
        QueueCommand::Restart { pr } => {
            client.restart(repo, pr).await?;
            writeln!(out, "#{pr}: restart asked")?;
        }
        QueueCommand::OnQueue(action) => {
            on_queue(out, &client, repo, args.branch.as_deref(), action).await?;
        }
    }

    Ok(())
}

/// `action` on the queue of `branch`, else on the queue of the default branch
/// of `repo`.
async fn on_queue(
    out: &mut impl Write,
    client: &Client,
    repo: RepoPath<'_>,
    branch: Option<&str>,
    action: QueueAction,
) -> anyhow::Result<()> {
    let queues = client.queues(repo).await?.queues;
    let queue = pick_queue(queues, branch, &format!("{}/{}", repo.org, repo.repo))?;
    let path = QueuePath {
        org: repo.org,
        repo: repo.repo,
        queue: &queue.target_branch,
    };

    match action {
        QueueAction::Status { pr } => status(out, client, path, &queue, pr).await?,
        QueueAction::Graph => graph(out, client, path).await?,
        QueueAction::Pause => set_state(out, client, path, RequestedQueueState::Paused).await?,
        QueueAction::Resume => {
            set_state(out, client, path, RequestedQueueState::Running).await?;
        }
        QueueAction::Drain => {
            set_state(out, client, path, RequestedQueueState::Draining).await?;
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
        writeln!(out, "queue of `{}`: {}", queue.target_branch, queue.state)?;
        if let Some(error) = &queue.config_error {
            writeln!(out, "  configuration file not valid: {error}")?;
        }
        let live = [
            ItemState::NotReady,
            ItemState::Pending,
            ItemState::Testing,
            ItemState::PendingFailure,
            ItemState::TestsPassed,
        ]
        .map(ItemState::as_str);
        for pr in client.prs(path, &live).await?.prs {
            writeln!(
                out,
                "  #{} {} ({})",
                pr.number, pr.state, pr.state_description
            )?;
        }
        return Ok(());
    };

    let item = client.pr(path.repo(), pr).await?;
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
        "queue of `{}`: {} asked",
        path.queue,
        serde_json::to_string(&state)?
    )?;
    Ok(())
}

fn pick_queue(mut queues: Vec<Queue>, wanted: Option<&str>, repo: &str) -> anyhow::Result<Queue> {
    let branches = queues
        .iter()
        .map(|q| q.target_branch.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    if let Some(wanted) = wanted {
        return queues
            .into_iter()
            .find(|q| q.target_branch == wanted)
            .with_context(|| format!("{repo} has no queue of `{wanted}`: {branches}"));
    }
    if queues.is_empty() {
        bail!(
            "{repo} has no queue yet: the first `merge` command on a PR of its default branch creates one"
        );
    }
    match queues.iter().position(|q| q.is_default_branch) {
        Some(index) => Ok(queues.remove(index)),
        None if queues.len() == 1 => Ok(queues.remove(0)),
        None => bail!(
            "{repo} has no queue on its default branch: choose one with `--branch` ({branches})"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn queue(branch: &str) -> Queue {
        serde_json::from_value(serde_json::json!({
            "id": format!("queue-{branch}"), "repo_id": "r", "target_branch": branch,
            "is_default_branch": branch == "main",
            "state": "running", "mode": "single", "state_changed_at": "2026-10-01T09:00:00Z",
            "state_changed_by": null, "strategy": "pipelined", "target_head_sha": null,
            "is_suspended": false, "has_config_file": false, "config_sha": null, "config_error": null,
            "repo": "app", "merged_last_24h": 0, "failed_last_24h": 0,
            "failed_or_cancelled_last_24h": 0, "last_merged_at": null,
            "counts": { "not_ready": 0, "pending": 0, "testing": 0, "pending_failure": 0,
                        "tests_passed": 0 },
            "live_runs": 0, "median_run_seconds": null,
            "window": { "current": 1, "floor": 1, "ceiling": 1 },
            "settings": { "batching_enabled": false, "batching_size": 1,
                          "batching_max_wait_minutes": 0, "failure_budget_retries": 0,
                          "merge_method": "squash" },
            "setup": { "findings": [], "checked_at": null },
        }))
        .unwrap()
    }

    #[test]
    fn the_queue_is_the_one_of_the_branch_else_the_default_one() {
        let only = pick_queue(vec![queue("release/1.0")], None, "acme/app").unwrap();
        assert_eq!(only.target_branch, "release/1.0");
        let two = || vec![queue("release/2.0"), queue("main")];
        assert_eq!(
            pick_queue(two(), Some("release/2.0"), "acme/app")
                .unwrap()
                .target_branch,
            "release/2.0"
        );
        assert_eq!(
            pick_queue(two(), None, "acme/app").unwrap().target_branch,
            "main"
        );
        let releases = vec![queue("release/1.0"), queue("release/2.0")];
        let error = pick_queue(releases, None, "acme/app")
            .unwrap_err()
            .to_string();
        assert!(error.contains("--branch"), "{error}");
        assert!(error.contains("release/1.0, release/2.0"), "{error}");
        assert!(pick_queue(two(), Some("x"), "acme/app").is_err());
        assert!(pick_queue(Vec::new(), None, "acme/app").is_err());
    }
}

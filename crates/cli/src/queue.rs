//! `cihub queue`: the merge queue, through the public API (`specs/API.md`).
//! The repository comes from `--repo`, else from the remote `origin` of git,
//! and its owner is the slug of the organization. A PR goes to the queue of
//! its base branch; the other commands take the queue of `--branch`, else
//! the queue of the default branch of the repository.

use std::fmt::Write as _;
use std::io::Write;
use std::time::Duration;

use anyhow::{Context, bail};
use cihub_api_types::{Pr, PriorityInput, Queue, RequestedQueueState, SubmitPr};
use cihub_client::{Client, QueuePath, RepoPath};
use cihub_domain::{ItemState, QueueState};
use clap::{Args, Subcommand};
use time::OffsetDateTime;

use crate::credentials::{Credentials, address};
use crate::output::{Cell, Color, Screen, Style, duration, now, state_color, state_words};
use crate::repo;

/// How often a watch reads again while the server refuses one more stream.
const POLL_EVERY: Duration = Duration::from_secs(10);

/// The columns of the table of a queue in a terminal; a pipe gets the
/// number, the state, the position, the title and the time of the state.
const HEADER: [&str; 5] = ["POS", "PR", "TITLE", "STATE", "SINCE"];

/// The states of the PRs that the table of a queue shows.
const LIVE: [ItemState; 5] = [
    ItemState::NotReady,
    ItemState::Pending,
    ItemState::Testing,
    ItemState::PendingFailure,
    ItemState::TestsPassed,
];

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub(crate) struct QueueArgs {
    /// The repository, as `owner/name`; the remote `origin` of git by default.
    #[arg(long, global = true, value_name = "OWNER/NAME", value_parser = repo::from_arg)]
    repo: Option<(String, String)>,
    /// The target branch of the queue of `status`, `watch`, `graph`, `pause`,
    /// `resume` and `drain`; the default branch of the repository without it.
    /// The commands on a PR ignore it: a PR is in the queue of its base
    /// branch.
    #[arg(long, global = true)]
    branch: Option<String>,
    #[command(subcommand)]
    command: QueueCommand,
}

#[derive(Subcommand)]
enum QueueCommand {
    /// Submits a PR to the queue of its base branch.
    Merge {
        /// The number of the PR.
        pr: u64,
        /// `urgent`, `high`, `medium`, `low`, or a value from 0 to 255.
        #[arg(short, long)]
        priority: Option<String>,
        /// Tests the PR alone, never in a batch.
        #[arg(long)]
        no_batch: bool,
        /// A profile of the queue; the `conditions` of the profiles decide
        /// without it.
        #[arg(long)]
        profile: Option<String>,
    },
    /// Takes the PR out of its queue.
    Cancel {
        /// The number of the PR.
        pr: u64,
    },
    /// Tests the PR again, in its queue.
    Restart {
        /// The number of the PR.
        pr: u64,
    },
    #[command(flatten)]
    OnQueue(QueueAction),
}

/// A command on one queue: the one of `--branch`, else the one of the
/// default branch of the repository.
#[derive(Subcommand)]
enum QueueAction {
    /// The state of the queue, or of a PR.
    Status {
        /// The number of a PR; the queue without it.
        pr: Option<u64>,
        /// Writes the queue and its PRs, or the PR, as the API gives them.
        #[arg(long)]
        json: bool,
    },
    /// Follows a PR until it ends, or the queue until it is interrupted. With
    /// a PR, ends with 0 when it merges, 1 when it fails or is cancelled.
    Watch {
        /// The number of a PR; the queue without it.
        pr: Option<u64>,
        /// Writes each change as one line of JSON, as the API gives it.
        #[arg(long)]
        json: bool,
    },
    /// The merge graph of the queue.
    Graph {
        /// Writes the graph as the API gives it.
        #[arg(long)]
        json: bool,
    },
    /// Stops the tests and the merges; the queue keeps its order.
    Pause,
    /// Starts the queue again.
    Resume,
    /// Finishes the items in the queue and refuses new ones.
    Drain,
}

/// What a command on a queue works with.
struct Target<'a> {
    client: &'a Client,
    style: Style,
    repo: RepoPath<'a>,
}

pub(crate) async fn run(out: &mut impl Write, style: Style, args: QueueArgs) -> anyhow::Result<()> {
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
            let target = Target {
                client: &client,
                style,
                repo,
            };
            on_queue(out, &target, args.branch.as_deref(), action).await?;
        }
    }

    Ok(())
}

/// `action` on the queue of `branch`, else on the queue of the default branch
/// of the repository.
async fn on_queue(
    out: &mut impl Write,
    target: &Target<'_>,
    branch: Option<&str>,
    action: QueueAction,
) -> anyhow::Result<()> {
    let repo = target.repo;
    let queues = target.client.queues(repo).await?.queues;
    let queue = pick_queue(queues, branch, &repo_name(repo))?;
    let path = QueuePath {
        org: repo.org,
        repo: repo.repo,
        queue: &queue.target_branch,
    };

    match action {
        QueueAction::Status { pr: Some(pr), json } => pr_status(out, target, pr, json).await,
        QueueAction::Status { pr: None, json } => queue_status(out, target, path, json).await,
        QueueAction::Watch { pr: Some(pr), json } => watch_pr(out, target, pr, json).await,
        QueueAction::Watch { pr: None, json } => {
            watch_queue(out, target, path, &queue.id, json).await
        }
        QueueAction::Graph { json } => graph(out, target.client, path, json).await,
        QueueAction::Pause => {
            set_state(out, target.client, path, RequestedQueueState::Paused).await
        }
        QueueAction::Resume => {
            set_state(out, target.client, path, RequestedQueueState::Running).await
        }
        QueueAction::Drain => {
            set_state(out, target.client, path, RequestedQueueState::Draining).await
        }
    }
}

async fn queue_status(
    out: &mut impl Write,
    target: &Target<'_>,
    path: QueuePath<'_>,
    json: bool,
) -> anyhow::Result<()> {
    let (queue, prs) = read_queue(target.client, path).await?;

    if json {
        let view = serde_json::json!({ "queue": queue, "prs": prs });
        writeln!(out, "{}", serde_json::to_string_pretty(&view)?)?;
        return Ok(());
    }

    let text = queue_view(target.style, &repo_name(target.repo), &queue, &prs, now());
    out.write_all(text.as_bytes())?;
    Ok(())
}

async fn pr_status(
    out: &mut impl Write,
    target: &Target<'_>,
    number: u64,
    json: bool,
) -> anyhow::Result<()> {
    let pr = target.client.pr(target.repo, number).await?;

    if json {
        writeln!(out, "{}", serde_json::to_string_pretty(&pr)?)?;
        return Ok(());
    }

    let text = pr_view(target.style, &repo_name(target.repo), &pr, now());
    out.write_all(text.as_bytes())?;
    Ok(())
}

/// Follows the PR until it ends; an error, so a non-zero exit code, unless
/// it merged.
async fn watch_pr(
    out: &mut impl Write,
    target: &Target<'_>,
    number: u64,
    json: bool,
) -> anyhow::Result<()> {
    let mut screen = Screen::new(target.style);
    let mut written = Ok(());
    let repo_name = repo_name(target.repo);

    let pr = target
        .client
        .follow_pr(target.repo, number, POLL_EVERY, |pr| {
            let text = if json {
                serde_json::to_string(pr)
                    .map(|line| line + "\n")
                    .unwrap_or_default()
            } else {
                pr_view(target.style, &repo_name, pr, now())
            };
            if written.is_ok() {
                written = screen.draw(out, &text);
            }
        })
        .await?;
    written?;

    if pr.state != ItemState::Merged {
        bail!("#{number} {}: {}", pr.state, pr.state_description);
    }

    Ok(())
}

/// Draws the queue again on each of its changes, until it is interrupted.
async fn watch_queue(
    out: &mut impl Write,
    target: &Target<'_>,
    path: QueuePath<'_>,
    queue_id: &str,
    json: bool,
) -> anyhow::Result<()> {
    let mut screen = Screen::new(target.style);
    let mut watch = target.client.watch_queue(path.org, queue_id, POLL_EVERY);
    let repo_name = repo_name(target.repo);

    loop {
        let (queue, prs) = read_queue(target.client, path).await?;
        let text = if json {
            serde_json::to_string(&serde_json::json!({ "queue": queue, "prs": prs }))? + "\n"
        } else if target.style.is_terminal {
            queue_view(target.style, &repo_name, &queue, &prs, now())
        } else {
            // In a pipe, an empty line ends the rows of each reading.
            queue_view(target.style, &repo_name, &queue, &prs, now()) + "\n"
        };

        screen.draw(out, &text)?;
        watch.changed().await?;
    }
}

async fn read_queue(client: &Client, path: QueuePath<'_>) -> anyhow::Result<(Queue, Vec<Pr>)> {
    let queues = client.queues(path.repo()).await?.queues;
    let queue = queues
        .into_iter()
        .find(|queue| queue.target_branch == path.queue)
        .with_context(|| format!("the queue of `{}` no longer exists", path.queue))?;
    let live = LIVE.map(ItemState::as_str);
    let prs = client.prs(path, &live).await?.prs;

    Ok((queue, prs))
}

/// The queue and its live PRs: a summary and a table in a terminal, the rows
/// alone in a pipe.
fn queue_view(style: Style, repo: &str, queue: &Queue, prs: &[Pr], now: OffsetDateTime) -> String {
    let rows: Vec<Vec<Cell>> = prs.iter().map(|pr| pr_row(style, pr, now)).collect();
    if !style.is_terminal {
        return style.table(&HEADER, &rows);
    }

    let counts = &queue.counts;
    let in_queue = counts.not_ready
        + counts.pending
        + counts.testing
        + counts.pending_failure
        + counts.tests_passed;
    let state_color = (queue.state != QueueState::Running).then_some(Color::Yellow);
    let mut summary = vec![
        format!("{in_queue} in the queue"),
        plural(queue.live_runs, "run live", "runs live"),
    ];
    if let Some(seconds) = queue.median_run_seconds {
        summary.push(format!("median run {}", duration(seconds.into())));
    }
    summary.push(format!("{} merged in the last 24h", queue.merged_last_24h));

    let mut text = format!(
        "{repo} · queue of {} · {}\n{}\n",
        queue.target_branch,
        style.paint(&queue.state.as_str().replace('_', " "), state_color),
        style.paint(&summary.join(" · "), Some(Color::Dim)),
    );
    if queue.is_suspended {
        let line =
            "CIHub lost its access to the repository: the queue merges nothing until it returns.";
        let _ = writeln!(text, "{}", style.paint(line, Some(Color::Red)));
    }
    if let Some(error) = &queue.config_error {
        let line = format!("The configuration file is not valid: {error}");
        let _ = writeln!(text, "{}", style.paint(&line, Some(Color::Red)));
    }
    text.push('\n');

    if rows.is_empty() {
        let _ = writeln!(text, "No PR in the queue of {}.", queue.target_branch);
        return text;
    }

    text + &style.table(&HEADER, &rows)
}

/// A PR as a row: `POS`, `PR`, `TITLE`, `STATE`, `SINCE` in a terminal; the
/// number, the state, the position, the title and the time in a pipe.
fn pr_row(style: Style, pr: &Pr, now: OffsetDateTime) -> Vec<Cell> {
    let title = pr.title.clone().unwrap_or_default();
    let since = style.time(pr.state_changed_at, now);
    // An ended PR keeps the position that it had, which means nothing more.
    let position = pr.position.filter(|_| !pr.state.is_terminal());
    if !style.is_terminal {
        let position = position.map(|p| p.to_string()).unwrap_or_default();
        return [
            pr.number.to_string(),
            pr.state.as_str().to_owned(),
            position,
            title,
            since,
        ]
        .map(Cell::plain)
        .into();
    }

    let position = position.map_or_else(|| "-".to_owned(), |p| p.to_string());
    vec![
        Cell::plain(position),
        Cell::plain(format!("#{}", pr.number)),
        Cell::plain(title),
        Cell::painted(state_words(pr.state), state_color(pr.state)),
        Cell::plain(since),
    ]
}

/// A PR: its state, why, and what blocks it, in a terminal; its row in a
/// pipe.
fn pr_view(style: Style, repo: &str, pr: &Pr, now: OffsetDateTime) -> String {
    if !style.is_terminal {
        return style.table(&HEADER, &[pr_row(style, pr, now)]);
    }

    let title = pr
        .title
        .as_deref()
        .map(|title| format!(" {title}"))
        .unwrap_or_default();
    let mut state = vec![style.paint(&capitalized(&state_words(pr.state)), state_color(pr.state))];
    if let Some(position) = pr.position.filter(|_| !pr.state.is_terminal()) {
        state.push(format!("position {position}"));
    }
    state.push(format!("since {}", style.time(pr.state_changed_at, now)));

    let mut text = format!(
        "#{}{title} · {repo} → {}\n{}\n  {}\n",
        pr.number,
        pr.queue,
        state.join(" · "),
        pr.state_description
    );
    for reason in &pr.block_reasons {
        let _ = writeln!(
            text,
            "  {} {}",
            style.paint("✗", Some(Color::Red)),
            reason.describe()
        );
    }
    if let Some(url) = &pr.url {
        let _ = writeln!(text, "{}", style.paint(url, Some(Color::Dim)));
    }

    text
}

async fn graph(
    out: &mut impl Write,
    client: &Client,
    path: QueuePath<'_>,
    json: bool,
) -> anyhow::Result<()> {
    let graph = client.graph(path).await?;

    if json {
        writeln!(out, "{}", serde_json::to_string_pretty(&graph)?)?;
        return Ok(());
    }

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

    let asked = match state {
        RequestedQueueState::Running => "resume",
        RequestedQueueState::Paused => "pause",
        RequestedQueueState::Draining => "drain",
    };
    writeln!(out, "queue of `{}`: {asked} asked", path.queue)?;
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
            "{repo} has no queue yet: comment `/cihub merge` on a PR of its default branch to create one"
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

fn repo_name(repo: RepoPath<'_>) -> String {
    format!("{}/{}", repo.org, repo.repo)
}

fn plural(count: u32, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TERMINAL: Style = Style::new(true, true);
    const COLORS: Style = Style::new(true, false);
    const PIPE: Style = Style::new(false, false);

    fn at(text: &str) -> OffsetDateTime {
        OffsetDateTime::parse(text, &time::format_description::well_known::Rfc3339).unwrap()
    }

    fn queue(branch: &str) -> Queue {
        serde_json::from_value(serde_json::json!({
            "id": format!("queue-{branch}"), "repo_id": "r", "target_branch": branch,
            "is_default_branch": branch == "main",
            "state": "running", "mode": "single", "state_changed_at": "2026-10-01T09:00:00Z",
            "state_changed_by": null, "created_at": "2026-10-01T09:00:00Z",
            "updated_at": "2026-10-01T09:00:00Z", "strategy": "pipelined", "target_head_sha": null,
            "is_suspended": false, "has_config_file": false, "config_sha": null, "config_error": null,
            "repo": "app", "merged_last_24h": 14, "failed_last_24h": 0,
            "failed_or_cancelled_last_24h": 0, "last_merged_at": null,
            "counts": { "not_ready": 1, "pending": 0, "testing": 1, "pending_failure": 0,
                        "tests_passed": 0 },
            "live_runs": 1, "median_run_seconds": 720,
            "window": { "current": 1, "floor": 1, "ceiling": 1 },
            "settings": { "batching_enabled": false, "batching_size": 1,
                          "batching_max_wait_minutes": 0, "failure_budget_retries": 0,
                          "merge_method": "squash" },
            "setup": { "findings": [], "checked_at": null },
        }))
        .unwrap()
    }

    fn pr(number: u64, state: &str, position: Option<i64>, blocks: &serde_json::Value) -> Pr {
        serde_json::from_value(serde_json::json!({
            "id": format!("item-{number}"), "queue_id": "queue-main", "repo": "app",
            "queue": "main", "number": number, "title": format!("Change {number}"),
            "url": format!("https://scm.example/acme/app/pull/{number}"),
            "author": { "login": "ada", "avatar_url": null }, "head_sha": "abc",
            "state": state, "state_reason": "submitted",
            "state_description": "the PR was submitted to the queue",
            "state_changed_at": "2026-10-29T09:00:00Z", "priority": 100,
            "position": position, "block_reasons": blocks, "group_id": null, "run_id": null,
            "merged_sha": null, "no_batch": false, "profile": null, "submitted_by": "ada",
            "submitted_via": "comment", "submitted_at": "2026-10-29T09:00:00Z",
            "ahead": [], "merges_in_seconds": null,
        }))
        .unwrap()
    }

    fn live_prs() -> Vec<Pr> {
        let blocks = serde_json::json!([{ "kind": "review_required", "have": 1, "need": 2 }]);
        vec![
            pr(412, "testing", Some(1), &serde_json::json!([])),
            pr(418, "not_ready", None, &blocks),
        ]
    }

    #[test]
    fn a_terminal_shows_the_queue_as_a_table_under_its_summary() {
        let now = at("2026-10-29T09:04:00Z");

        assert_eq!(
            queue_view(TERMINAL, "acme/app", &queue("main"), &live_prs(), now),
            "acme/app · queue of main · running\n\
             2 in the queue · 1 run live · median run 12m · 14 merged in the last 24h\n\
             \n\
             POS  PR    TITLE       STATE      SINCE\n\
             1    #412  Change 412  testing    4m\n\
             -    #418  Change 418  not ready  4m\n"
        );
        assert_eq!(
            queue_view(TERMINAL, "acme/app", &queue("main"), &[], now),
            "acme/app · queue of main · running\n\
             2 in the queue · 1 run live · median run 12m · 14 merged in the last 24h\n\
             \n\
             No PR in the queue of main.\n"
        );
        assert!(
            queue_view(COLORS, "acme/app", &queue("main"), &live_prs(), now)
                .contains("\x1b[33mtesting\x1b[0m")
        );
    }

    #[test]
    fn a_terminal_says_that_the_queue_lost_its_access() {
        let mut suspended = queue("main");
        suspended.is_suspended = true;

        assert_eq!(
            queue_view(
                TERMINAL,
                "acme/app",
                &suspended,
                &[],
                at("2026-10-29T09:04:00Z")
            ),
            "acme/app · queue of main · running\n\
             2 in the queue · 1 run live · median run 12m · 14 merged in the last 24h\n\
             CIHub lost its access to the repository: the queue merges nothing until it returns.\n\
             \n\
             No PR in the queue of main.\n"
        );
    }

    #[test]
    fn a_pipe_gets_the_rows_of_the_queue_alone() {
        let now = at("2026-10-29T09:04:00Z");

        assert_eq!(
            queue_view(PIPE, "acme/app", &queue("main"), &live_prs(), now),
            "412\ttesting\t1\tChange 412\t2026-10-29T09:00:00Z\n\
             418\tnot_ready\t\tChange 418\t2026-10-29T09:00:00Z\n"
        );
        assert_eq!(queue_view(PIPE, "acme/app", &queue("main"), &[], now), "");
    }

    #[test]
    fn a_pr_shows_its_state_and_what_blocks_it() {
        let now = at("2026-10-29T09:12:00Z");
        let blocked = &live_prs()[1];

        assert_eq!(
            pr_view(TERMINAL, "acme/app", blocked, now),
            "#418 Change 418 · acme/app → main\n\
             Not ready · since 12m\n  \
             the PR was submitted to the queue\n  \
             ✗ 1 of 2 required approvals\n\
             https://scm.example/acme/app/pull/418\n"
        );
        assert!(
            pr_view(COLORS, "acme/app", blocked, now).contains("\x1b[31m✗\x1b[0m"),
            "a reason is marked in red"
        );
        assert_eq!(
            pr_view(PIPE, "acme/app", blocked, now),
            "418\tnot_ready\t\tChange 418\t2026-10-29T09:00:00Z\n"
        );
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

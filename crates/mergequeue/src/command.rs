//! The commands that a comment on a PR holds (`docs/GITHUB.md`):
//! `/cihub merge`, which `submit_pr` runs, and `/cihub cancel` and
//! `/cihub restart`, which the job `pr_command` runs (`DECISIONS.md` D157).

use cihub_domain::{Actor, EventKind, ItemId, ItemState, Permission, Priority, QueueId, RepoId};
use cihub_platform::Job;
use cihub_scm::{Scm, ScmUser};
use cihub_store::{PgPool, begin_system};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tracing::info;

use crate::jobs::{
    Account, Failure, acknowledge, give_back_facts, load_repo, load_target, read_payload, reply,
    user_of,
};

/// The type of the job that runs `/cihub cancel` and `/cihub restart`.
pub(crate) const PR_COMMAND: &str = "pr_command";

/// A command of a comment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PrCommand {
    Merge(Merge),
    Cancel,
    Restart,
}

/// `/cihub merge [--priority=urgent|high|medium|low|0-255] [--no-batch]
/// [--direct] [--queue=<name>]`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Merge {
    /// `None` leaves the priority to the `priority_rules` (`DECISIONS.md` D148).
    pub priority: Option<Priority>,
    pub no_batch: bool,
    pub direct: bool,
    /// The queue of the branch to enter, by its name (D148).
    pub queue: Option<String>,
}

/// The first line of `body` that is a command. A command with an option
/// that CIHub does not know is not a command.
pub(crate) fn parse_command(body: &str) -> Option<PrCommand> {
    body.lines().find_map(|line| {
        let mut words = line.split_whitespace();
        if words.next() != Some("/cihub") {
            return None;
        }
        match words.next()? {
            "merge" => parse_merge(words).map(PrCommand::Merge),
            "cancel" if words.next().is_none() => Some(PrCommand::Cancel),
            "restart" if words.next().is_none() => Some(PrCommand::Restart),
            _ => None,
        }
    })
}

fn parse_merge<'a>(words: impl Iterator<Item = &'a str>) -> Option<Merge> {
    let mut merge = Merge::default();

    for word in words {
        if word == "--no-batch" {
            merge.no_batch = true;
        } else if word == "--direct" {
            merge.direct = true;
        } else if let Some(name) = word.strip_prefix("--queue=") {
            if name.is_empty() {
                return None;
            }
            merge.queue = Some(name.to_owned());
        } else {
            let value = word.strip_prefix("--priority=")?;
            let priority =
                Priority::from_name(value).or_else(|| value.parse().ok().map(Priority))?;
            merge.priority = Some(priority);
        }
    }

    Some(merge)
}

/// What `pr_command` changes on the live item of a PR.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ItemCommand {
    Cancel,
    Restart,
}

impl ItemCommand {
    fn name(self) -> &'static str {
        match self {
            Self::Cancel => "/cihub cancel",
            Self::Restart => "/cihub restart",
        }
    }
}

/// The payload of `pr_command`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PrCommandRequest {
    pub repo_id: RepoId,
    pub pr_number: u64,
    pub requested_by: Account,
    pub command: ItemCommand,
    /// The comment of the command (`DECISIONS.md` D166).
    pub comment_id: String,
}

/// Checks the permission of the author, finds the live item of the PR, and
/// gives its request to its queue. A refused command gets a comment that
/// says why.
pub(crate) async fn run_pr_command(
    pool: &PgPool,
    scm: &dyn Scm,
    job: &Job,
    now: OffsetDateTime,
) -> Result<(), Failure> {
    let request: PrCommandRequest = read_payload(job)?;
    let class = job.class;
    let repo = load_repo(pool, request.repo_id).await?;
    let number = request.pr_number;
    let name = request.command.name();
    let refuse = |text: String| reply(scm, &repo.repo, number, name, text, class);
    let author = ScmUser::from(&request.requested_by);
    if scm.permission(&repo.repo, &author, class).await? < Permission::Write {
        info!(pr_number = number, "command refused: permission");
        return refuse("the command needs the write permission on the repository.".to_owned())
            .await;
    }
    let Some((queue_id, item_id, state)) = live_item(pool, repo.repo_id, number).await? else {
        return refuse("this PR has no item in a queue.".to_owned()).await;
    };
    let target = load_target(pool, queue_id).await?;
    if !target.settings()?.commands_enabled.value {
        return refuse(format!(
            "the commands are off for the queue `{}`.",
            target.name
        ))
        .await;
    }
    let kind = match request.command {
        ItemCommand::Cancel => EventKind::ItemCancelRequested {
            item_id,
            by_admin: false,
        },
        ItemCommand::Restart if matches!(state, ItemState::Testing | ItemState::PendingFailure) => {
            EventKind::ItemRestartRequested {
                item_id,
                by_api: false,
            }
        }
        ItemCommand::Restart => {
            return refuse(format!(
                "only a PR under test can restart. This PR is `{state}`."
            ))
            .await;
        }
    };
    let mut tx = begin_system(pool).await?;
    let user_id = user_of(&mut tx, target.scm_instance_id, &author).await?;
    tx.commit().await?;
    let facts = vec![(Actor::User { user_id }, kind)];
    give_back_facts(pool, &target, facts, now).await?;
    acknowledge(scm, &repo.repo, Some(&request.comment_id), class).await;
    info!(
        pr_number = number,
        command = name,
        "command given to the queue"
    );
    Ok(())
}

/// The live item of the PR `number` in a queue of `repo`: its queue, its
/// identifier and its state.
async fn live_item(
    pool: &PgPool,
    repo: RepoId,
    number: u64,
) -> Result<Option<(QueueId, ItemId, ItemState)>, Failure> {
    let mut tx = begin_system(pool).await?;
    let row: Option<(String, String, String)> = sqlx::query_as(
        "SELECT i.queue_id, i.id, i.state::text FROM queue_items i
         JOIN queues q ON q.id = i.queue_id
         WHERE q.repo_id = $1 AND i.pr_number = $2 AND i.is_current
           AND i.state NOT IN ('merged', 'failed', 'cancelled')
         ORDER BY i.submitted_at DESC
         LIMIT 1",
    )
    .bind(repo.to_string())
    .bind(i64::try_from(number).unwrap_or(i64::MAX))
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    let Some((queue, item, state)) = row else {
        return Ok(None);
    };
    let never = |text: String| Failure::Never(text);
    Ok(Some((
        queue.parse().map_err(|_| never(format!("queue {queue}")))?,
        item.parse().map_err(|_| never(format!("item {item}")))?,
        state.parse().map_err(|_| never(format!("state {state}")))?,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[expect(
        clippy::unnecessary_wraps,
        reason = "each case of the table compares an `Option`"
    )]
    fn merge(priority: Option<Priority>, no_batch: bool, direct: bool) -> Option<PrCommand> {
        Some(PrCommand::Merge(Merge {
            priority,
            no_batch,
            direct,
            queue: None,
        }))
    }

    #[test]
    fn commands_and_their_options() {
        let cases = [
            ("/cihub merge", merge(None, false, false)),
            ("LGTM\n  /cihub merge --no-batch", merge(None, true, false)),
            (
                "/cihub merge --priority=urgent --direct",
                merge(Some(Priority::URGENT), false, true),
            ),
            (
                "/cihub merge --priority=42",
                merge(Some(Priority(42)), false, false),
            ),
            (
                "/cihub merge --queue=hotfix --no-batch",
                Some(PrCommand::Merge(Merge {
                    queue: Some("hotfix".into()),
                    priority: None,
                    no_batch: true,
                    direct: false,
                })),
            ),
            ("/cihub merge --queue=", None),
            ("/cihub merge --priority=256", None),
            ("/cihub merge --priority=soon", None),
            ("/cihub merge --force", None),
            ("/cihub cancel", Some(PrCommand::Cancel)),
            ("thanks\n/cihub restart", Some(PrCommand::Restart)),
            ("/cihub cancel now", None),
            ("/cihub rebase", None),
            ("please /cihub merge", None),
            ("/cihubmerge", None),
            ("", None),
        ];
        for (body, want) in cases {
            assert_eq!(parse_command(body), want, "{body:?}");
        }
    }
}

//! The commands that a comment on a PR holds (`docs/GITHUB.md`):
//! `/cihub merge`, which `submit_pr` runs, and `/cihub cancel` and
//! `/cihub restart`, which the job `pr_command` runs (`AGENTS.md` D157).

use std::sync::Arc;

use cihub_domain::{
    Actor, ConnectionId, EventKind, ItemId, ItemState, JobClass, OrgId, Permission, Priority,
    QueueId, RepoId, ScmInstanceId,
};
use cihub_platform::{Job, Outcome};
use cihub_scm::{Scm, ScmUser};
use cihub_store::{PgPool, begin_system};
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::OffsetDateTime;
use tracing::info;

use crate::MergeQueueError;
use crate::jobs::{Account, Failure, give_back, outcome, read_payload, reply, subject, user_of};
use crate::target::{QueueTarget, RepoTarget};

/// The type of the job that runs `/cihub cancel` and `/cihub restart`.
pub const PR_COMMAND: &str = "pr_command";

/// A command of a comment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PrCommand {
    Merge(Merge),
    Cancel,
    Restart,
}

/// `/cihub merge [--priority=urgent|high|medium|low|0-255] [--no-batch]
/// [--direct] [--queue=<name>]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Merge {
    /// `None` leaves the priority to the `priority_rules` (`AGENTS.md` D148).
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
    let mut merge = Merge {
        priority: None,
        no_batch: false,
        direct: false,
        queue: None,
    };
    for word in words {
        match word {
            "--no-batch" => merge.no_batch = true,
            "--direct" => merge.direct = true,
            _ => {
                if let Some(name) = word.strip_prefix("--queue=") {
                    if name.is_empty() {
                        return None;
                    }
                    merge.queue = Some(name.to_owned());
                    continue;
                }
                let value = word.strip_prefix("--priority=")?;
                let priority =
                    Priority::from_name(value).or_else(|| value.parse().ok().map(Priority))?;
                merge.priority = Some(priority);
            }
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
}

/// The job of `request` after the delivery `delivery_id` (`AGENTS.md`
/// D157).
pub(crate) fn pr_command_job(
    org_id: OrgId,
    connection: (ScmInstanceId, ConnectionId),
    delivery_id: &str,
    request: &PrCommandRequest,
) -> Job {
    Job {
        idempotency_key: format!("{PR_COMMAND}:{}:1", subject(delivery_id)),
        job_type: PR_COMMAND.to_owned(),
        class: JobClass::Interactive,
        org_id: Some(org_id),
        connection: Some(connection),
        queue_id: None,
        payload: json!(request),
        attempts: 0,
    }
}

pub(crate) async fn execute_pr_command(
    pool: PgPool,
    scm: Arc<dyn Scm>,
    job: Job,
    now: OffsetDateTime,
) -> Outcome {
    outcome(run_pr_command(&pool, scm.as_ref(), &job, now).await)
}

/// Checks the permission of the author, finds the live item of the PR, and
/// gives its request to its queue. A refused command gets a comment that
/// says why.
async fn run_pr_command(
    pool: &PgPool,
    scm: &dyn Scm,
    job: &Job,
    now: OffsetDateTime,
) -> Result<(), Failure> {
    let request: PrCommandRequest = read_payload(job)?;
    let class = job.class;
    let mut tx = begin_system(pool)
        .await
        .map_err(|err| Failure::from(MergeQueueError::Store(err)))?;
    let repo = RepoTarget::load(&mut tx, request.repo_id).await?;
    tx.commit()
        .await
        .map_err(|err| Failure::from(MergeQueueError::Query(err)))?;
    let number = request.pr_number;
    let name = request.command.name();
    let refuse = |text: String| reply(scm, &repo.repo, number, text, class);
    let author = ScmUser::from(&request.requested_by);
    if scm.permission(&repo.repo, &author, class).await? < Permission::Write {
        info!(pr_number = number, "command refused: permission");
        return refuse(format!(
            "`{name}` needs the write permission on the repository."
        ))
        .await;
    }
    let Some((queue_id, item_id, state)) = live_item(pool, repo.repo_id, number).await? else {
        return refuse(format!("`{name}`: this PR has no item in a queue.")).await;
    };
    let mut tx = begin_system(pool)
        .await
        .map_err(|err| Failure::from(MergeQueueError::Store(err)))?;
    let target = QueueTarget::load(&mut tx, queue_id).await?;
    tx.commit()
        .await
        .map_err(|err| Failure::from(MergeQueueError::Query(err)))?;
    if !target.settings()?.commands_enabled.value {
        return refuse(format!(
            "`{name}`: the commands are off for the queue `{}`.",
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
                "`{name}`: only an item in test can restart; this one is `{state}`."
            ))
            .await;
        }
    };
    let mut tx = begin_system(pool)
        .await
        .map_err(|err| Failure::from(MergeQueueError::Store(err)))?;
    let user_id = user_of(&mut tx, target.scm_instance_id, &author).await?;
    tx.commit()
        .await
        .map_err(|err| Failure::from(MergeQueueError::Query(err)))?;
    let mut tx = begin_system(pool)
        .await
        .map_err(|err| Failure::from(MergeQueueError::Store(err)))?;
    give_back(
        &mut tx,
        target.org_id,
        queue_id,
        vec![(Actor::User { user_id }, kind)],
        now,
    )
    .await?;
    tx.commit()
        .await
        .map_err(|err| Failure::from(MergeQueueError::Query(err)))?;
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
    let mut tx = begin_system(pool)
        .await
        .map_err(|err| Failure::from(MergeQueueError::Store(err)))?;
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
    .await
    .map_err(|err| Failure::from(MergeQueueError::Query(err)))?;
    tx.commit()
        .await
        .map_err(|err| Failure::from(MergeQueueError::Query(err)))?;
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

//! What each PR shows of its item (`DECISIONS.md` D156): the status of the
//! queue on its head, the comment of the queue and the state labels. The
//! reconciliation renders the surfaces of `plan` into `pr_surfaces`; the job
//! `publish_surfaces` makes the SCM show the last rendering.

use std::fmt::Write as _;

use cihub_config::QueueSettings;
use cihub_domain::{BlockReason, ConnectionId, ItemState, JobClass, OrgId, RepoId, ScmInstanceId};
use cihub_mq_core::Surface;
use cihub_platform::Job;
use cihub_scm::{CommitStatus, SUBMIT_BOX, Scm, ScmError, StatusUpdate};
use cihub_store::{PgPool, begin_system};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::PgConnection;
use time::OffsetDateTime;
use tracing::info;

use crate::MergeQueueError;
use crate::jobs::{Failure, connection_job, job, queue_status_context, read_payload, target_of};
use crate::target::QueueTarget;

/// The type of the job that publishes the surfaces of a PR.
pub(crate) const PUBLISH_SURFACES: &str = "publish_surfaces";

/// The labels that say the state of an item (`docs/GITHUB.md`).
pub(crate) const STATE_LABELS: [&str; 3] = ["cihub:queued", "cihub:testing", "cihub:failed"];

/// A commit status of the SCM is at most this long.
const DESCRIPTION_LIMIT: usize = 140;

/// What a PR must show, as the SCM writes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Rendered {
    /// The head that carries the status.
    pub sha: String,
    pub status: Option<RenderedStatus>,
    pub comment: Option<String>,
    pub add_labels: Vec<String>,
    pub remove_labels: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RenderedStatus {
    pub state: StatusState,
    pub description: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StatusState {
    Pending,
    Success,
    Failure,
    Error,
}

impl From<StatusState> for CommitStatus {
    fn from(state: StatusState) -> Self {
        match state {
            StatusState::Pending => Self::Pending,
            StatusState::Success => Self::Success,
            StatusState::Failure => Self::Failure,
            StatusState::Error => Self::Error,
        }
    }
}

/// The status of the queue on `sha`.
pub(crate) fn queue_status(
    target_branch: &str,
    sha: &str,
    state: StatusState,
    description: &str,
) -> StatusUpdate {
    StatusUpdate {
        sha: sha.to_owned(),
        context: queue_status_context(target_branch),
        status: state.into(),
        description: description.to_owned(),
        // The page of the item arrives with the dashboard (lot 10).
        details_url: None,
    }
}

/// Renders `surface` with the settings of its queue.
pub(crate) fn render(surface: &Surface, settings: &QueueSettings, target_branch: &str) -> Rendered {
    let status = settings.status_check_enabled.value.then(|| RenderedStatus {
        state: status_state(surface.state),
        description: truncate(&description(surface)),
    });
    let comment = settings
        .comments_enabled
        .value
        .then(|| comment(surface, target_branch, settings.commands_enabled.value));
    let (add_labels, remove_labels) = labels(surface, settings.state_labels_enabled.value);
    Rendered {
        sha: surface.head_sha.clone(),
        status,
        comment,
        add_labels,
        remove_labels,
    }
}

/// The hash of a rendering: a new one asks for a new publication.
pub(crate) fn hash(rendered: &Rendered) -> String {
    let bytes = serde_json::to_vec(rendered).unwrap_or_default();
    let digest = Sha256::digest(bytes);
    digest.iter().take(8).fold(String::new(), |mut hex, byte| {
        let _ = write!(hex, "{byte:02x}");
        hex
    })
}

fn status_state(state: ItemState) -> StatusState {
    match state {
        ItemState::NotReady
        | ItemState::Pending
        | ItemState::Testing
        | ItemState::PendingFailure => StatusState::Pending,
        ItemState::TestsPassed | ItemState::Merged => StatusState::Success,
        ItemState::Failed => StatusState::Failure,
        ItemState::Cancelled => StatusState::Error,
    }
}

fn description(surface: &Surface) -> String {
    let at = |text: &str| match surface.position {
        Some(position) => format!("{text}, position {position}"),
        None => text.to_owned(),
    };
    match surface.state {
        ItemState::NotReady => match surface.block_reasons.first() {
            Some(reason) => format!("Waiting: {}", block_text(reason)),
            None => "Waiting to enter the queue".to_owned(),
        },
        ItemState::Pending => at("Queued"),
        ItemState::Testing => at("Testing"),
        ItemState::PendingFailure => at("Tests failed, waiting for the PRs ahead"),
        ItemState::TestsPassed => "Tests passed, merging".to_owned(),
        ItemState::Merged => "Merged by the queue".to_owned(),
        ItemState::Failed => format!("Failed: {}", surface.reason.describe()),
        ItemState::Cancelled => format!("Cancelled: {}", surface.reason.describe()),
    }
}

fn truncate(text: &str) -> String {
    if text.chars().count() <= DESCRIPTION_LIMIT {
        return text.to_owned();
    }
    let mut short: String = text.chars().take(DESCRIPTION_LIMIT - 1).collect();
    short.push('…');
    short
}

fn block_text(reason: &BlockReason) -> String {
    match reason {
        BlockReason::ReviewRequired { have, need } => {
            format!("{have} of {need} required approvals")
        }
        BlockReason::ChangesRequested => "a review asks for changes".to_owned(),
        BlockReason::FailingRequiredCheck { check } => {
            format!("the required check `{check}` failed")
        }
        BlockReason::AwaitingRequiredCheck { check, status } => {
            format!("the required check `{check}` is {status}")
        }
        BlockReason::BehindBase => "the branch is behind its base".to_owned(),
        BlockReason::MergeConflict => "the branch conflicts with its base".to_owned(),
        BlockReason::StackNeedsRebase { onto_pr_number } => {
            format!("the branch must be rebased onto #{onto_pr_number}")
        }
        BlockReason::Undetermined => "the SCM does not say yet why the PR is blocked".to_owned(),
        BlockReason::QueueConditionsUnmet { conditions } => {
            let list: Vec<String> = conditions.iter().map(|c| format!("`{c}`")).collect();
            format!("the queue conditions {} are not met", list.join(", "))
        }
    }
}

/// The comment of the queue on the PR, in Markdown (`DECISIONS.md` D173): the
/// state line, its details, then the commands in small print.
fn comment(surface: &Surface, target_branch: &str, commands: bool) -> String {
    let mut blocks = vec![state_line(surface, target_branch)];
    if surface.state == ItemState::NotReady && !surface.block_reasons.is_empty() {
        let reasons: Vec<String> = surface
            .block_reasons
            .iter()
            .map(|reason| format!("- {}", capitalized(&block_text(reason))))
            .collect();
        blocks.push(reasons.join("\n"));
    }
    let queued = !surface.state.is_terminal() && surface.state != ItemState::NotReady;
    let mut details = Vec::new();
    if !surface.failing_checks.is_empty() && surface.state != ItemState::Merged {
        let checks: Vec<String> = surface
            .failing_checks
            .iter()
            .map(|c| format!("`{c}`"))
            .collect();
        details.push(format!("Failing checks: {}", checks.join(", ")));
    }
    if queued {
        let prs: Vec<String> = surface
            .predecessors
            .iter()
            .map(|n| format!("#{n}"))
            .collect();
        match (prs.is_empty(), &surface.test_branch) {
            (false, Some(branch)) => {
                details.push(format!(
                    "Tested on top of {} in `{branch}`.",
                    prs.join(", ")
                ));
            }
            (false, None) => details.push(format!("Tested on top of {}.", prs.join(", "))),
            (true, Some(branch)) => details.push(format!("Tested in `{branch}`.")),
            (true, None) => {}
        }
        if let Some(number) = surface.draft_pr_number {
            details.push(format!("Draft PR of the test: #{number}"));
        }
    }
    if !details.is_empty() {
        blocks.push(details.join("\n"));
    }
    // Only an item in test can restart (`DECISIONS.md` D157).
    let mut notes = Vec::new();
    match surface.state {
        ItemState::Failed | ItemState::Cancelled if commands => {
            blocks.push(format!("- [ ] {SUBMIT_BOX} Submit again"));
            notes.push("Or comment `/cihub merge`.");
        }
        ItemState::NotReady => {
            if !surface.block_reasons.is_empty() {
                notes.push("The PR enters the queue when nothing above blocks it");
            }
            if commands {
                notes.push("`/cihub cancel` to withdraw");
            }
        }
        ItemState::Pending if commands => notes.push("`/cihub cancel` to leave the queue"),
        ItemState::Testing | ItemState::PendingFailure if commands => {
            notes.push("`/cihub restart` to test again");
            notes.push("`/cihub cancel` to leave the queue");
        }
        _ => {}
    }
    if !notes.is_empty() {
        blocks.push(format!("<sub>{}</sub>", notes.join(" · ")));
    }
    blocks.join("\n\n") + "\n"
}

/// The first line of the comment: an icon, the state, and why.
fn state_line(surface: &Surface, target_branch: &str) -> String {
    let queue = format!("in the queue for `{target_branch}`");
    let at = match surface.position {
        Some(position) => format!("position {position} {queue}"),
        None => queue,
    };
    match surface.state {
        ItemState::NotReady => {
            format!("⏳ **Waiting** · not in the queue for `{target_branch}` yet")
        }
        ItemState::Pending => format!("🕓 **Queued** · {at}"),
        ItemState::Testing => format!("🧪 **Testing** · {at}"),
        ItemState::PendingFailure => {
            "⚠️ **Tests failed** · waiting for the PRs ahead before it fails".to_owned()
        }
        ItemState::TestsPassed => format!("🚀 **Tests passed** · merging into `{target_branch}`"),
        ItemState::Merged => format!("✅ **Merged** into `{target_branch}` by the queue"),
        ItemState::Failed => format!("❌ **Failed** · {}", surface.reason.describe()),
        ItemState::Cancelled => format!("⚪ **Cancelled** · {}", surface.reason.describe()),
    }
}

fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// The state labels to add and to remove. Without `enabled`, only a PR that
/// carries a state label already follows its state. The other state labels
/// are always removed: the last observation of the PR may not show a label
/// that the queue added since, and removing a missing label changes nothing.
fn labels(surface: &Surface, enabled: bool) -> (Vec<String>, Vec<String>) {
    let carried: Vec<&str> = STATE_LABELS
        .into_iter()
        .filter(|label| surface.labels.contains(*label))
        .collect();
    if !enabled && carried.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let wanted = match surface.state {
        ItemState::NotReady | ItemState::Pending => Some(STATE_LABELS[0]),
        ItemState::Testing | ItemState::PendingFailure | ItemState::TestsPassed => {
            Some(STATE_LABELS[1])
        }
        ItemState::Failed => Some(STATE_LABELS[2]),
        ItemState::Merged | ItemState::Cancelled => None,
    };
    let add = wanted
        .filter(|label| !carried.contains(label))
        .map(str::to_owned)
        .into_iter()
        .collect();
    let remove = STATE_LABELS
        .into_iter()
        .filter(|label| Some(*label) != wanted)
        .map(str::to_owned)
        .collect();
    (add, remove)
}

/// Writes the rendering of each surface that changed in `pr_surfaces`, and
/// returns the publication jobs of every PR of the queue whose SCM does not
/// show its last rendering.
///
/// # Errors
///
/// Postgres fails.
pub(crate) async fn converge_surfaces(
    conn: &mut PgConnection,
    target: &QueueTarget,
    settings: &QueueSettings,
    surfaces: &[Surface],
) -> Result<Vec<Job>, MergeQueueError> {
    let known: Vec<(i64, String)> =
        sqlx::query_as("SELECT pr_number, desired_hash FROM pr_surfaces WHERE queue_id = $1")
            .bind(target.queue_id.to_string())
            .fetch_all(&mut *conn)
            .await
            .map_err(MergeQueueError::Query)?;
    for surface in surfaces {
        let rendered = render(surface, settings, &target.target_branch);
        let desired_hash = hash(&rendered);
        let number = i64::try_from(surface.pr_number).unwrap_or(i64::MAX);
        if known
            .iter()
            .any(|(n, h)| *n == number && *h == desired_hash)
        {
            continue;
        }
        let desired = serde_json::to_value(&rendered).map_err(MergeQueueError::Snapshot)?;
        sqlx::query(
            "INSERT INTO pr_surfaces (org_id, queue_id, pr_number, desired, desired_hash)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (queue_id, pr_number) DO UPDATE
             SET desired = EXCLUDED.desired, desired_hash = EXCLUDED.desired_hash",
        )
        .bind(target.org_id.to_string())
        .bind(target.queue_id.to_string())
        .bind(number)
        .bind(desired)
        .bind(&desired_hash)
        .execute(&mut *conn)
        .await
        .map_err(MergeQueueError::Query)?;
    }
    let unpublished: Vec<(i64, String)> = sqlx::query_as(
        "SELECT pr_number, desired_hash FROM pr_surfaces
         WHERE queue_id = $1 AND published_hash IS DISTINCT FROM desired_hash
         ORDER BY pr_number",
    )
    .bind(target.queue_id.to_string())
    .fetch_all(&mut *conn)
    .await
    .map_err(MergeQueueError::Query)?;
    Ok(unpublished
        .into_iter()
        .map(|(number, hash)| publish_job(target, number, &hash))
        .collect())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct PublishSurfaces {
    pr_number: i64,
}

fn publish_job(target: &QueueTarget, pr_number: i64, hash: &str) -> Job {
    let key = format!(
        "{PUBLISH_SURFACES}:{}-{pr_number}-{hash}:1",
        target.queue_id
    );
    let payload = json!(PublishSurfaces { pr_number });
    job(
        target,
        key,
        PUBLISH_SURFACES,
        JobClass::Interactive,
        payload,
    )
}

/// The rendering that a PR must show, what it shows, and its comment.
type SurfaceRow = (serde_json::Value, String, Option<String>, Option<String>);

/// Makes the SCM show the last rendering of the PR. One job at a time
/// publishes for a PR, so that an older rendering never lands after a newer
/// one: the job takes a lock of the PR for its whole run.
pub(crate) async fn publish(
    pool: &PgPool,
    scm: &dyn Scm,
    job: &Job,
    _now: OffsetDateTime,
) -> Result<(), Failure> {
    let request: PublishSurfaces = read_payload(job)?;
    let target = target_of(pool, job).await?;
    let mut lock = begin_system(pool).await?;
    lock_comment(&mut lock, target.repo_id, request.pr_number).await?;
    let row: Option<SurfaceRow> = sqlx::query_as(
        "SELECT s.desired, s.desired_hash, s.published_hash, c.comment_external_id
         FROM pr_surfaces s
         LEFT JOIN pr_comments c ON c.repo_id = $3 AND c.pr_number = s.pr_number
         WHERE s.queue_id = $1 AND s.pr_number = $2",
    )
    .bind(target.queue_id.to_string())
    .bind(request.pr_number)
    .bind(target.repo_id.to_string())
    .fetch_optional(&mut *lock)
    .await?;
    let Some((desired, desired_hash, published, comment_id)) = row else {
        return Ok(());
    };
    if published.as_deref() == Some(desired_hash.as_str()) {
        return Ok(());
    }
    let rendered: Rendered =
        serde_json::from_value(desired).map_err(|err| Failure::Never(err.to_string()))?;
    let number = u64::try_from(request.pr_number).unwrap_or_default();
    let repo = &target.repo;
    let class = job.class;
    if let Some(status) = &rendered.status {
        let update = queue_status(
            &target.target_branch,
            &rendered.sha,
            status.state,
            &status.description,
        );
        scm.set_status(repo, &update, class).await?;
    }
    for label in &rendered.add_labels {
        scm.add_label(repo, number, label, class).await?;
    }
    for label in &rendered.remove_labels {
        scm.remove_label(repo, number, label, class).await?;
    }
    // The comment comes last, just before the commit that keeps its
    // identifier: a stop between them gives a second comment.
    if let Some(body) = &rendered.comment {
        let id = publish_comment(scm, &target, number, body, comment_id, class).await?;
        keep_comment(&mut lock, &target, request.pr_number, &id).await?;
    }
    sqlx::query(
        "UPDATE pr_surfaces SET published_hash = $3 WHERE queue_id = $1 AND pr_number = $2",
    )
    .bind(target.queue_id.to_string())
    .bind(request.pr_number)
    .bind(&desired_hash)
    .execute(&mut *lock)
    .await?;
    lock.commit().await?;
    info!(
        pr_number = number,
        hash = desired_hash,
        "surfaces published"
    );
    Ok(())
}

/// Takes the lock of the comment of the PR `pr_number` of `repo` for the
/// transaction: one job at a time writes the comment of a PR, whatever its
/// queue (`DECISIONS.md` D163).
async fn lock_comment(
    conn: &mut PgConnection,
    repo: RepoId,
    pr_number: i64,
) -> Result<(), Failure> {
    let locked: bool =
        sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtext($1), $2::integer)")
            .bind(format!("pr_comments:{repo}"))
            .bind(pr_number)
            .fetch_one(conn)
            .await?;
    if locked {
        Ok(())
    } else {
        Err(Failure::Retry(
            "another job writes the comment of this PR".into(),
        ))
    }
}

/// Keeps `id` as the comment of the PR.
async fn keep_comment(
    conn: &mut PgConnection,
    target: &QueueTarget,
    pr_number: i64,
    id: &str,
) -> Result<(), Failure> {
    sqlx::query(
        "INSERT INTO pr_comments (org_id, repo_id, pr_number, comment_external_id)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (repo_id, pr_number) DO UPDATE
         SET comment_external_id = EXCLUDED.comment_external_id",
    )
    .bind(target.org_id.to_string())
    .bind(target.repo_id.to_string())
    .bind(pr_number)
    .bind(id)
    .execute(conn)
    .await?;
    Ok(())
}

/// Updates the comment of the queue, or creates it, and returns its
/// identifier.
async fn publish_comment(
    scm: &dyn Scm,
    target: &QueueTarget,
    number: u64,
    body: &str,
    known: Option<String>,
    class: JobClass,
) -> Result<String, Failure> {
    let repo = &target.repo;
    if let Some(id) = known {
        match scm.update_comment(repo, &id, body, class).await {
            Ok(()) => return Ok(id),
            // Someone deleted the comment: the queue writes a new one.
            Err(ScmError::NotFound(_)) => {}
            Err(err) => return Err(err.into()),
        }
    }
    Ok(scm.create_comment(repo, number, body, class).await?)
}

/// The status `success` before the merge of the PR (`DECISIONS.md` D156): a
/// rule that requires the status of the queue lets the merge through.
pub(crate) fn merging_status(target_branch: &str, sha: &str) -> StatusUpdate {
    queue_status(
        target_branch,
        sha,
        StatusState::Success,
        "Tests passed, merging",
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use cihub_config::{Layer, Setting};
    use cihub_domain::{ItemId, Reason};
    use ulid::Ulid;

    use super::*;

    fn surface(state: ItemState) -> Surface {
        Surface {
            item_id: ItemId::from_ulid(Ulid(1)),
            pr_number: 7,
            head_sha: "abc".into(),
            state,
            reason: Reason::Submitted,
            position: Some(2),
            predecessors: vec![5],
            test_branch: Some("cihub-merge/main/run".into()),
            draft_pr_number: None,
            failing_checks: Vec::new(),
            block_reasons: Vec::new(),
            labels: BTreeSet::new(),
        }
    }

    fn settings(edit: impl FnOnce(&mut Layer)) -> QueueSettings {
        let mut layer = Layer::default();
        edit(&mut layer);
        QueueSettings::resolve(&Layer::default(), &layer).unwrap()
    }

    #[test]
    fn each_state_gives_its_status() {
        let defaults = settings(|_| {});
        let cases = [
            (ItemState::NotReady, StatusState::Pending),
            (ItemState::Testing, StatusState::Pending),
            (ItemState::TestsPassed, StatusState::Success),
            (ItemState::Merged, StatusState::Success),
            (ItemState::Failed, StatusState::Failure),
            (ItemState::Cancelled, StatusState::Error),
        ];
        for (state, want) in cases {
            let rendered = render(&surface(state), &defaults, "main");
            assert_eq!(rendered.status.unwrap().state, want, "{state:?}");
            assert_eq!(rendered.sha, "abc");
        }
    }

    #[test]
    fn each_state_gives_its_comment() {
        let mut blocked = surface(ItemState::NotReady);
        blocked.block_reasons = vec![
            BlockReason::ReviewRequired { have: 1, need: 2 },
            BlockReason::AwaitingRequiredCheck {
                check: "build".into(),
                status: "in progress".into(),
            },
        ];
        let mut testing = surface(ItemState::Testing);
        testing.draft_pr_number = Some(42);
        let mut retrying = surface(ItemState::PendingFailure);
        retrying.failing_checks = vec!["build".into()];
        let mut failed = surface(ItemState::Failed);
        failed.reason = Reason::TestRunRequiredStatusFailed;
        failed.failing_checks = vec!["build".into(), "lint".into()];
        let mut cancelled = surface(ItemState::Cancelled);
        cancelled.reason = Reason::PrPushedTo;
        let cases = [
            (
                blocked,
                "⏳ **Waiting** · not in the queue for `main` yet\n\n\
                 - 1 of 2 required approvals\n\
                 - The required check `build` is in progress\n\n\
                 <sub>The PR enters the queue when nothing above blocks it · `/cihub cancel` to withdraw</sub>\n",
            ),
            (
                surface(ItemState::Pending),
                "🕓 **Queued** · position 2 in the queue for `main`\n\n\
                 Tested on top of #5 in `cihub-merge/main/run`.\n\n\
                 <sub>`/cihub cancel` to leave the queue</sub>\n",
            ),
            (
                testing,
                "🧪 **Testing** · position 2 in the queue for `main`\n\n\
                 Tested on top of #5 in `cihub-merge/main/run`.\n\
                 Draft PR of the test: #42\n\n\
                 <sub>`/cihub restart` to test again · `/cihub cancel` to leave the queue</sub>\n",
            ),
            (
                retrying,
                "⚠️ **Tests failed** · waiting for the PRs ahead before it fails\n\n\
                 Failing checks: `build`\n\
                 Tested on top of #5 in `cihub-merge/main/run`.\n\n\
                 <sub>`/cihub restart` to test again · `/cihub cancel` to leave the queue</sub>\n",
            ),
            (
                surface(ItemState::TestsPassed),
                "🚀 **Tests passed** · merging into `main`\n\n\
                 Tested on top of #5 in `cihub-merge/main/run`.\n",
            ),
            (
                surface(ItemState::Merged),
                "✅ **Merged** into `main` by the queue\n",
            ),
            (
                failed,
                "❌ **Failed** · a required check failed\n\n\
                 Failing checks: `build`, `lint`\n\n\
                 - [ ] <!-- cihub:submit --> Submit again\n\n\
                 <sub>Or comment `/cihub merge`.</sub>\n",
            ),
            (
                cancelled,
                "⚪ **Cancelled** · the PR received a new commit after its submission\n\n\
                 - [ ] <!-- cihub:submit --> Submit again\n\n\
                 <sub>Or comment `/cihub merge`.</sub>\n",
            ),
        ];
        let defaults = settings(|_| {});
        for (surface, want) in cases {
            let comment = render(&surface, &defaults, "main").comment.unwrap();
            assert_eq!(comment, want, "{:?}", surface.state);
        }
    }

    #[test]
    fn without_commands_the_comment_names_none() {
        let off = settings(|l| l.commands_enabled = Some(Setting::new(false)));
        for state in [ItemState::Testing, ItemState::Failed] {
            let comment = render(&surface(state), &off, "main").comment.unwrap();
            assert!(!comment.contains("/cihub"), "{comment}");
            assert!(!comment.contains(SUBMIT_BOX), "{comment}");
        }
    }

    #[test]
    fn a_failure_shows_the_sentence_of_its_reason() {
        let mut failed = surface(ItemState::Failed);
        failed.reason = Reason::PrPushedTo;
        let rendered = render(&failed, &settings(|_| {}), "main");
        let want = "Failed: the PR received a new commit after its submission";
        assert_eq!(rendered.status.unwrap().description, want);
        let comment = rendered.comment.unwrap();
        assert!(
            comment
                .starts_with("❌ **Failed** · the PR received a new commit after its submission\n"),
            "{comment}"
        );
    }

    #[test]
    fn the_settings_turn_off_the_status_and_the_comment() {
        let off = settings(|l| {
            l.status_check_enabled = Some(Setting::new(false));
            l.comments_enabled = Some(Setting::new(false));
        });
        let rendered = render(&surface(ItemState::Pending), &off, "main");
        assert_eq!((rendered.status, rendered.comment), (None, None));
    }

    #[test]
    fn state_labels_follow_the_state_of_a_pr_that_carries_one() {
        let off = settings(|_| {});
        let on = settings(|l| l.state_labels_enabled = Some(Setting::new(true)));
        let mut testing = surface(ItemState::Testing);
        let labels = |rendered: Rendered| (rendered.add_labels, rendered.remove_labels);
        assert_eq!(labels(render(&testing, &off, "main")), (vec![], vec![]));
        let others = |labels: &[&str]| labels.iter().map(|l| (*l).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            labels(render(&testing, &on, "main")),
            (
                others(&["cihub:testing"]),
                others(&["cihub:queued", "cihub:failed"])
            )
        );
        testing.labels = BTreeSet::from(["cihub:queued".to_owned(), "bug".to_owned()]);
        assert_eq!(
            labels(render(&testing, &off, "main")),
            (
                others(&["cihub:testing"]),
                others(&["cihub:queued", "cihub:failed"])
            )
        );
        let mut merged = surface(ItemState::Merged);
        merged.labels = BTreeSet::from(["cihub:testing".to_owned()]);
        assert_eq!(
            labels(render(&merged, &off, "main")),
            (vec![], others(&STATE_LABELS))
        );
    }

    #[test]
    fn a_long_description_is_cut_and_a_change_gives_a_new_hash() {
        let mut blocked = surface(ItemState::NotReady);
        blocked.block_reasons = vec![BlockReason::QueueConditionsUnmet {
            conditions: vec!["x".repeat(200)],
        }];
        let rendered = render(&blocked, &settings(|_| {}), "main");
        let description = rendered.status.clone().unwrap().description;
        assert_eq!(description.chars().count(), DESCRIPTION_LIMIT);
        let other = render(&surface(ItemState::Pending), &settings(|_| {}), "main");
        assert_ne!(hash(&rendered), hash(&other));
        assert_eq!(hash(&rendered), hash(&rendered.clone()));
    }
}

/// The type of the job that posts the welcome comment of a PR
/// (`DECISIONS.md` D163).
pub(crate) const WELCOME_PR: &str = "welcome_pr";

/// The payload of `welcome_pr`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct WelcomePr {
    pub repo_id: RepoId,
    pub pr_number: u64,
    pub base_branch: String,
}

/// The welcome of `request` after the delivery `delivery_id`.
pub(crate) fn welcome_job(
    org_id: OrgId,
    connection: (ScmInstanceId, ConnectionId),
    delivery_id: &str,
    request: &WelcomePr,
) -> Job {
    connection_job(org_id, connection, delivery_id, WELCOME_PR, json!(request))
}

/// The comment that welcomes a PR on a branch of a queue, with its
/// submission box.
pub(crate) fn welcome(target_branch: &str) -> String {
    format!(
        "CIHub manages merges into `{target_branch}`.\n\n\
         - [ ] {SUBMIT_BOX} Add this PR to the merge queue\n\n\
         <sub>Or comment `/cihub merge`.</sub>\n"
    )
}

/// Posts the welcome comment of the PR, once: a PR that has a comment of
/// CIHub already keeps it. The first active queue of the branch decides
/// whether the comment and the commands are on.
pub(crate) async fn post_welcome(
    pool: &PgPool,
    scm: &dyn Scm,
    job: &Job,
    _now: OffsetDateTime,
) -> Result<(), Failure> {
    let request: WelcomePr = read_payload(job)?;
    let mut lock = begin_system(pool).await?;
    let queues =
        QueueTarget::queues_of_branch(&mut lock, request.repo_id, &request.base_branch).await?;
    let Some(target) = queues.into_iter().find(|queue| queue.enabled) else {
        return Ok(());
    };
    let settings = target.settings()?;
    if !(settings.comments_enabled.value && settings.commands_enabled.value) {
        return Ok(());
    }
    let number = i64::try_from(request.pr_number).unwrap_or(i64::MAX);
    lock_comment(&mut lock, target.repo_id, number).await?;
    let known: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pr_comments WHERE repo_id = $1 AND pr_number = $2)",
    )
    .bind(target.repo_id.to_string())
    .bind(number)
    .fetch_one(&mut *lock)
    .await?;
    if known {
        return Ok(());
    }
    let body = welcome(&target.target_branch);
    let id = scm
        .create_comment(&target.repo, request.pr_number, &body, job.class)
        .await?;
    keep_comment(&mut lock, &target, number, &id).await?;
    lock.commit().await?;
    info!(pr_number = request.pr_number, "welcome comment posted");
    Ok(())
}

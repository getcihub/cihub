//! What each PR shows of its item (`DECISIONS.md` D156): the status of the
//! queue on its head, the comment of the queue and the state labels. The
//! reconciliation renders the surfaces of `plan` into `pr_surfaces`; the job
//! `publish_surfaces` makes the SCM show the last rendering.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use cihub_config::QueueSettings;
use cihub_domain::{BlockReason, EventKind, EventType, ItemState, JobClass, Reason, RepoId};
use cihub_mq_core::{BatchWait, Surface};
use cihub_platform::Job;
use cihub_scm::{CommitStatus, SUBMIT_BOX, Scm, ScmError, StatusUpdate};
use cihub_store::{PgPool, begin_system, event_kind};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgConnection;
use time::{OffsetDateTime, UtcOffset};
use tracing::info;

use crate::MergeQueueError;
use crate::jobs::{Failure, job, queue_status_context, read_payload, target_of};
use crate::target::QueueTarget;

/// The type of the job that publishes the surfaces of a PR.
pub(crate) const PUBLISH_SURFACES: &str = "publish_surfaces";

/// The labels that say the state of an item (`docs/GITHUB.md`).
const STATE_LABELS: [&str; 3] = ["cihub:queued", "cihub:testing", "cihub:failed"];

/// A commit status of the SCM is at most this long.
const DESCRIPTION_LIMIT: usize = 140;

/// The events of its items that the history of a PR shows (`DECISIONS.md`
/// D244): the decisions. A request of a user is not a row: the decision that
/// answers it names the user. The history also shows the test branches of the
/// runs of the PR (`DECISIONS.md` D245).
const HISTORY_EVENTS: [EventType; 2] =
    [EventType::ItemTransitioned, EventType::ItemReadinessChanged];

/// The comment shows the last rows of the history, then counts the others.
const HISTORY_ROWS: usize = 20;

/// What a PR must show, as the SCM writes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Rendered {
    /// The head that carries the status.
    pub sha: String,
    pub status: Option<RenderedStatus>,
    pub comment: Option<String>,
    pub add_labels: Vec<String>,
    pub remove_labels: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct RenderedStatus {
    pub state: StatusState,
    pub description: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StatusState {
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
fn queue_status(
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

/// Renders `surface` with the settings of its queue and the logged `history`
/// of its PR.
fn render(
    surface: &Surface,
    settings: &QueueSettings,
    target_branch: &str,
    history: &[HistoryEvent],
) -> Rendered {
    let status = settings.status_check_enabled.value.then(|| RenderedStatus {
        state: status_state(surface.state),
        description: truncate(&description(surface)),
    });
    let comment = settings
        .comments_enabled
        .value
        .then(|| comment(surface, settings, target_branch, history));
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
fn hash(rendered: &Rendered) -> String {
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
/// state line, its details, the history (`DECISIONS.md` D244), then the
/// commands in small print.
fn comment(
    surface: &Surface,
    settings: &QueueSettings,
    target_branch: &str,
    history: &[HistoryEvent],
) -> String {
    let commands = settings.commands_enabled.value;
    let mut blocks = vec![state_line(surface, target_branch)];

    if let Some(wait) = &surface.batch_wait {
        blocks.push(batch_wait_line(wait, settings.batching_size.value));
    }

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
        // Where the CI of the test shows: its draft PR, else its commit;
        // GitHub links both.
        let test = surface
            .draft_pr_number
            .map(|number| format!("#{number}"))
            .or_else(|| surface.test_sha.clone());
        match (prs.is_empty(), test) {
            (false, Some(test)) => {
                details.push(format!("Tested on top of {} in {test}.", prs.join(", ")));
            }
            (false, None) => details.push(format!("Tested on top of {}.", prs.join(", "))),
            (true, Some(test)) => details.push(format!("Tested in {test}.")),
            (true, None) => {}
        }
    }

    if !details.is_empty() {
        blocks.push(details.join("\n"));
    }

    blocks.extend(history_block(history));

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
    let (icon, label) = state_mark(surface.state);

    match surface.state {
        ItemState::NotReady => {
            format!("{icon} **{label}** · not in the queue for `{target_branch}` yet")
        }
        ItemState::Pending | ItemState::Testing => format!("{icon} **{label}** · {at}"),
        ItemState::PendingFailure => {
            format!("{icon} **{label}** · waiting for the PRs ahead before it fails")
        }
        ItemState::TestsPassed => format!("{icon} **{label}** · merging into `{target_branch}`"),
        ItemState::Merged => format!("{icon} **{label}** into `{target_branch}` by the queue"),
        ItemState::Failed | ItemState::Cancelled => {
            format!("{icon} **{label}** · {}", surface.reason.describe())
        }
    }
}

/// The icon and the label of a state, in the comment and in its history.
fn state_mark(state: ItemState) -> (&'static str, &'static str) {
    match state {
        ItemState::NotReady => ("⏳", "Waiting"),
        ItemState::Pending => ("🕓", "Queued"),
        ItemState::Testing => ("🧪", "Testing"),
        ItemState::PendingFailure => ("⚠️", "Tests failed"),
        ItemState::TestsPassed => ("🚀", "Tests passed"),
        ItemState::Merged => ("✅", "Merged"),
        ItemState::Failed => ("❌", "Failed"),
        ItemState::Cancelled => ("⚪", "Cancelled"),
    }
}

/// An event of the log of a PR, as its history reads it.
#[derive(Debug)]
struct HistoryEvent {
    pub occurred_at: OffsetDateTime,
    pub kind: EventKind,
    /// The login of the user whose request caused the event; `None` for the
    /// system and the bot.
    pub login: Option<String>,
    /// For a test branch: the other PRs of its group, then the PRs ahead of
    /// the group, in order. Empty for another event.
    pub tested_with: Vec<i64>,
    pub tested_on_top_of: Vec<i64>,
}

/// The history of a PR as a folded table (`DECISIONS.md` D244), `None` without
/// a row. Two consecutive events with the same text and the same user are one
/// row; the comment shows the last rows and counts the older ones.
fn history_block(events: &[HistoryEvent]) -> Option<String> {
    let mut rows: Vec<(&HistoryEvent, String)> = events
        .iter()
        .filter_map(|event| Some((event, event_text(event)?)))
        .collect();

    rows.dedup_by(|later, kept| later.1 == kept.1 && later.0.login == kept.0.login);

    if rows.is_empty() {
        return None;
    }

    let total = rows.len();
    let older = total.saturating_sub(HISTORY_ROWS);
    // GitHub renders the table only with a blank line after `</summary>` and
    // before `</details>`.
    let mut block = format!("<details><summary>History ({total})</summary>\n\n");

    match older {
        0 => {}
        1 => block.push_str("1 older row.\n\n"),
        n => {
            let _ = write!(block, "{n} older rows.\n\n");
        }
    }

    block.push_str("| Time (UTC) | Event | By |\n| --- | --- | --- |\n");

    for (event, text) in rows.iter().skip(older) {
        let _ = writeln!(
            block,
            "| {} | {} | {} |",
            utc_minute(event.occurred_at),
            cell(text),
            by_cell(event.login.as_deref())
        );
    }

    block.push_str("\n</details>");

    Some(block)
}

/// The sentence of a row, `None` for an event that the history does not show.
fn event_text(event: &HistoryEvent) -> Option<String> {
    match &event.kind {
        EventKind::ItemTransitioned {
            to,
            reason,
            merged_sha,
            ..
        } => {
            let text = transition_text(*to, *reason);
            // The SCM links a full SHA.
            Some(match merged_sha {
                Some(sha) => format!("{text}, as {sha}"),
                None => text,
            })
        }
        EventKind::ItemReadinessChanged { block_reasons, .. } => {
            Some(readiness_text(block_reasons))
        }
        EventKind::TestBranchAssembled {
            test_sha,
            draft_pr_number,
            ..
        } => Some(test_branch_text(
            test_sha,
            *draft_pr_number,
            &event.tested_with,
            &event.tested_on_top_of,
        )),
        _ => None,
    }
}

/// A mention notifies its user: the code span names the user without it
/// (`DECISIONS.md` D244).
fn by_cell(login: Option<&str>) -> String {
    login.map_or_else(String::new, |login| format!("`@{login}`"))
}

fn transition_text(to: ItemState, reason: Reason) -> String {
    let (icon, label) = state_mark(to);
    format!("{icon} {label}: {}", reason.describe())
}

/// Where the CI of a test runs, as the details of the comment say it.
fn test_branch_text(
    test_sha: &str,
    draft_pr_number: Option<u64>,
    with: &[i64],
    on_top_of: &[i64],
) -> String {
    let prs = |numbers: &[i64]| -> String {
        let numbers: Vec<String> = numbers.iter().map(|n| format!("#{n}")).collect();
        numbers.join(", ")
    };
    let mut text = match draft_pr_number {
        Some(number) => format!("🧪 Test branch ready: the CI runs in #{number}"),
        None => format!("🧪 Test branch ready: the CI runs on {test_sha}"),
    };

    if !with.is_empty() {
        let _ = write!(text, ", with {}", prs(with));
    }

    if !on_top_of.is_empty() {
        let _ = write!(text, ", on top of {}", prs(on_top_of));
    }

    text
}

fn readiness_text(block_reasons: &[BlockReason]) -> String {
    if block_reasons.is_empty() {
        return "✅ Nothing blocks the PR".to_owned();
    }
    let reasons: Vec<String> = block_reasons.iter().map(block_text).collect();
    format!("⏳ Waiting: {}", reasons.join("; "))
}

/// `YYYY-MM-DD HH:MM` in UTC: an absolute time does not go stale between two
/// edits of the comment.
fn utc_minute(at: OffsetDateTime) -> String {
    let at = at.to_offset(UtcOffset::UTC);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        at.year(),
        u8::from(at.month()),
        at.day(),
        at.hour(),
        at.minute()
    )
}

/// A check name or a condition can hold `|`, and a newline ends a row.
fn cell(text: &str) -> String {
    text.replace('|', "\\|").replace(['\r', '\n'], " ")
}

/// When the batch stops waiting for more PRs, never when its test starts: a
/// closed batch still waits for room in the window (`DECISIONS.md` D241). An
/// absolute time does not go stale between two edits of the comment; the
/// seconds are dropped, so the batch closes during the minute shown.
fn batch_wait_line(wait: &BatchWait, size: u32) -> String {
    let prs = if wait.members == 1 { "PR" } else { "PRs" };
    let at = wait.closes_at.to_offset(UtcOffset::UTC);
    format!(
        "Batch of {} {prs}: it waits for more PRs until {:02}:{:02} UTC, or until it has {size}.",
        wait.members,
        at.hour(),
        at.minute()
    )
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
    let numbers: Vec<i64> = surfaces
        .iter()
        .map(|surface| i64::try_from(surface.pr_number).unwrap_or(i64::MAX))
        .collect();
    // Without a comment, the history has no use (`DECISIONS.md` D244).
    let histories = if settings.comments_enabled.value {
        read_histories(conn, target, &numbers).await?
    } else {
        BTreeMap::new()
    };
    let known: Vec<(i64, String)> =
        sqlx::query_as("SELECT pr_number, desired_hash FROM pr_surfaces WHERE queue_id = $1")
            .bind(target.queue_id.to_string())
            .fetch_all(&mut *conn)
            .await?;

    for (surface, number) in surfaces.iter().zip(numbers) {
        let history = histories.get(&number).map_or(&[][..], Vec::as_slice);
        let rendered = render(surface, settings, &target.target_branch, history);
        let desired_hash = hash(&rendered);
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
        .await?;
    }

    let unpublished: Vec<(i64, String)> = sqlx::query_as(
        "SELECT pr_number, desired_hash FROM pr_surfaces
         WHERE queue_id = $1 AND published_hash IS DISTINCT FROM desired_hash
         ORDER BY pr_number",
    )
    .bind(target.queue_id.to_string())
    .fetch_all(&mut *conn)
    .await?;

    Ok(unpublished
        .into_iter()
        .map(|(number, hash)| publish_job(target, number, &hash))
        .collect())
}

/// An event of the history, as `read_histories` selects it: its PR, id, type,
/// payload and time, the login of the user whose request caused it, and the
/// PRs tested with it and before it.
type HistoryRow = (
    i64,
    String,
    String,
    Value,
    OffsetDateTime,
    Option<String>,
    Vec<i64>,
    Vec<i64>,
);

/// The events of the history of the PRs `numbers` of the queue, by PR, in the
/// order of the log (`DECISIONS.md` D244). All the items of a PR count, not
/// only the current one. The reading starts from these items: their events
/// come through `events.item_id`. A decision has the system as actor; the
/// user is the actor of its cause. The test branches come through the runs
/// that tested the PR (`DECISIONS.md` D245).
async fn read_histories(
    conn: &mut PgConnection,
    target: &QueueTarget,
    numbers: &[i64],
) -> Result<BTreeMap<i64, Vec<HistoryEvent>>, MergeQueueError> {
    let types: Vec<&str> = HISTORY_EVENTS.into_iter().map(EventType::as_str).collect();
    let rows: Vec<HistoryRow> = sqlx::query_as(
        "SELECT pr_number, id, type, payload, occurred_at, login, tested_with, tested_on_top_of
         FROM (
         SELECT i.pr_number, e.id, e.type::text AS type, e.payload, e.occurred_at, u.login,
                '{}'::bigint[] AS tested_with, '{}'::bigint[] AS tested_on_top_of, e.seq
         FROM queue_items i
         JOIN events e ON e.item_id = i.id AND e.type = ANY($2::event_type[])
         LEFT JOIN events c ON c.id = e.cause_id
         LEFT JOIN users u ON u.id = c.actor_user_id
         WHERE i.queue_id = $1 AND i.pr_number = ANY($3)
         UNION ALL
         SELECT n.pr_number, e.id, e.type::text, e.payload, e.occurred_at, NULL,
                array_remove(r.tested_pr_numbers, n.pr_number),
                ARRAY(SELECT p.pr_number
                      FROM unnest(g.predecessor_ids) WITH ORDINALITY AS a (id, rank)
                      JOIN queue_items p ON p.id = a.id
                      ORDER BY a.rank),
                e.seq
         FROM test_runs r
         CROSS JOIN LATERAL unnest(r.tested_pr_numbers) AS n (pr_number)
         JOIN test_groups g ON g.id = r.group_id
         JOIN events e ON e.run_id = r.id AND e.type = 'test_branch.assembled'
         WHERE r.queue_id = $1 AND r.tested_pr_numbers && $3 AND n.pr_number = ANY($3)
         ) history
         ORDER BY seq",
    )
    .bind(target.queue_id.to_string())
    .bind(&types)
    .bind(numbers)
    .fetch_all(conn)
    .await?;
    let mut histories: BTreeMap<i64, Vec<HistoryEvent>> = BTreeMap::new();

    for (number, id, kind, payload, occurred_at, login, tested_with, tested_on_top_of) in rows {
        let kind = event_kind(&kind, &payload)
            .map_err(|source| MergeQueueError::ReadEvent { id, source })?;
        histories.entry(number).or_default().push(HistoryEvent {
            occurred_at,
            kind,
            login,
            tested_with,
            tested_on_top_of,
        });
    }

    Ok(histories)
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
    use cihub_domain::{ItemId, JobError, Mergeability, Reason, RequiredStatusesSource, RunId};
    use ulid::Ulid;

    use super::*;

    fn surface(state: ItemState) -> Surface {
        Surface {
            pr_number: 7,
            head_sha: "abc".into(),
            state,
            reason: Reason::Submitted,
            position: Some(2),
            predecessors: vec![5],
            test_sha: Some("def456".into()),
            draft_pr_number: None,
            failing_checks: Vec::new(),
            block_reasons: Vec::new(),
            labels: BTreeSet::new(),
            batch_wait: None,
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
            let rendered = render(&surface(state), &defaults, "main", &[]);
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
                 Tested on top of #5 in def456.\n\n\
                 <sub>`/cihub cancel` to leave the queue</sub>\n",
            ),
            (
                testing,
                "🧪 **Testing** · position 2 in the queue for `main`\n\n\
                 Tested on top of #5 in #42.\n\n\
                 <sub>`/cihub restart` to test again · `/cihub cancel` to leave the queue</sub>\n",
            ),
            (
                retrying,
                "⚠️ **Tests failed** · waiting for the PRs ahead before it fails\n\n\
                 Failing checks: `build`\n\
                 Tested on top of #5 in def456.\n\n\
                 <sub>`/cihub restart` to test again · `/cihub cancel` to leave the queue</sub>\n",
            ),
            (
                surface(ItemState::TestsPassed),
                "🚀 **Tests passed** · merging into `main`\n\n\
                 Tested on top of #5 in def456.\n",
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
            let comment = render(&surface, &defaults, "main", &[]).comment.unwrap();
            assert_eq!(comment, want, "{:?}", surface.state);
        }
    }

    /// `DECISIONS.md` D241.
    #[test]
    fn a_waiting_batch_says_when_it_closes() {
        let closes_at = time::Date::from_calendar_date(2026, time::Month::October, 8)
            .unwrap()
            .with_hms(12, 40, 59)
            .unwrap()
            .assume_offset(UtcOffset::from_hms(2, 0, 0).unwrap());
        let size_3 = settings(|l| l.batching_size = Some(Setting::new(3)));
        let cases = [
            (
                2,
                "🕓 **Queued** · position 1 in the queue for `main`\n\n\
                 Batch of 2 PRs: it waits for more PRs until 10:40 UTC, or until it has 3.\n\n\
                 <sub>`/cihub cancel` to leave the queue</sub>\n",
            ),
            (
                1,
                "🕓 **Queued** · position 1 in the queue for `main`\n\n\
                 Batch of 1 PR: it waits for more PRs until 10:40 UTC, or until it has 3.\n\n\
                 <sub>`/cihub cancel` to leave the queue</sub>\n",
            ),
        ];

        for (members, want) in cases {
            let mut queued = surface(ItemState::Pending);
            queued.position = Some(1);
            queued.predecessors = Vec::new();
            queued.test_sha = None;
            queued.batch_wait = Some(BatchWait { members, closes_at });
            let rendered = render(&queued, &size_3, "main", &[]);
            assert_eq!(rendered.comment.unwrap(), want, "{members}");
            let status = rendered.status.unwrap().description;
            assert_eq!(status, "Queued, position 1", "the status does not change");
        }
    }

    #[test]
    fn without_commands_the_comment_names_none() {
        let off = settings(|l| l.commands_enabled = Some(Setting::new(false)));
        for state in [ItemState::Testing, ItemState::Failed] {
            let comment = render(&surface(state), &off, "main", &[]).comment.unwrap();
            assert!(!comment.contains("/cihub"), "{comment}");
            assert!(!comment.contains(SUBMIT_BOX), "{comment}");
        }
    }

    #[test]
    fn a_failure_shows_the_sentence_of_its_reason() {
        let mut failed = surface(ItemState::Failed);
        failed.reason = Reason::PrPushedTo;

        let rendered = render(&failed, &settings(|_| {}), "main", &[]);
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
        let rendered = render(&surface(ItemState::Pending), &off, "main", &[]);
        assert_eq!((rendered.status, rendered.comment), (None, None));
    }

    #[test]
    fn state_labels_follow_the_state_of_a_pr_that_carries_one() {
        let off = settings(|_| {});
        let on = settings(|l| l.state_labels_enabled = Some(Setting::new(true)));
        let mut testing = surface(ItemState::Testing);
        let labels = |rendered: Rendered| (rendered.add_labels, rendered.remove_labels);

        assert_eq!(
            labels(render(&testing, &off, "main", &[])),
            (vec![], vec![])
        );

        let others = |labels: &[&str]| labels.iter().map(|l| (*l).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            labels(render(&testing, &on, "main", &[])),
            (
                others(&["cihub:testing"]),
                others(&["cihub:queued", "cihub:failed"])
            )
        );

        testing.labels = BTreeSet::from(["cihub:queued".to_owned(), "bug".to_owned()]);

        assert_eq!(
            labels(render(&testing, &off, "main", &[])),
            (
                others(&["cihub:testing"]),
                others(&["cihub:queued", "cihub:failed"])
            )
        );

        let mut merged = surface(ItemState::Merged);
        merged.labels = BTreeSet::from(["cihub:testing".to_owned()]);

        assert_eq!(
            labels(render(&merged, &off, "main", &[])),
            (vec![], others(&STATE_LABELS))
        );
    }

    #[test]
    fn a_long_description_is_cut_and_a_change_gives_a_new_hash() {
        let mut blocked = surface(ItemState::NotReady);
        blocked.block_reasons = vec![BlockReason::QueueConditionsUnmet {
            conditions: vec!["x".repeat(200)],
        }];

        let rendered = render(&blocked, &settings(|_| {}), "main", &[]);
        let description = rendered.status.clone().unwrap().description;

        assert_eq!(description.chars().count(), DESCRIPTION_LIMIT);

        let other = render(&surface(ItemState::Pending), &settings(|_| {}), "main", &[]);
        assert_ne!(hash(&rendered), hash(&other));
        assert_eq!(hash(&rendered), hash(&rendered.clone()));
    }

    fn at(day: u8, hour: u8, minute: u8) -> OffsetDateTime {
        time::Date::from_calendar_date(2026, time::Month::October, day)
            .unwrap()
            .with_hms(hour, minute, 30)
            .unwrap()
            .assume_utc()
    }

    fn logged(kind: EventKind, login: Option<&str>, occurred_at: OffsetDateTime) -> HistoryEvent {
        HistoryEvent {
            occurred_at,
            kind,
            login: login.map(str::to_owned),
            tested_with: Vec::new(),
            tested_on_top_of: Vec::new(),
        }
    }

    fn transitioned(to: ItemState, reason: Reason) -> EventKind {
        EventKind::ItemTransitioned {
            item_id: ItemId::from_ulid(Ulid(1)),
            from: None,
            to,
            reason,
            position: None,
            verified_by_run_id: None,
            merged_sha: None,
        }
    }

    fn readiness(block_reasons: Vec<BlockReason>) -> EventKind {
        EventKind::ItemReadinessChanged {
            item_id: ItemId::from_ulid(Ulid(1)),
            mergeability: Mergeability::Mergeable,
            block_reasons,
        }
    }

    /// The text of the rows of a history, without its frame.
    fn rows_of(history: &[HistoryEvent]) -> Vec<String> {
        let block = history_block(history).unwrap();
        block
            .lines()
            .filter(|line| line.starts_with("| 2026"))
            .map(str::to_owned)
            .collect()
    }

    /// `DECISIONS.md` D244.
    #[test]
    fn each_decision_gives_its_row() {
        let cases = [
            (
                transitioned(ItemState::NotReady, Reason::Submitted),
                Some("nlecoy"),
                "| 2026-10-07 14:03 | ⏳ Waiting: the PR was submitted to the queue | `@nlecoy` |",
            ),
            (
                transitioned(ItemState::Testing, Reason::TestRunStarted),
                None,
                "| 2026-10-07 14:03 | 🧪 Testing: a test run started |  |",
            ),
            (
                transitioned(ItemState::Failed, Reason::PrPushedTo),
                None,
                "| 2026-10-07 14:03 | ❌ Failed: the PR received a new commit after its submission |  |",
            ),
            (
                readiness(vec![BlockReason::ReviewRequired { have: 1, need: 2 }]),
                None,
                "| 2026-10-07 14:03 | ⏳ Waiting: 1 of 2 required approvals |  |",
            ),
            (
                readiness(vec![BlockReason::ChangesRequested, BlockReason::BehindBase]),
                None,
                "| 2026-10-07 14:03 | ⏳ Waiting: a review asks for changes; the branch is behind its base |  |",
            ),
            (
                readiness(Vec::new()),
                None,
                "| 2026-10-07 14:03 | ✅ Nothing blocks the PR |  |",
            ),
        ];
        for (kind, actor, want) in cases {
            let rows = rows_of(&[logged(kind.clone(), actor, at(7, 14, 3))]);
            assert_eq!(rows, [want], "{kind:?}");
        }
    }

    /// `DECISIONS.md` D245.
    #[test]
    fn a_merge_gives_its_commit() {
        let merged = EventKind::ItemTransitioned {
            item_id: ItemId::from_ulid(Ulid(1)),
            from: Some(ItemState::TestsPassed),
            to: ItemState::Merged,
            reason: Reason::MergeGraphMerge,
            position: None,
            verified_by_run_id: None,
            merged_sha: Some("abc123".into()),
        };
        assert_eq!(
            rows_of(&[logged(merged, None, at(7, 14, 3))]),
            [
                "| 2026-10-07 14:03 | ✅ Merged: the queue merged the PR after its tests passed, as abc123 |  |"
            ]
        );
    }

    /// `DECISIONS.md` D245.
    #[test]
    fn a_test_branch_gives_where_its_ci_runs() {
        let assembled = |draft_pr_number| EventKind::TestBranchAssembled {
            run_id: RunId::from_ulid(Ulid(1)),
            test_sha: "def456".into(),
            draft_pr_number,
            required_statuses: Vec::new(),
            required_statuses_source: RequiredStatusesSource::RepoConfig,
        };
        let cases = [
            (Some(54), vec![], vec![], "the CI runs in #54"),
            (
                Some(54),
                vec![],
                vec![50, 51],
                "the CI runs in #54, on top of #50, #51",
            ),
            (
                Some(54),
                vec![52],
                vec![50],
                "the CI runs in #54, with #52, on top of #50",
            ),
            (None, vec![], vec![], "the CI runs on def456"),
        ];
        for (draft, with, on_top_of, want) in cases {
            let event = HistoryEvent {
                tested_with: with,
                tested_on_top_of: on_top_of,
                ..logged(assembled(draft), None, at(7, 14, 3))
            };
            let want = format!("| 2026-10-07 14:03 | 🧪 Test branch ready: {want} |  |");
            assert_eq!(rows_of(&[event]), [want]);
        }
    }

    #[test]
    fn a_request_or_an_event_of_the_queue_has_no_row() {
        let item_id = ItemId::from_ulid(Ulid(1));
        let kinds = [
            EventKind::ItemRestartRequested {
                item_id,
                by_api: false,
            },
            EventKind::MergeFailed {
                item_id,
                direct: false,
                error: JobError::MergeRejected,
            },
            EventKind::QueueWindowChanged {
                before: 1,
                after: 2,
            },
        ];

        for kind in kinds {
            let event = logged(kind.clone(), Some("nlecoy"), at(7, 14, 3));
            assert_eq!(history_block(&[event]), None, "{kind:?}");
        }

        assert_eq!(history_block(&[]), None);
    }

    #[test]
    fn consecutive_identical_rows_give_one_row() {
        let blocked = || readiness(vec![BlockReason::ChangesRequested]);
        let restarted = || transitioned(ItemState::Pending, Reason::TestsRestartedByUser);
        let history = [
            logged(blocked(), None, at(7, 14, 3)),
            logged(blocked(), None, at(7, 14, 4)),
            logged(readiness(Vec::new()), None, at(7, 14, 5)),
            logged(blocked(), None, at(7, 14, 6)),
            logged(restarted(), Some("alice"), at(7, 14, 7)),
            logged(restarted(), Some("bob"), at(7, 14, 8)),
        ];

        assert_eq!(
            rows_of(&history),
            [
                "| 2026-10-07 14:03 | ⏳ Waiting: a review asks for changes |  |",
                "| 2026-10-07 14:05 | ✅ Nothing blocks the PR |  |",
                "| 2026-10-07 14:06 | ⏳ Waiting: a review asks for changes |  |",
                "| 2026-10-07 14:07 | 🕓 Queued: a user restarted the tests | `@alice` |",
                "| 2026-10-07 14:08 | 🕓 Queued: a user restarted the tests | `@bob` |",
            ]
        );
    }

    #[test]
    fn the_history_shows_the_last_rows_and_counts_the_older_ones() {
        let history = |count: u8| -> Vec<HistoryEvent> {
            (0..count)
                .map(|n| {
                    let kind = transitioned(ItemState::Pending, Reason::TestsRestartedByUser);
                    logged(kind, Some(&format!("user{n}")), at(7, 10, n))
                })
                .collect()
        };
        // The count, the line of the older rows, and the first row shown.
        let cases = [
            (3, None, "user0"),
            (20, None, "user0"),
            (21, Some("1 older row.\n\n"), "user1"),
            (25, Some("5 older rows.\n\n"), "user5"),
        ];
        for (count, older, first) in cases {
            let events = history(count);
            let block = history_block(&events).unwrap();
            let head = format!("<details><summary>History ({count})</summary>\n\n");
            let table = "| Time (UTC) | Event | By |";
            assert!(block.starts_with(&format!("{head}{}{table}", older.unwrap_or(""))));
            assert!(block.ends_with("\n\n</details>"), "{block}");
            let rows = rows_of(&events);
            assert_eq!(rows.len(), usize::from(count).min(HISTORY_ROWS), "{count}");
            assert!(rows[0].ends_with(&format!("| `@{first}` |")), "{rows:?}");
        }
    }

    #[test]
    fn a_pipe_or_a_newline_does_not_break_the_table() {
        let blocked = readiness(vec![
            BlockReason::FailingRequiredCheck {
                check: "build | lint".into(),
            },
            BlockReason::QueueConditionsUnmet {
                conditions: vec!["a\nb".into()],
            },
        ]);
        let rows = rows_of(&[logged(blocked, None, at(7, 14, 3))]);
        assert_eq!(
            rows,
            [
                "| 2026-10-07 14:03 | ⏳ Waiting: the required check `build \\| lint` failed; the queue conditions `a b` are not met |  |"
            ]
        );
    }

    #[test]
    fn the_history_sits_between_the_details_and_the_commands() {
        let history = [logged(
            transitioned(ItemState::NotReady, Reason::Submitted),
            Some("nlecoy"),
            at(7, 14, 3),
        )];
        let rendered = render(
            &surface(ItemState::Testing),
            &settings(|_| {}),
            "main",
            &history,
        );
        let want = "🧪 **Testing** · position 2 in the queue for `main`\n\n\
                    Tested on top of #5 in def456.\n\n\
                    <details><summary>History (1)</summary>\n\n\
                    | Time (UTC) | Event | By |\n| --- | --- | --- |\n\
                    | 2026-10-07 14:03 | ⏳ Waiting: the PR was submitted to the queue | `@nlecoy` |\n\n\
                    </details>\n\n\
                    <sub>`/cihub restart` to test again · `/cihub cancel` to leave the queue</sub>\n";
        assert_eq!(rendered.comment.as_deref(), Some(want));
        let bare = render(&surface(ItemState::Testing), &settings(|_| {}), "main", &[]);

        assert_ne!(hash(&rendered), hash(&bare));
    }

    #[test]
    fn without_comments_the_history_shows_nowhere() {
        let off = settings(|l| l.comments_enabled = Some(Setting::new(false)));
        let history = [logged(
            transitioned(ItemState::NotReady, Reason::Submitted),
            Some("nlecoy"),
            at(7, 14, 3),
        )];
        let rendered = render(&surface(ItemState::Testing), &off, "main", &history);

        assert_eq!(rendered.comment, None);

        let status = rendered.status.unwrap().description;
        assert_eq!(status, "Testing, position 2", "the status does not change");
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

/// The comment that welcomes a PR on a branch of a queue, with its
/// submission box.
fn welcome(target_branch: &str) -> String {
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

    let Some(target) = queues.into_iter().next() else {
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

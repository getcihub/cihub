//! The words of what a PR shows: the rendering of a surface into a status
//! description, a Markdown comment and state labels, and the sentences that
//! the comment and the dashboard share. It does no I/O and publishes
//! nothing: `surfaces` writes and publishes what it renders.

mod history;

use std::fmt::Write as _;

use cihub_config::QueueSettings;
use cihub_domain::{AwaitedCheckStatus, BlockReason, ItemState, Reason};
use cihub_mq_core::{BatchWait, Surface};
use cihub_scm::{CommitStatus, SUBMIT_BOX};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::UtcOffset;

pub(crate) use history::HistoryEvent;
use history::history_block;

/// The labels that say the state of an item (`specs/GITHUB.md`).
const STATE_LABELS: [&str; 3] = ["cihub:queued", "cihub:testing", "cihub:failed"];

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
    /// The page of the PR in the dashboard.
    pub details_url: Option<String>,
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

/// Renders `surface` with the settings of its queue, the logged `history` of
/// its PR, and `page`, the page of the PR in the dashboard.
pub(crate) fn render(
    surface: &Surface,
    settings: &QueueSettings,
    target_branch: &str,
    history: &[HistoryEvent],
    page: Option<&str>,
) -> Rendered {
    let status = settings.status_check_enabled.value.then(|| RenderedStatus {
        state: status_state(surface.state),
        description: truncate(&description(surface)),
        details_url: page.map(str::to_owned),
    });
    let comment = settings
        .comments_enabled
        .value
        .then(|| comment(surface, settings, target_branch, history, page));
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
///
/// # Errors
///
/// The rendering does not serialize to JSON.
pub(crate) fn hash(rendered: &impl Serialize) -> Result<String, serde_json::Error> {
    let bytes = serde_json::to_vec(rendered)?;
    let digest = Sha256::digest(bytes);

    Ok(digest.iter().take(8).fold(String::new(), |mut hex, byte| {
        let _ = write!(hex, "{byte:02x}");
        hex
    }))
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

pub(crate) fn truncate(text: &str) -> String {
    if text.chars().count() <= DESCRIPTION_LIMIT {
        return text.to_owned();
    }

    let mut short: String = text.chars().take(DESCRIPTION_LIMIT - 1).collect();
    short.push('…');

    short
}

pub(crate) fn block_text(reason: &BlockReason) -> String {
    match reason {
        BlockReason::ReviewRequired { have, need } => {
            format!("{have} of {need} required approvals")
        }
        BlockReason::ChangesRequested => "a review asks for changes".to_owned(),
        BlockReason::FailingRequiredCheck { check } => {
            format!("the required check `{check}` failed")
        }
        BlockReason::AwaitingRequiredCheck { check, status } => match status {
            AwaitedCheckStatus::Missing => format!("the required check `{check}` has not reported"),
            AwaitedCheckStatus::Queued => format!("the required check `{check}` is queued"),
            AwaitedCheckStatus::Running => format!("the required check `{check}` is running"),
        },
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

/// The comment of the queue on the PR, in Markdown: the state line, its
/// details, the history, then the commands in small print.
fn comment(
    surface: &Surface,
    settings: &QueueSettings,
    target_branch: &str,
    history: &[HistoryEvent],
    page: Option<&str>,
) -> String {
    let commands = settings.commands_enabled.value;
    let mut blocks = vec![state_line(surface, target_branch)];

    if let Some(wait) = &surface.batch_wait {
        blocks.push(batch_wait_line(wait));
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

    if let Some(profile) = &surface.profile {
        details.push(format!("Profile `{profile}` of `.cihub.yaml`."));
    }

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

    if let Some(page) = page {
        blocks.push(format!("[Open in CIHub]({page})"));
    }

    blocks.extend(history_block(history));

    // Only an item in test can restart.
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

/// The reason of a transition, and the commit of a merge: the sentence of
/// the history of the comment and of the dashboard.
pub(crate) fn transition_sentence(reason: Reason, merged_sha: Option<&str>) -> String {
    match merged_sha {
        Some(sha) => format!("{}, as {sha}", reason.describe()),
        None => reason.describe().to_owned(),
    }
}

/// What blocks a PR, for the comment and the dashboard; `None` when nothing
/// does. A check or a condition is a Markdown code span.
pub(crate) fn blocks_sentence(block_reasons: &[BlockReason]) -> Option<String> {
    if block_reasons.is_empty() {
        return None;
    }

    let reasons: Vec<String> = block_reasons.iter().map(block_text).collect();

    Some(reasons.join("; "))
}

/// When the batch stops waiting for more PRs, never when its test starts: a
/// closed batch still waits for room in the window. An absolute time does
/// not go stale between two edits of the comment; the seconds are dropped,
/// so the batch closes during the minute shown.
fn batch_wait_line(wait: &BatchWait) -> String {
    let prs = if wait.members == 1 { "PR" } else { "PRs" };
    let at = wait.closes_at.to_offset(UtcOffset::UTC);
    format!(
        "Batch of {} {prs}: it waits for more PRs until {:02}:{:02} UTC, or until it has {}.",
        wait.members,
        at.hour(),
        at.minute(),
        wait.size
    )
}

pub(crate) fn capitalized(text: &str) -> String {
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

/// `text` as one segment of a path: every byte but the unreserved ones of
/// RFC 3986 is percent-encoded, as a branch can hold `/`.
pub(crate) fn path_segment(text: &str) -> String {
    text.bytes().fold(String::new(), |mut segment, byte| {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            segment.push(char::from(byte));
        } else {
            let _ = write!(segment, "%{byte:02X}");
        }
        segment
    })
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use cihub_config::{Layer, Setting};
    use cihub_domain::Reason;

    use super::*;

    pub(super) fn surface(state: ItemState) -> Surface {
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
            profile: None,
            batch_wait: None,
        }
    }

    pub(super) fn settings(edit: impl FnOnce(&mut Layer)) -> QueueSettings {
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
            let rendered = render(&surface(state), &defaults, "main", &[], None);
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
                status: AwaitedCheckStatus::Running,
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
                 - The required check `build` is running\n\n\
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
            let comment = render(&surface, &defaults, "main", &[], None)
                .comment
                .unwrap();
            assert_eq!(comment, want, "{:?}", surface.state);
        }
    }

    #[test]
    fn the_comment_names_the_profile_of_the_pr() {
        for state in [ItemState::Pending, ItemState::Merged] {
            let mut item = surface(state);
            item.profile = Some("dependencies".into());

            let comment = render(&item, &settings(|_| {}), "main", &[], None)
                .comment
                .unwrap();

            assert!(
                comment.contains("Profile `dependencies` of `.cihub.yaml`."),
                "{state:?}: {comment}"
            );
        }

        let comment = render(
            &surface(ItemState::Pending),
            &settings(|_| {}),
            "main",
            &[],
            None,
        )
        .comment
        .unwrap();
        assert!(!comment.contains("Profile"), "{comment}");
    }

    #[test]
    fn a_waiting_batch_says_when_it_closes() {
        let closes_at = time::Date::from_calendar_date(2026, time::Month::October, 8)
            .unwrap()
            .with_hms(12, 40, 59)
            .unwrap()
            .assume_offset(UtcOffset::from_hms(2, 0, 0).unwrap());
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
            queued.batch_wait = Some(BatchWait {
                members,
                size: 3,
                closes_at,
            });
            let rendered = render(&queued, &settings(|_| {}), "main", &[], None);
            assert_eq!(rendered.comment.unwrap(), want, "{members}");
            let status = rendered.status.unwrap().description;
            assert_eq!(status, "Queued, position 1", "the status does not change");
        }
    }

    #[test]
    fn without_commands_the_comment_names_none() {
        let off = settings(|l| l.commands_enabled = Some(Setting::new(false)));
        for state in [ItemState::Testing, ItemState::Failed] {
            let comment = render(&surface(state), &off, "main", &[], None)
                .comment
                .unwrap();
            assert!(!comment.contains("/cihub"), "{comment}");
            assert!(!comment.contains(SUBMIT_BOX), "{comment}");
        }
    }

    #[test]
    fn a_failure_shows_the_sentence_of_its_reason() {
        let mut failed = surface(ItemState::Failed);
        failed.reason = Reason::PrPushedTo;

        let rendered = render(&failed, &settings(|_| {}), "main", &[], None);
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
        let rendered = render(&surface(ItemState::Pending), &off, "main", &[], None);
        assert_eq!((rendered.status, rendered.comment), (None, None));
    }

    #[test]
    fn state_labels_follow_the_state_of_a_pr_that_carries_one() {
        let off = settings(|_| {});
        let on = settings(|l| l.state_labels_enabled = Some(Setting::new(true)));
        let mut testing = surface(ItemState::Testing);
        let labels = |rendered: Rendered| (rendered.add_labels, rendered.remove_labels);

        assert_eq!(
            labels(render(&testing, &off, "main", &[], None)),
            (vec![], vec![])
        );

        let others = |labels: &[&str]| labels.iter().map(|l| (*l).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            labels(render(&testing, &on, "main", &[], None)),
            (
                others(&["cihub:testing"]),
                others(&["cihub:queued", "cihub:failed"])
            )
        );

        testing.labels = BTreeSet::from(["cihub:queued".to_owned(), "bug".to_owned()]);

        assert_eq!(
            labels(render(&testing, &off, "main", &[], None)),
            (
                others(&["cihub:testing"]),
                others(&["cihub:queued", "cihub:failed"])
            )
        );

        let mut merged = surface(ItemState::Merged);
        merged.labels = BTreeSet::from(["cihub:testing".to_owned()]);

        assert_eq!(
            labels(render(&merged, &off, "main", &[], None)),
            (vec![], others(&STATE_LABELS))
        );
    }

    #[test]
    fn a_long_description_is_cut_and_a_change_gives_a_new_hash() {
        let mut blocked = surface(ItemState::NotReady);
        blocked.block_reasons = vec![BlockReason::QueueConditionsUnmet {
            conditions: vec!["x".repeat(200)],
        }];

        let rendered = render(&blocked, &settings(|_| {}), "main", &[], None);
        let description = rendered.status.clone().unwrap().description;

        assert_eq!(description.chars().count(), DESCRIPTION_LIMIT);

        let other = render(
            &surface(ItemState::Pending),
            &settings(|_| {}),
            "main",
            &[],
            None,
        );
        assert_ne!(hash(&rendered).unwrap(), hash(&other).unwrap());
        assert_eq!(hash(&rendered).unwrap(), hash(&rendered.clone()).unwrap());
    }

    #[test]
    fn the_status_and_the_comment_link_to_the_page_of_the_pr() {
        let page = "https://cihub.example.com/orgs/acme/repos/app/queues/main/prs/7";
        let rendered = render(
            &surface(ItemState::Testing),
            &settings(|_| {}),
            "main",
            &[],
            Some(page),
        );

        assert_eq!(rendered.status.unwrap().details_url.as_deref(), Some(page));
        assert!(
            rendered
                .comment
                .unwrap()
                .contains(&format!("[Open in CIHub]({page})"))
        );
    }

    #[test]
    fn each_block_reason_gives_its_sentence() {
        let cases = [
            (
                BlockReason::ReviewRequired { have: 1, need: 2 },
                "1 of 2 required approvals",
            ),
            (BlockReason::ChangesRequested, "a review asks for changes"),
            (
                BlockReason::FailingRequiredCheck {
                    check: "build".into(),
                },
                "the required check `build` failed",
            ),
            (
                BlockReason::AwaitingRequiredCheck {
                    check: "build".into(),
                    status: AwaitedCheckStatus::Missing,
                },
                "the required check `build` has not reported",
            ),
            (
                BlockReason::AwaitingRequiredCheck {
                    check: "build".into(),
                    status: AwaitedCheckStatus::Queued,
                },
                "the required check `build` is queued",
            ),
            (
                BlockReason::AwaitingRequiredCheck {
                    check: "build".into(),
                    status: AwaitedCheckStatus::Running,
                },
                "the required check `build` is running",
            ),
            (BlockReason::BehindBase, "the branch is behind its base"),
            (
                BlockReason::MergeConflict,
                "the branch conflicts with its base",
            ),
            (
                BlockReason::StackNeedsRebase { onto_pr_number: 4 },
                "the branch must be rebased onto #4",
            ),
            (
                BlockReason::Undetermined,
                "the SCM does not say yet why the PR is blocked",
            ),
            (
                BlockReason::QueueConditionsUnmet {
                    conditions: vec!["a".into(), "b".into()],
                },
                "the queue conditions `a`, `b` are not met",
            ),
        ];

        for (reason, want) in cases {
            assert_eq!(block_text(&reason), want, "{reason:?}");
        }
    }

    #[test]
    fn the_blocks_are_one_sentence_or_none() {
        assert_eq!(blocks_sentence(&[]), None);
        assert_eq!(
            blocks_sentence(&[BlockReason::ChangesRequested, BlockReason::BehindBase]).as_deref(),
            Some("a review asks for changes; the branch is behind its base")
        );
    }

    #[test]
    fn a_transition_gives_its_reason_and_the_commit_of_a_merge() {
        assert_eq!(
            transition_sentence(Reason::TestRunRequiredStatusFailed, None),
            "a required check failed"
        );
        assert_eq!(
            transition_sentence(Reason::MergeGraphMerge, Some("0123abc")),
            "the queue merged the PR after its tests passed, as 0123abc"
        );
    }

    #[test]
    fn each_state_gives_its_status_description() {
        let mut blocked = surface(ItemState::NotReady);
        blocked.block_reasons = vec![BlockReason::BehindBase];

        let mut failed = surface(ItemState::Failed);
        failed.reason = Reason::TestRunTimeout;

        let mut cancelled = surface(ItemState::Cancelled);
        cancelled.reason = Reason::UserRequested;

        let mut unplaced = surface(ItemState::Pending);
        unplaced.position = None;

        let cases = [
            (blocked, "Waiting: the branch is behind its base"),
            (surface(ItemState::NotReady), "Waiting to enter the queue"),
            (surface(ItemState::Pending), "Queued, position 2"),
            (unplaced, "Queued"),
            (surface(ItemState::Testing), "Testing, position 2"),
            (
                surface(ItemState::PendingFailure),
                "Tests failed, waiting for the PRs ahead, position 2",
            ),
            (surface(ItemState::TestsPassed), "Tests passed, merging"),
            (surface(ItemState::Merged), "Merged by the queue"),
            (failed, "Failed: the tests took longer than the time limit"),
            (cancelled, "Cancelled: a user asked for it"),
        ];

        for (surface, want) in cases {
            assert_eq!(description(&surface), want, "{:?}", surface.state);
        }
    }

    #[test]
    fn the_welcome_comment_names_the_branch_and_the_box() {
        assert_eq!(
            welcome("main"),
            "CIHub manages merges into `main`.\n\n\
             - [ ] <!-- cihub:submit --> Add this PR to the merge queue\n\n\
             <sub>Or comment `/cihub merge`.</sub>\n"
        );
    }

    #[test]
    fn a_name_of_the_file_stays_one_segment_of_the_page() {
        assert_eq!(path_segment("main"), "main");
        assert_eq!(path_segment("release/2.0 rc"), "release%2F2.0%20rc");
    }
}

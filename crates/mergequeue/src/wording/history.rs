//! The history of a PR in its comment: a folded table with one row per
//! decision, read from the log. Pure: `surfaces` reads the events.

use std::fmt::Write as _;

use cihub_domain::{BlockReason, EventKind};
use time::{OffsetDateTime, UtcOffset};

use super::{blocks_sentence, state_mark, transition_sentence};

/// The comment shows the last rows of the history, then counts the others.
const HISTORY_ROWS: usize = 20;

/// An event of the log of a PR, as its history reads it.
#[derive(Debug)]
pub(crate) struct HistoryEvent {
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

/// The history of a PR as a folded table, `None` without a row. Two
/// consecutive events with the same text and the same user are one row; the
/// comment shows the last rows and counts the older ones.
pub(super) fn history_block(events: &[HistoryEvent]) -> Option<String> {
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
            let (icon, label) = state_mark(*to);
            // The SCM links a full SHA.
            let sentence = transition_sentence(*reason, merged_sha.as_deref());
            Some(format!("{icon} {label}: {sentence}"))
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

/// A mention notifies its user: the code span names the user without it.
fn by_cell(login: Option<&str>) -> String {
    login.map_or_else(String::new, |login| format!("`@{login}`"))
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
    match blocks_sentence(block_reasons) {
        None => "✅ Nothing blocks the PR".to_owned(),
        Some(blocks) => format!("⏳ Waiting: {blocks}"),
    }
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

#[cfg(test)]
mod tests {
    use cihub_config::Setting;
    use cihub_domain::{
        ItemId, ItemState, JobError, Mergeability, Reason, RequiredStatusesSource, RunId,
    };
    use ulid::Ulid;

    use super::super::tests::{settings, surface};
    use super::super::{hash, render};
    use super::*;
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
            None,
        );
        let want = "🧪 **Testing** · position 2 in the queue for `main`\n\n\
                    Tested on top of #5 in def456.\n\n\
                    <details><summary>History (1)</summary>\n\n\
                    | Time (UTC) | Event | By |\n| --- | --- | --- |\n\
                    | 2026-10-07 14:03 | ⏳ Waiting: the PR was submitted to the queue | `@nlecoy` |\n\n\
                    </details>\n\n\
                    <sub>`/cihub restart` to test again · `/cihub cancel` to leave the queue</sub>\n";
        assert_eq!(rendered.comment.as_deref(), Some(want));
        let bare = render(
            &surface(ItemState::Testing),
            &settings(|_| {}),
            "main",
            &[],
            None,
        );

        assert_ne!(hash(&rendered).unwrap(), hash(&bare).unwrap());
    }

    #[test]
    fn without_comments_the_history_shows_nowhere() {
        let off = settings(|l| l.comments_enabled = Some(Setting::new(false)));
        let history = [logged(
            transitioned(ItemState::NotReady, Reason::Submitted),
            Some("nlecoy"),
            at(7, 14, 3),
        )];
        let rendered = render(&surface(ItemState::Testing), &off, "main", &history, None);

        assert_eq!(rendered.comment, None);

        let status = rendered.status.unwrap().description;
        assert_eq!(status, "Testing, position 2", "the status does not change");
    }
}

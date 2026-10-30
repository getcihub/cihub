use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{AwaitedCheckStatus, GroupId, ItemId, Mergeability, OrgId, QueueId, Reason, RunId};

/// One submission of a PR to a queue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent flags of a submission, not the states of a machine"
)]
pub struct Item {
    /// The item; a new submission of the same PR is a new item.
    pub id: ItemId,
    /// The organization of the queue.
    pub org_id: OrgId,
    /// The queue of the base branch of the PR.
    pub queue_id: QueueId,
    /// The number of the PR on its repository.
    pub pr_number: u64,
    /// Head SHA of the PR at submission time.
    pub head_sha: String,
    /// Where the item is in its life.
    pub state: ItemState,
    /// Reason of the last transition.
    pub state_reason: Reason,
    /// The order of service: a lower value is served first.
    pub priority: Priority,
    /// The submission asks that the PR be tested alone, never in a batch.
    pub no_batch: bool,
    /// The profile of the queue that gives the settings of the item; `None`
    /// takes those of the queue.
    pub profile: Option<String>,
    /// How the PR was submitted.
    pub submitted_via: SubmittedVia,
    /// Place in the queue; `None` until admission.
    pub position: Option<i64>,
    /// True for the live item of this PR; at most one per PR and queue.
    pub is_current: bool,
    /// Whether the SCM accepts to merge the PR, at its last read.
    pub mergeability: Mergeability,
    /// Why a `not_ready` item cannot enter the queue yet.
    pub block_reasons: Vec<BlockReason>,
    /// The group that tests the item.
    pub group_id: Option<GroupId>,
    /// The run that proved the item: its own or a successor's.
    pub verified_by_run_id: Option<RunId>,
    /// The commit that the merge of the PR put on the target branch; `None`
    /// until the merge.
    pub merged_sha: Option<String>,
    /// When the submission came: the time of the creation of the item.
    #[serde(with = "time::serde::rfc3339")]
    pub submitted_at: OffsetDateTime,
    /// When the item first entered the queue (`pending`): the start of its
    /// wait for a batch.
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub admitted_at: Option<OffsetDateTime>,
    /// A direct merge was tried and could not happen: the item follows the
    /// path of the tests.
    pub direct_merge_tried: bool,
}

closed_enum! {
    /// Public state of a queue item.
    pub enum ItemState {
        /// Submitted, but the PR cannot enter the queue yet: see its
        /// [`BlockReason`]s.
        NotReady,
        /// In the queue, waiting for a group and a test.
        Pending,
        /// A run of its group tests it.
        Testing,
        /// A run proved it; it waits for its predecessors to merge.
        TestsPassed,
        /// A run of its group failed; the queue waits to know who is at
        /// fault.
        PendingFailure,
        /// The PR merged.
        Merged,
        /// The PR is at fault, or its merge failed; it left the queue.
        Failed,
        /// The item left the queue before its end: a person asked for it, or
        /// the PR changed.
        Cancelled,
    }
}

impl ItemState {
    /// Whether the item can change state no more.
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Merged | Self::Failed | Self::Cancelled)
    }

    /// Whether a person can ask for the cancellation of the item.
    pub const fn can_cancel(self) -> bool {
        !self.is_terminal()
    }

    /// Whether a person can ask for a new test of the item: only an item in
    /// test has one to restart.
    pub const fn can_restart(self) -> bool {
        matches!(self, Self::Testing | Self::PendingFailure)
    }
}

closed_enum! {
    /// How a PR was submitted to its queue.
    pub enum SubmittedVia {
        /// A command or the box of a comment on the PR.
        Comment,
        /// The entry label of the queue on the PR.
        Label,
        /// The API, or the command line through the API.
        Api,
    }
}

/// Lower value is served first; the named levels are shortcuts for values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Priority(pub u8);

impl Priority {
    /// `urgent`: goes in front of the groups in test.
    pub const URGENT: Self = Self(0);
    /// `high`.
    pub const HIGH: Self = Self(10);
    /// `medium`, the default.
    pub const MEDIUM: Self = Self(100);
    /// `low`.
    pub const LOW: Self = Self(200);

    const NAMES: [(&'static str, Self); 4] = [
        ("urgent", Self::URGENT),
        ("high", Self::HIGH),
        ("medium", Self::MEDIUM),
        ("low", Self::LOW),
    ];

    /// The value of the named level `name`; `None` for another name.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::NAMES
            .iter()
            .find(|(n, _)| *n == name)
            .map(|&(_, p)| p)
    }
}

/// Why a `not_ready` item cannot enter the queue yet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BlockReason {
    /// The PR lacks approvals.
    ReviewRequired {
        /// The approvals that the PR has.
        have: u32,
        /// The approvals that the rules require.
        need: u32,
    },
    /// A reviewer asked for changes.
    ChangesRequested,
    /// A check that the rules require failed on the head of the PR.
    FailingRequiredCheck {
        /// The name of the check.
        check: String,
    },
    /// A check that the rules require has not concluded on the head of the
    /// PR.
    AwaitingRequiredCheck {
        /// The name of the check.
        check: String,
        /// Where the check stands.
        status: AwaitedCheckStatus,
    },
    /// The rules require the PR to be up to date with its base, and it is
    /// not.
    BehindBase,
    /// The PR conflicts with its base.
    MergeConflict,
    /// The PR is stacked on another PR, and must be rebased onto it.
    StackNeedsRebase {
        /// The PR to rebase onto.
        onto_pr_number: u64,
    },
    /// The SCM does not say yet why the PR is blocked: its mergeability is
    /// unknown, or no required check explains it.
    Undetermined,
    /// The PR does not meet these `queue_conditions` of the queue, as the
    /// file writes them.
    QueueConditionsUnmet {
        /// The conditions that are not met.
        conditions: Vec<String>,
    },
}

impl BlockReason {
    /// The sentence that a person reads for this reason: the PR comment and
    /// status, the API and the CLI. In English, without a capital or a final
    /// period; a check or a condition is a Markdown code span.
    pub fn describe(&self) -> String {
        match self {
            Self::ReviewRequired { have, need } => {
                format!("{have} of {need} required approvals")
            }
            Self::ChangesRequested => "a review asks for changes".to_owned(),
            Self::FailingRequiredCheck { check } => {
                format!("the required check `{check}` failed")
            }
            Self::AwaitingRequiredCheck { check, status } => match status {
                AwaitedCheckStatus::Missing => {
                    format!("the required check `{check}` has not reported")
                }
                AwaitedCheckStatus::Queued => format!("the required check `{check}` is queued"),
                AwaitedCheckStatus::Running => format!("the required check `{check}` is running"),
            },
            Self::BehindBase => "the branch is behind its base".to_owned(),
            Self::MergeConflict => "the branch conflicts with its base".to_owned(),
            Self::StackNeedsRebase { onto_pr_number } => {
                format!("the branch must be rebased onto #{onto_pr_number}")
            }
            Self::Undetermined => "the SCM does not say yet why the PR is blocked".to_owned(),
            Self::QueueConditionsUnmet { conditions } => {
                let list: Vec<String> = conditions.iter().map(|c| format!("`{c}`")).collect();
                format!("the queue conditions {} are not met", list.join(", "))
            }
        }
    }
}

/// Targets that a PR changes, which place it in the merge graph. `All` in
/// single mode, for a PR that uploaded `ALL`, and past the 20 MB upload limit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImpactedTargets {
    /// The PR can change everything: it waits for every PR ahead of it.
    All,
    /// The targets that the PR changes, as the CI names them.
    Targets(BTreeSet<String>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_names_give_their_value() {
        assert_eq!(Priority::from_name("urgent"), Some(Priority::URGENT));
        assert_eq!(Priority::from_name("high"), Some(Priority::HIGH));
        assert_eq!(Priority::from_name("medium"), Some(Priority::MEDIUM));
        assert_eq!(Priority::from_name("low"), Some(Priority::LOW));
        assert_eq!(Priority::from_name("later"), None);
    }

    #[test]
    fn block_reason_is_tagged_by_kind() {
        let json =
            serde_json::to_string(&BlockReason::ReviewRequired { have: 1, need: 2 }).unwrap();
        assert_eq!(json, r#"{"kind":"review_required","have":1,"need":2}"#);
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
            assert_eq!(reason.describe(), want, "{reason:?}");
        }
    }

    #[test]
    fn impacted_targets_serialize_all_or_a_set() {
        assert_eq!(
            serde_json::to_string(&ImpactedTargets::All).unwrap(),
            r#""all""#
        );
        let targets = ImpactedTargets::Targets(["//b".into(), "//a".into()].into());
        assert_eq!(
            serde_json::to_string(&targets).unwrap(),
            r#"{"targets":["//a","//b"]}"#
        );
    }
}

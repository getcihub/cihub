use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{
    CheckReport, CheckStatus, GroupId, ItemId, ItemState, Mergeability, OrgId, QueueId, Reason,
    RunId, SubmittedVia,
};

/// One submission of a PR to a queue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent flags of a submission, not the states of a machine"
)]
pub struct Item {
    pub id: ItemId,
    pub org_id: OrgId,
    pub queue_id: QueueId,
    pub pr_number: u64,
    /// Head SHA of the PR at submission time.
    pub head_sha: String,
    pub state: ItemState,
    /// Reason of the last transition.
    pub state_reason: Reason,
    pub priority: Priority,
    pub no_batch: bool,
    /// The profile of the queue that gives the settings of the item; `None`
    /// takes those of the queue.
    pub profile: Option<String>,
    pub submitted_via: SubmittedVia,
    /// Place in the queue; `None` until admission.
    pub position: Option<i64>,
    /// True for the live item of this PR; at most one per PR and queue.
    pub is_current: bool,
    pub mergeability: Mergeability,
    /// Why a `not_ready` item cannot enter the queue yet.
    pub block_reasons: Vec<BlockReason>,
    /// The group that tests the item.
    pub group_id: Option<GroupId>,
    /// The run that proved the item: its own or a successor's.
    pub verified_by_run_id: Option<RunId>,
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

/// Lower value is served first; the named levels are shortcuts for values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Priority(pub u8);

impl Priority {
    pub const URGENT: Self = Self(0);
    pub const HIGH: Self = Self(10);
    pub const MEDIUM: Self = Self(100);
    pub const LOW: Self = Self(200);

    const NAMES: [(&'static str, Self); 4] = [
        ("urgent", Self::URGENT),
        ("high", Self::HIGH),
        ("medium", Self::MEDIUM),
        ("low", Self::LOW),
    ];

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
    ReviewRequired {
        have: u32,
        need: u32,
    },
    ChangesRequested,
    FailingRequiredCheck {
        check: String,
    },
    AwaitingRequiredCheck {
        check: String,
        status: AwaitedCheckStatus,
    },
    BehindBase,
    MergeConflict,
    StackNeedsRebase {
        onto_pr_number: u64,
    },
    Undetermined,
    /// The PR does not meet these `queue_conditions` of the queue, as the
    /// file writes them.
    QueueConditionsUnmet {
        conditions: Vec<String>,
    },
}

closed_enum! {
    /// Where a required check that has not concluded stands.
    pub enum AwaitedCheckStatus {
        /// The check has not reported on the head of the PR.
        Missing,
        Queued,
        Running,
    }
}

impl AwaitedCheckStatus {
    /// The status of a required check without a conclusion, from its report
    /// on the head of the PR, if any.
    pub fn of(report: Option<&CheckReport>) -> Self {
        match report.map(|report| report.status) {
            None => Self::Missing,
            Some(CheckStatus::Queued) => Self::Queued,
            Some(CheckStatus::InProgress | CheckStatus::Completed) => Self::Running,
        }
    }
}

/// Targets that a PR changes, which place it in the merge graph. `All` in
/// single mode, for a PR that uploaded `ALL`, and past the 20 MB upload limit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImpactedTargets {
    All,
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
    fn an_awaited_check_is_missing_queued_or_running() {
        let report = |status| CheckReport {
            name: "ci".into(),
            kind: crate::CheckKind::Check,
            status,
            conclusion: None,
            details_url: None,
            started_at: None,
            completed_at: None,
        };

        let cases = [
            (None, AwaitedCheckStatus::Missing),
            (
                Some(report(CheckStatus::Queued)),
                AwaitedCheckStatus::Queued,
            ),
            (
                Some(report(CheckStatus::InProgress)),
                AwaitedCheckStatus::Running,
            ),
        ];

        for (report, want) in cases {
            assert_eq!(AwaitedCheckStatus::of(report.as_ref()), want);
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

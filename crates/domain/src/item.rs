use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{
    GroupId, ItemId, ItemState, Mergeability, OrgId, QueueId, Reason, RunId, SubmittedVia,
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
    /// Submitted with `--direct`: merged by `cihub-sudo`.
    pub force_enqueued: bool,
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
    /// path of the tests (`DECISIONS.md` D153).
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

    pub fn name(self) -> Option<&'static str> {
        Self::NAMES
            .iter()
            .find(|(_, p)| *p == self)
            .map(|&(n, _)| n)
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
        status: String,
    },
    BehindBase,
    MergeConflict,
    StackNeedsRebase {
        onto_pr_number: u64,
    },
    Undetermined,
    /// The PR does not meet these `queue_conditions` of the queue, as the
    /// file writes them (`DECISIONS.md` D142).
    QueueConditionsUnmet {
        conditions: Vec<String>,
    },
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
    fn priority_names_round_trip() {
        for name in ["urgent", "high", "medium", "low"] {
            assert_eq!(
                Priority::from_name(name).and_then(Priority::name),
                Some(name)
            );
        }
        assert_eq!(Priority(42).name(), None);
    }

    #[test]
    fn block_reason_is_tagged_by_kind() {
        let json =
            serde_json::to_string(&BlockReason::ReviewRequired { have: 1, need: 2 }).unwrap();
        assert_eq!(json, r#"{"kind":"review_required","have":1,"need":2}"#);
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

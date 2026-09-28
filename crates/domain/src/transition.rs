use serde::{Deserialize, Serialize};

use crate::{
    CancelledReason, Event, EventKind, FailedReason, Item, ItemId, ItemState, MergedReason,
    PendingReason, QueueId, TestsPassedReason,
};

/// Target state of an item transition, with its reason. The spec lists no
/// reason for `testing` and `pending_failure`; `not_ready` is only an initial
/// state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "state", content = "reason", rename_all = "snake_case")]
pub enum Target {
    Pending(PendingReason),
    Testing,
    TestsPassed(TestsPassedReason),
    PendingFailure,
    Merged(MergedReason),
    Failed(FailedReason),
    Cancelled(CancelledReason),
}

impl Target {
    pub const fn state(self) -> ItemState {
        match self {
            Self::Pending(_) => ItemState::Pending,
            Self::Testing => ItemState::Testing,
            Self::TestsPassed(_) => ItemState::TestsPassed,
            Self::PendingFailure => ItemState::PendingFailure,
            Self::Merged(_) => ItemState::Merged,
            Self::Failed(_) => ItemState::Failed,
            Self::Cancelled(_) => ItemState::Cancelled,
        }
    }

    pub fn reason(self) -> Option<&'static str> {
        match self {
            Self::Pending(r) => Some(r.as_str()),
            Self::TestsPassed(r) => Some(r.as_str()),
            Self::Merged(r) => Some(r.as_str()),
            Self::Failed(r) => Some(r.as_str()),
            Self::Cancelled(r) => Some(r.as_str()),
            Self::Testing | Self::PendingFailure => None,
        }
    }
}

/// A validated state change. The store applies it with a guard on `from`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transition {
    pub item_id: ItemId,
    pub from: ItemState,
    pub to: Target,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransitionError {
    /// The event is not about this item.
    ItemMismatch {
        item_id: ItemId,
        item_queue_id: QueueId,
        event_item_id: Option<ItemId>,
        event_queue_id: QueueId,
    },
    NotAllowed {
        from: ItemState,
        to: Target,
    },
}

impl std::fmt::Display for TransitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ItemMismatch {
                item_id,
                event_item_id,
                ..
            } => write!(
                f,
                "event for item {event_item_id:?} applied to item {item_id}"
            ),
            Self::NotAllowed { from, to } => {
                write!(f, "transition {from} -> {}", to.state())?;
                if let Some(reason) = to.reason() {
                    write!(f, " ({reason})")?;
                }
                f.write_str(" is not allowed")
            }
        }
    }
}

impl std::error::Error for TransitionError {}

/// Validates the item state change that `event` asks for.
///
/// # Errors
///
/// `ItemMismatch` if the event is for another item or queue; `NotAllowed` if
/// the state machine has no such edge from the current state.
pub fn transition(item: &Item, event: &Event) -> Result<Transition, TransitionError> {
    if event.item_id != Some(item.id) || event.queue_id != item.queue_id {
        return Err(TransitionError::ItemMismatch {
            item_id: item.id,
            item_queue_id: item.queue_id,
            event_item_id: event.item_id,
            event_queue_id: event.queue_id,
        });
    }
    let EventKind::ItemStateChanged { to } = event.kind;
    if !is_allowed(item.state, to) {
        return Err(TransitionError::NotAllowed {
            from: item.state,
            to,
        });
    }
    Ok(Transition {
        item_id: item.id,
        from: item.state,
        to,
    })
}

fn is_allowed(from: ItemState, to: Target) -> bool {
    use ItemState as S;
    use MergedReason as M;
    use TestsPassedReason as P;
    match (from, to) {
        _ if from.is_terminal() => false,
        (_, Target::Failed(reason)) => failed_from(reason) == from,
        // Any non-terminal state: cancellation, or a merge made outside the queue.
        (_, Target::Cancelled(_) | Target::Merged(M::ManualMerge))
        | (S::NotReady, Target::Pending(PendingReason::MergeItemNowReady))
        | (S::Pending, Target::Testing | Target::Merged(M::MergeGraphDirectMerge))
        | (
            S::Testing,
            Target::PendingFailure
            | Target::TestsPassed(P::TestRunPassed | P::PassedByOptimization),
        )
        | (S::PendingFailure, Target::TestsPassed(P::SuccessorTestRunPassed))
        | (S::TestsPassed, Target::Merged(M::MergeGraphMerge))
        | (S::Testing | S::PendingFailure | S::TestsPassed, Target::Pending(_)) => true,
        _ => false,
    }
}

/// The only state that a failure reason can leave.
const fn failed_from(reason: FailedReason) -> ItemState {
    use FailedReason as F;
    match reason {
        F::StartTestRunInvalidEvent
        | F::StartTestRunMergeConflict
        | F::StartTestRunConfigNotFound
        | F::StartTestRunConfigParsingFailure
        | F::StartTestRunConfigBadVersion
        | F::StartTestRunConfigMissingRequiredStatuses
        | F::StartTestRunConfigBadRequiredStatuses
        | F::StartTestRunNoRequiredStatuses
        | F::StartTestRunRetryExhausted
        | F::StartTestRunDraftPrsNotAllowed
        | F::StartTestRunPermissionsBlockTestBranch => ItemState::Testing,
        F::TestRunRequiredStatusFailed | F::TestRunTimeout | F::TestRunFailedByOptimization => {
            ItemState::PendingFailure
        }
        F::GithubApiDirectMergePrFailed => ItemState::Pending,
        F::GithubApiMergePrFailed
        | F::GithubApiRateLimitExceeded
        | F::GithubApiBranchProtectionUnmet
        | F::GithubApiMergeAlreadyInProgress
        | F::GithubApiRepeatedServerErrors
        | F::PrUpdatedAtMergeTime => ItemState::TestsPassed,
    }
}

#[cfg(test)]
mod tests {
    use strum::IntoEnumIterator;
    use time::OffsetDateTime;

    use super::*;
    use crate::{Actor, EventId, OrgId, Priority};

    fn all_targets() -> Vec<Target> {
        PendingReason::iter()
            .map(Target::Pending)
            .chain([Target::Testing, Target::PendingFailure])
            .chain(TestsPassedReason::iter().map(Target::TestsPassed))
            .chain(MergedReason::iter().map(Target::Merged))
            .chain(FailedReason::iter().map(Target::Failed))
            .chain(CancelledReason::iter().map(Target::Cancelled))
            .collect()
    }

    /// Edges of the item state diagram in the spec, one line per edge, with
    /// the reasons that its label allows.
    #[allow(clippy::unnested_or_patterns, reason = "one line per diagram edge")]
    fn expected(from: ItemState, to: Target) -> bool {
        use ItemState as S;
        use MergedReason as M;
        use TestsPassedReason as P;
        let failed =
            |prefix: &str| matches!(to, Target::Failed(r) if r.as_str().starts_with(prefix));
        let merge_api_failed = matches!(to, Target::Failed(r) if r != FailedReason::GithubApiDirectMergePrFailed)
            && (failed("github_api_") || failed("pr_updated_at_merge_time"));
        matches!(
            (from, to),
            (
                S::NotReady,
                Target::Pending(PendingReason::MergeItemNowReady)
            ) | (S::Pending, Target::Testing)
                | (S::Pending, Target::Merged(M::MergeGraphDirectMerge))
                | (
                    S::Pending,
                    Target::Failed(FailedReason::GithubApiDirectMergePrFailed)
                )
                | (
                    S::Testing,
                    Target::TestsPassed(P::TestRunPassed | P::PassedByOptimization)
                )
                | (S::Testing, Target::PendingFailure)
                | (S::Testing, Target::Pending(_))
                | (S::PendingFailure, Target::Pending(_))
                | (
                    S::PendingFailure,
                    Target::TestsPassed(P::SuccessorTestRunPassed)
                )
                | (S::TestsPassed, Target::Merged(M::MergeGraphMerge))
                | (S::TestsPassed, Target::Pending(_))
                | (
                    S::NotReady | S::Pending | S::Testing | S::PendingFailure | S::TestsPassed,
                    Target::Cancelled(_) | Target::Merged(M::ManualMerge)
                )
        ) || (from == S::Testing && failed("start_test_run_"))
            || (from == S::PendingFailure && failed("test_run_"))
            || (from == S::TestsPassed && merge_api_failed)
    }

    fn item(state: ItemState) -> Item {
        Item {
            id: ItemId::new(),
            org_id: OrgId::new(),
            queue_id: QueueId::new(),
            pr_number: 1,
            head_sha: "0123456789abcdef0123456789abcdef01234567".into(),
            state,
            priority: Priority::MEDIUM,
            no_batch: false,
            force_enqueued: false,
            position: 1000,
            is_current: true,
        }
    }

    fn event(item: &Item, to: Target) -> Event {
        Event {
            id: EventId::new(),
            org_id: item.org_id,
            queue_id: item.queue_id,
            item_id: Some(item.id),
            group_id: None,
            run_id: None,
            actor: Actor::System,
            occurred_at: OffsetDateTime::UNIX_EPOCH,
            kind: EventKind::ItemStateChanged { to },
        }
    }

    #[test]
    fn follows_the_state_diagram() {
        for from in ItemState::iter() {
            for to in all_targets() {
                let item = item(from);
                let want = if expected(from, to) {
                    Ok(Transition {
                        item_id: item.id,
                        from,
                        to,
                    })
                } else {
                    Err(TransitionError::NotAllowed { from, to })
                };
                assert_eq!(
                    transition(&item, &event(&item, to)),
                    want,
                    "{from} -> {to:?}"
                );
            }
        }
    }

    #[test]
    fn rejects_an_event_for_another_item() {
        let item = item(ItemState::Pending);
        let mut event = event(&item, Target::Testing);
        event.item_id = Some(ItemId::new());
        assert!(matches!(
            transition(&item, &event),
            Err(TransitionError::ItemMismatch { .. })
        ));
    }

    #[test]
    fn target_serializes_state_and_reason() {
        let json = serde_json::to_string(&Target::Failed(FailedReason::TestRunTimeout)).unwrap();
        assert_eq!(json, r#"{"state":"failed","reason":"test_run_timeout"}"#);
        let json = serde_json::to_string(&Target::Testing).unwrap();
        assert_eq!(json, r#"{"state":"testing"}"#);
    }
}

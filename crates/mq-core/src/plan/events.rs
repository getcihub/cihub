//! Constructors of the events that the rules decide.

use cihub_config::QueueSettings;
use cihub_domain::{
    EventKind, GroupId, GroupState, Item, ItemId, ItemState, JobError, QueueState, Reason, RunId,
    RunStatus, TestGroup, TestRun, test_branch,
};

use crate::Snapshot;
use crate::ids::IdGen;

/// Optional fields of an item transition.
#[derive(Default)]
pub(super) struct ItemExtras {
    pub(super) position: Option<i64>,
    pub(super) verified_by_run_id: Option<RunId>,
    pub(super) merged_sha: Option<String>,
}

pub(super) fn item_to(item: &Item, to: ItemState, reason: Reason) -> EventKind {
    item_to_with(item, to, reason, ItemExtras::default())
}

pub(super) fn item_to_with(
    item: &Item,
    to: ItemState,
    reason: Reason,
    extras: ItemExtras,
) -> EventKind {
    EventKind::ItemTransitioned {
        item_id: item.id,
        from: Some(item.state),
        to,
        reason,
        position: extras.position,
        verified_by_run_id: extras.verified_by_run_id,
        merged_sha: extras.merged_sha,
    }
}

pub(super) fn group_to(group: &TestGroup, to: GroupState, reason: Reason) -> EventKind {
    EventKind::GroupTransitioned {
        group_id: group.id,
        from: group.state,
        to,
        reason,
    }
}

pub(super) fn run_to(
    run: &TestRun,
    to: RunStatus,
    reason: Reason,
    failing: Vec<String>,
) -> EventKind {
    EventKind::RunTransitioned {
        run_id: run.id,
        group_id: run.group_id,
        from: Some(run.status),
        to,
        reason,
        attempt: run.attempt,
        test_branch: run.test_branch.clone(),
        failure_causing_statuses: failing,
    }
}

pub(super) fn new_run(
    group_id: GroupId,
    first_pr: u64,
    attempt: u32,
    reason: Reason,
    settings: &QueueSettings,
    ids: &mut IdGen,
) -> EventKind {
    let run_id = RunId::from_ulid(ids.next());
    EventKind::RunTransitioned {
        run_id,
        group_id,
        from: None,
        to: RunStatus::Assembling,
        reason,
        attempt,
        test_branch: test_branch(&settings.branch_prefix.value, first_pr, run_id),
        failure_causing_statuses: Vec::new(),
    }
}

/// The reason of a failed assembly.
pub(super) const fn assembly_reason(error: JobError) -> Reason {
    match error {
        JobError::MergeConflict => Reason::StartTestRunMergeConflict,
        JobError::ConfigParsingFailure => Reason::StartTestRunConfigParsingFailure,
        JobError::ConfigBadVersion => Reason::StartTestRunConfigBadVersion,
        JobError::ConfigMissingRequiredStatuses => {
            Reason::StartTestRunConfigMissingRequiredStatuses
        }
        JobError::ConfigBadRequiredStatuses => Reason::StartTestRunConfigBadRequiredStatuses,
        JobError::NoRequiredStatuses => Reason::StartTestRunNoRequiredStatuses,
        JobError::RetryExhausted => Reason::StartTestRunRetryExhausted,
        JobError::DraftPrsNotAllowed => Reason::StartTestRunDraftPrsNotAllowed,
        JobError::PermissionsBlockTestBranch => Reason::StartTestRunPermissionsBlockTestBranch,
        JobError::InvalidEvent
        | JobError::MergeRejected
        | JobError::RateLimitExceeded
        | JobError::BranchProtectionUnmet
        | JobError::MergeAlreadyInProgress
        | JobError::RepeatedServerErrors
        | JobError::HeadChanged
        | JobError::PrAlreadyMerged
        | JobError::DirectMergeUnavailable => Reason::StartTestRunInvalidEvent,
    }
}

/// The new item of a submission: `not_ready`, or `cancelled` at once while
/// the queue drains or switches its mode.
pub(super) fn submitted(s: &Snapshot, ids: &mut IdGen) -> EventKind {
    let (to, reason) = match s.state {
        QueueState::Draining => (ItemState::Cancelled, Reason::QueueDraining),
        QueueState::SwitchingModes => (ItemState::Cancelled, Reason::QueueSwitchingModes),
        QueueState::Running | QueueState::Paused => (ItemState::NotReady, Reason::Submitted),
    };

    EventKind::ItemTransitioned {
        item_id: ItemId::from_ulid(ids.next()),
        from: None,
        to,
        reason,
        position: None,
        verified_by_run_id: None,
        merged_sha: None,
    }
}

pub(super) fn queue_to(s: &Snapshot, to: QueueState, reason: Reason) -> EventKind {
    EventKind::QueueTransitioned {
        from: s.state,
        to,
        reason,
    }
}

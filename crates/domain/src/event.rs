use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{
    BlockReason, CheckReport, ConflictCulprit, EventId, GroupId, GroupKind, GroupState,
    ImpactedTargets, ItemId, ItemState, JobError, Mergeability, OrgId, Permission, PrState,
    Priority, QueueControl, QueueId, QueueMode, QueueState, Reason, RequiredStatusesSource,
    ReviewDecision, RunId, RunStatus, SubmittedVia, UserId,
};

/// Immutable, append-only record of a fact or a decision in the log of a
/// queue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub id: EventId,
    pub org_id: OrgId,
    pub queue_id: QueueId,
    pub actor: Actor,
    #[serde(with = "time::serde::rfc3339")]
    pub occurred_at: OffsetDateTime,
    /// The fact that caused a decision; `None` for a fact.
    pub cause_id: Option<EventId>,
    pub kind: EventKind,
}

/// Payload of an event. Facts change only the observed state; decisions
/// change only the state of the queue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventKind {
    #[serde(rename = "item.submitted")]
    ItemSubmitted {
        pr_number: u64,
        head_sha: String,
        priority: Priority,
        no_batch: bool,
        direct: bool,
        submitted_via: SubmittedVia,
        author_id: UserId,
        additions: u64,
        deletions: u64,
        changed_files: u64,
        sender_permission: Option<Permission>,
        /// The profile of the queue that the submission chose, if any.
        profile: Option<String>,
    },
    #[serde(rename = "item.cancel_requested")]
    ItemCancelRequested { item_id: ItemId, by_admin: bool },
    #[serde(rename = "item.restart_requested")]
    ItemRestartRequested { item_id: ItemId, by_api: bool },
    #[serde(rename = "pr.observed")]
    PrObserved {
        pr_number: u64,
        state: PrState,
        head_sha: String,
        base_branch: String,
        draft: bool,
        labels: BTreeSet<String>,
        mergeability: Mergeability,
        review_decision: ReviewDecision,
        checks: Vec<CheckReport>,
        unmet_conditions: Vec<String>,
        required_checks: Vec<String>,
    },
    #[serde(rename = "target.pushed")]
    TargetPushed {
        before: Option<String>,
        after: String,
        sender_id: Option<UserId>,
    },
    #[serde(rename = "check.observed")]
    CheckObserved {
        test_sha: String,
        check: CheckReport,
    },
    #[serde(rename = "targets.received")]
    TargetsReceived {
        pr_number: u64,
        head_sha: String,
        targets: ImpactedTargets,
    },
    #[serde(rename = "test_branch.assembled")]
    TestBranchAssembled {
        run_id: RunId,
        test_sha: String,
        draft_pr_number: Option<u64>,
        required_statuses: Vec<String>,
        required_statuses_source: RequiredStatusesSource,
    },
    #[serde(rename = "test_branch.assembly_failed")]
    TestBranchAssemblyFailed {
        run_id: RunId,
        error: JobError,
        conflict_culprit: Option<ConflictCulprit>,
    },
    #[serde(rename = "merge.succeeded")]
    MergeSucceeded { item_id: ItemId, merged_sha: String },
    #[serde(rename = "merge.failed")]
    MergeFailed {
        item_id: ItemId,
        direct: bool,
        error: JobError,
    },
    #[serde(rename = "queue.control_requested")]
    QueueControlRequested { control: QueueControl },
    /// Only the mode: the other settings are a parameter of `plan`.
    #[serde(rename = "queue.settings_changed")]
    QueueSettingsChanged { before: QueueMode, after: QueueMode },
    #[serde(rename = "access.lost")]
    AccessLost,
    #[serde(rename = "access.restored")]
    AccessRestored,
    /// A creation has no state before.
    #[serde(rename = "item.transitioned")]
    ItemTransitioned {
        item_id: ItemId,
        from: Option<ItemState>,
        to: ItemState,
        reason: Reason,
        position: Option<i64>,
        verified_by_run_id: Option<RunId>,
        merged_sha: Option<String>,
    },
    #[serde(rename = "item.readiness_changed")]
    ItemReadinessChanged {
        item_id: ItemId,
        mergeability: Mergeability,
        block_reasons: Vec<BlockReason>,
    },
    /// New positions after the interval ran out or an `urgent` preemption.
    #[serde(rename = "queue.renumbered")]
    QueueRenumbered { positions: BTreeMap<ItemId, i64> },
    #[serde(rename = "group.formed")]
    GroupFormed {
        group_id: GroupId,
        kind: GroupKind,
        item_ids: Vec<ItemId>,
        predecessor_ids: Vec<ItemId>,
        base_sha: String,
        parent_group_id: Option<GroupId>,
    },
    #[serde(rename = "group.transitioned")]
    GroupTransitioned {
        group_id: GroupId,
        from: GroupState,
        to: GroupState,
        reason: Reason,
    },
    /// A creation has no status before.
    #[serde(rename = "run.transitioned")]
    RunTransitioned {
        run_id: RunId,
        group_id: GroupId,
        from: Option<RunStatus>,
        to: RunStatus,
        reason: Reason,
        attempt: u32,
        test_branch: String,
        failure_causing_statuses: Vec<String>,
    },
    #[serde(rename = "queue.transitioned")]
    QueueTransitioned {
        from: QueueState,
        to: QueueState,
        reason: Reason,
    },
    #[serde(rename = "queue.window_changed")]
    QueueWindowChanged { before: u32, after: u32 },
}

closed_enum! {
    /// Type of an event, stored in `events.type`. The name has the form
    /// `<subject>.<verb>`.
    pub enum EventType {
        /// A user, a label, the API or the CLI submitted a PR.
        #[serde(rename = "item.submitted")]
        #[strum(serialize = "item.submitted")]
        ItemSubmitted,
        /// A user or an administrator asked to cancel an item.
        #[serde(rename = "item.cancel_requested")]
        #[strum(serialize = "item.cancel_requested")]
        ItemCancelRequested,
        /// A user or the API asked to restart the tests of an item.
        #[serde(rename = "item.restart_requested")]
        #[strum(serialize = "item.restart_requested")]
        ItemRestartRequested,
        /// A webhook, the tick or a resync read the state of a PR.
        #[serde(rename = "pr.observed")]
        #[strum(serialize = "pr.observed")]
        PrObserved,
        /// The target branch moved.
        #[serde(rename = "target.pushed")]
        #[strum(serialize = "target.pushed")]
        TargetPushed,
        /// A check reported on the test SHA of a live run.
        #[serde(rename = "check.observed")]
        #[strum(serialize = "check.observed")]
        CheckObserved,
        /// The impacted targets of a PR head arrived.
        #[serde(rename = "targets.received")]
        #[strum(serialize = "targets.received")]
        TargetsReceived,
        /// The assembly job created the test commit.
        #[serde(rename = "test_branch.assembled")]
        #[strum(serialize = "test_branch.assembled")]
        TestBranchAssembled,
        /// The assembly job failed.
        #[serde(rename = "test_branch.assembly_failed")]
        #[strum(serialize = "test_branch.assembly_failed")]
        TestBranchAssemblyFailed,
        /// The merge job merged the PR.
        #[serde(rename = "merge.succeeded")]
        #[strum(serialize = "merge.succeeded")]
        MergeSucceeded,
        /// The merge job failed.
        #[serde(rename = "merge.failed")]
        #[strum(serialize = "merge.failed")]
        MergeFailed,
        /// An operator asked to pause, resume or drain the queue.
        #[serde(rename = "queue.control_requested")]
        #[strum(serialize = "queue.control_requested")]
        QueueControlRequested,
        /// An operator changed the mode of the queue.
        #[serde(rename = "queue.settings_changed")]
        #[strum(serialize = "queue.settings_changed")]
        QueueSettingsChanged,
        /// CIHub lost its access to the repository.
        #[serde(rename = "access.lost")]
        #[strum(serialize = "access.lost")]
        AccessLost,
        /// CIHub has its access to the repository again.
        #[serde(rename = "access.restored")]
        #[strum(serialize = "access.restored")]
        AccessRestored,
        /// Decision: an item changed state.
        #[serde(rename = "item.transitioned")]
        #[strum(serialize = "item.transitioned")]
        ItemTransitioned,
        /// Decision: new mergeability and block reasons of an item.
        #[serde(rename = "item.readiness_changed")]
        #[strum(serialize = "item.readiness_changed")]
        ItemReadinessChanged,
        /// Decision: new positions.
        #[serde(rename = "queue.renumbered")]
        #[strum(serialize = "queue.renumbered")]
        QueueRenumbered,
        /// Decision: a new group.
        #[serde(rename = "group.formed")]
        #[strum(serialize = "group.formed")]
        GroupFormed,
        /// Decision: a group changed state.
        #[serde(rename = "group.transitioned")]
        #[strum(serialize = "group.transitioned")]
        GroupTransitioned,
        /// Decision: a run changed status.
        #[serde(rename = "run.transitioned")]
        #[strum(serialize = "run.transitioned")]
        RunTransitioned,
        /// Decision: the queue changed state.
        #[serde(rename = "queue.transitioned")]
        #[strum(serialize = "queue.transitioned")]
        QueueTransitioned,
        /// Decision: a new speculation window.
        #[serde(rename = "queue.window_changed")]
        #[strum(serialize = "queue.window_changed")]
        QueueWindowChanged,
    }
}

impl EventType {
    /// A decision changes the state of the queue; a fact only the observed
    /// state.
    pub const fn is_decision(self) -> bool {
        matches!(
            self,
            Self::ItemTransitioned
                | Self::ItemReadinessChanged
                | Self::QueueRenumbered
                | Self::GroupFormed
                | Self::GroupTransitioned
                | Self::RunTransitioned
                | Self::QueueTransitioned
                | Self::QueueWindowChanged
        )
    }
}

/// The item, group and run that an event is about, stored next to the event
/// to read the timeline of one of them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Subjects {
    pub item_id: Option<ItemId>,
    pub group_id: Option<GroupId>,
    pub run_id: Option<RunId>,
}

impl EventKind {
    pub const fn subjects(&self) -> Subjects {
        let (item_id, group_id, run_id) = match self {
            Self::ItemCancelRequested { item_id, .. }
            | Self::ItemRestartRequested { item_id, .. }
            | Self::MergeSucceeded { item_id, .. }
            | Self::MergeFailed { item_id, .. }
            | Self::ItemTransitioned { item_id, .. }
            | Self::ItemReadinessChanged { item_id, .. } => (Some(*item_id), None, None),
            Self::GroupFormed { group_id, .. } | Self::GroupTransitioned { group_id, .. } => {
                (None, Some(*group_id), None)
            }
            Self::RunTransitioned {
                run_id, group_id, ..
            } => (None, Some(*group_id), Some(*run_id)),
            Self::TestBranchAssembled { run_id, .. }
            | Self::TestBranchAssemblyFailed { run_id, .. } => (None, None, Some(*run_id)),
            Self::ItemSubmitted { .. }
            | Self::PrObserved { .. }
            | Self::TargetPushed { .. }
            | Self::CheckObserved { .. }
            | Self::TargetsReceived { .. }
            | Self::QueueControlRequested { .. }
            | Self::QueueSettingsChanged { .. }
            | Self::AccessLost
            | Self::AccessRestored
            | Self::QueueRenumbered { .. }
            | Self::QueueTransitioned { .. }
            | Self::QueueWindowChanged { .. } => (None, None, None),
        };
        Subjects {
            item_id,
            group_id,
            run_id,
        }
    }

    pub const fn event_type(&self) -> EventType {
        match self {
            Self::ItemSubmitted { .. } => EventType::ItemSubmitted,
            Self::ItemCancelRequested { .. } => EventType::ItemCancelRequested,
            Self::ItemRestartRequested { .. } => EventType::ItemRestartRequested,
            Self::PrObserved { .. } => EventType::PrObserved,
            Self::TargetPushed { .. } => EventType::TargetPushed,
            Self::CheckObserved { .. } => EventType::CheckObserved,
            Self::TargetsReceived { .. } => EventType::TargetsReceived,
            Self::TestBranchAssembled { .. } => EventType::TestBranchAssembled,
            Self::TestBranchAssemblyFailed { .. } => EventType::TestBranchAssemblyFailed,
            Self::MergeSucceeded { .. } => EventType::MergeSucceeded,
            Self::MergeFailed { .. } => EventType::MergeFailed,
            Self::QueueControlRequested { .. } => EventType::QueueControlRequested,
            Self::QueueSettingsChanged { .. } => EventType::QueueSettingsChanged,
            Self::AccessLost => EventType::AccessLost,
            Self::AccessRestored => EventType::AccessRestored,
            Self::ItemTransitioned { .. } => EventType::ItemTransitioned,
            Self::ItemReadinessChanged { .. } => EventType::ItemReadinessChanged,
            Self::QueueRenumbered { .. } => EventType::QueueRenumbered,
            Self::GroupFormed { .. } => EventType::GroupFormed,
            Self::GroupTransitioned { .. } => EventType::GroupTransitioned,
            Self::RunTransitioned { .. } => EventType::RunTransitioned,
            Self::QueueTransitioned { .. } => EventType::QueueTransitioned,
            Self::QueueWindowChanged { .. } => EventType::QueueWindowChanged,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Actor {
    System,
    Bot,
    User { user_id: UserId },
}

closed_enum! {
    /// Kind of the actor of an event, stored in `events.actor_kind`.
    pub enum ActorKind {
        /// CIHub itself.
        System,
        /// The account that CIHub acts with on the SCM.
        Bot,
        /// A user of the SCM.
        User,
    }
}

impl Actor {
    pub const fn kind(&self) -> ActorKind {
        match self {
            Self::System => ActorKind::System,
            Self::Bot => ActorKind::Bot,
            Self::User { .. } => ActorKind::User,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use strum::IntoEnumIterator;

    use super::*;

    #[test]
    fn event_type_names_are_dotted() {
        for t in EventType::iter() {
            assert_eq!(serde_json::to_string(&t).unwrap(), format!("\"{t}\""));
            assert_eq!(EventType::from_str(t.as_str()), Ok(t));
            assert!(t.as_str().contains('.'), "{t}");
        }
    }

    #[test]
    fn payload_is_tagged_by_event_type() {
        let kind = EventKind::QueueWindowChanged {
            before: 2,
            after: 3,
        };
        let json = serde_json::to_value(&kind).unwrap();

        assert_eq!(json["type"], kind.event_type().as_str());
        assert_eq!(json["payload"]["after"], 3);
        assert_eq!(
            serde_json::to_value(EventKind::AccessLost).unwrap()["type"],
            "access.lost"
        );
    }

    #[test]
    fn eight_types_are_decisions() {
        let decisions: Vec<_> = EventType::iter().filter(|t| t.is_decision()).collect();
        assert_eq!(decisions.len(), 8);
        assert!(!EventType::PrObserved.is_decision());
    }

    #[test]
    fn subjects_name_the_item_group_and_run() {
        let item_id = ItemId::from_ulid(ulid::Ulid(1));
        let group_id = GroupId::from_ulid(ulid::Ulid(2));
        let run_id = RunId::from_ulid(ulid::Ulid(3));
        let merged = EventKind::MergeSucceeded {
            item_id,
            merged_sha: "abc".into(),
        };

        assert_eq!(merged.subjects().item_id, Some(item_id));
        let run = EventKind::RunTransitioned {
            run_id,
            group_id,
            from: None,
            to: RunStatus::Assembling,
            reason: Reason::TestRunStarted,
            attempt: 1,
            test_branch: "cihub/run".into(),
            failure_causing_statuses: vec![],
        };

        assert_eq!(
            run.subjects(),
            Subjects {
                item_id: None,
                group_id: Some(group_id),
                run_id: Some(run_id)
            }
        );
        assert_eq!(EventKind::AccessLost.subjects(), Subjects::default());
    }

    #[test]
    fn actor_kind_follows_the_actor() {
        let user = Actor::User {
            user_id: UserId::from_ulid(ulid::Ulid(1)),
        };
        assert_eq!(user.kind(), ActorKind::User);
        assert_eq!(Actor::Bot.kind(), ActorKind::Bot);
        assert_eq!(Actor::System.kind(), ActorKind::System);
    }
}

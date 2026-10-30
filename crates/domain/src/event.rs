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
    /// The event.
    pub id: EventId,
    /// The organization of the queue.
    pub org_id: OrgId,
    /// The queue whose log holds the event.
    pub queue_id: QueueId,
    /// Who or what caused the event.
    pub actor: Actor,
    /// When the fact happened, or when the queue decided.
    #[serde(with = "time::serde::rfc3339")]
    pub occurred_at: OffsetDateTime,
    /// The fact that caused a decision; `None` for a fact.
    pub cause_id: Option<EventId>,
    /// What happened, and its data.
    pub kind: EventKind,
}

/// Payload of an event. Facts change only the observed state; decisions
/// change only the state of the queue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventKind {
    /// Fact: a PR was submitted to the queue.
    #[serde(rename = "item.submitted")]
    ItemSubmitted {
        /// The number of the PR on its repository.
        pr_number: u64,
        /// The head of the PR when it was submitted.
        head_sha: String,
        /// The priority that the submission asked for.
        priority: Priority,
        /// The PR is tested alone, never in a batch.
        no_batch: bool,
        /// How the PR was submitted.
        submitted_via: SubmittedVia,
        /// The author of the PR.
        author_id: UserId,
        /// The lines that the PR adds.
        additions: u64,
        /// The lines that the PR deletes.
        deletions: u64,
        /// The files that the PR changes.
        changed_files: u64,
        /// The permission on the repository of who submitted the PR; `None`
        /// when it was not read.
        sender_permission: Option<Permission>,
        /// The profile of the queue that the submission chose, if any.
        profile: Option<String>,
    },
    /// Fact: someone asked to cancel an item.
    #[serde(rename = "item.cancel_requested")]
    ItemCancelRequested {
        /// The item to cancel.
        item_id: ItemId,
        /// An administrator asked, rather than a user.
        by_admin: bool,
    },
    /// Fact: someone asked to restart the tests of an item.
    #[serde(rename = "item.restart_requested")]
    ItemRestartRequested {
        /// The item to restart.
        item_id: ItemId,
        /// An API request asked, rather than a user on the PR.
        by_api: bool,
    },
    /// Fact: CIHub read the state of a PR on the SCM.
    #[serde(rename = "pr.observed")]
    PrObserved {
        /// The number of the PR on its repository.
        pr_number: u64,
        /// The page of the PR on the SCM.
        url: String,
        /// Whether the PR is open, closed or merged.
        state: PrState,
        /// The head of the PR.
        head_sha: String,
        /// The branch that the PR merges into.
        base_branch: String,
        /// The PR is a draft.
        draft: bool,
        /// The labels of the PR.
        labels: BTreeSet<String>,
        /// Whether the SCM accepts to merge the PR.
        mergeability: Mergeability,
        /// The review decision of the PR.
        review_decision: ReviewDecision,
        /// The checks and statuses of the head.
        checks: Vec<CheckReport>,
        /// The `queue_conditions` that the PR does not meet, as the file
        /// writes them.
        unmet_conditions: Vec<String>,
        /// The checks that the rules of the base branch require.
        required_checks: Vec<String>,
    },
    /// Fact: the target branch moved.
    #[serde(rename = "target.pushed")]
    TargetPushed {
        /// The head before the push; `None` when a read of the branch, not a
        /// delivery of the push, found the new head.
        before: Option<String>,
        /// The head after the push.
        after: String,
        /// Who pushed; `None` when a read found the new head.
        sender_id: Option<UserId>,
    },
    /// Fact: a check reported on the test commit of a live run.
    #[serde(rename = "check.observed")]
    CheckObserved {
        /// The test commit of the run.
        test_sha: String,
        /// What the check reported.
        check: CheckReport,
    },
    /// Fact: the CI sent the impacted targets of a head of a PR.
    #[serde(rename = "targets.received")]
    TargetsReceived {
        /// The number of the PR on its repository.
        pr_number: u64,
        /// The head of the PR that the targets describe.
        head_sha: String,
        /// What the PR changes.
        targets: ImpactedTargets,
    },
    /// Fact: the assembly job created the test commit of a run.
    #[serde(rename = "test_branch.assembled")]
    TestBranchAssembled {
        /// The run.
        run_id: RunId,
        /// The test commit.
        test_sha: String,
        /// The draft PR that triggers the CI; `None` when the trigger is not
        /// `draft_pr`.
        draft_pr_number: Option<u64>,
        /// The checks that must pass on the test commit.
        required_statuses: Vec<String>,
        /// Where the required statuses come from.
        required_statuses_source: RequiredStatusesSource,
    },
    /// Fact: the assembly job failed.
    #[serde(rename = "test_branch.assembly_failed")]
    TestBranchAssemblyFailed {
        /// The run.
        run_id: RunId,
        /// Why the job failed.
        error: JobError,
        /// What a member conflicts with, for a merge conflict.
        conflict_culprit: Option<ConflictCulprit>,
    },
    /// Fact: the merge job merged the PR of an item.
    #[serde(rename = "merge.succeeded")]
    MergeSucceeded {
        /// The item.
        item_id: ItemId,
        /// The commit that the merge put on the target branch.
        merged_sha: String,
    },
    /// Fact: the merge job failed.
    #[serde(rename = "merge.failed")]
    MergeFailed {
        /// The item.
        item_id: ItemId,
        /// The merge was a direct merge, without a test branch.
        direct: bool,
        /// Why the job failed.
        error: JobError,
    },
    /// Fact: an operator asked to pause, resume or drain the queue.
    #[serde(rename = "queue.control_requested")]
    QueueControlRequested {
        /// What the operator asked.
        control: QueueControl,
    },
    /// Fact: an operator changed the mode of the queue. Only the mode: the
    /// other settings are a parameter of `plan`.
    #[serde(rename = "queue.settings_changed")]
    QueueSettingsChanged {
        /// The mode before the change.
        before: QueueMode,
        /// The mode after the change.
        after: QueueMode,
    },
    /// Fact: CIHub lost its access to the repository.
    #[serde(rename = "access.lost")]
    AccessLost,
    /// Fact: CIHub has its access to the repository again.
    #[serde(rename = "access.restored")]
    AccessRestored,
    /// Decision: an item changed state. A creation has no state before.
    #[serde(rename = "item.transitioned")]
    ItemTransitioned {
        /// The item.
        item_id: ItemId,
        /// The state before; `None` for a creation.
        from: Option<ItemState>,
        /// The state after.
        to: ItemState,
        /// Why the item changed state.
        reason: Reason,
        /// The new place of the item in the queue; `None` keeps its place.
        position: Option<i64>,
        /// The run that proved the item; `None` keeps the value that the item
        /// has.
        verified_by_run_id: Option<RunId>,
        /// The commit that the merge put on the target branch; `None` keeps
        /// the value that the item has.
        merged_sha: Option<String>,
    },
    /// Decision: new mergeability and block reasons of an item.
    #[serde(rename = "item.readiness_changed")]
    ItemReadinessChanged {
        /// The item.
        item_id: ItemId,
        /// Whether the SCM accepts to merge the PR.
        mergeability: Mergeability,
        /// Why the item cannot enter the queue yet; empty when it can.
        block_reasons: Vec<BlockReason>,
    },
    /// Decision: new positions after the interval ran out or an `urgent`
    /// preemption.
    #[serde(rename = "queue.renumbered")]
    QueueRenumbered {
        /// The new place of each item of the queue.
        positions: BTreeMap<ItemId, i64>,
    },
    /// Decision: a new test group.
    #[serde(rename = "group.formed")]
    GroupFormed {
        /// The group.
        group_id: GroupId,
        /// One PR, a batch, or a half of a bisection.
        kind: GroupKind,
        /// The members, in queue order.
        item_ids: Vec<ItemId>,
        /// The items that the group is tested on top of, in queue order.
        predecessor_ids: Vec<ItemId>,
        /// The head of the target branch that the test branch starts from.
        base_sha: String,
        /// The failed batch that a half of a bisection comes from.
        parent_group_id: Option<GroupId>,
    },
    /// Decision: a group changed state.
    #[serde(rename = "group.transitioned")]
    GroupTransitioned {
        /// The group.
        group_id: GroupId,
        /// The state before.
        from: GroupState,
        /// The state after.
        to: GroupState,
        /// Why the group changed state.
        reason: Reason,
    },
    /// Decision: a run changed status. A creation has no status before.
    #[serde(rename = "run.transitioned")]
    RunTransitioned {
        /// The run.
        run_id: RunId,
        /// The group that the run tests.
        group_id: GroupId,
        /// The status before; `None` for a creation.
        from: Option<RunStatus>,
        /// The status after.
        to: RunStatus,
        /// Why the run changed status.
        reason: Reason,
        /// The attempt of the group that the run is, from 1.
        attempt: u32,
        /// The branch that holds the test commit.
        test_branch: String,
        /// The required statuses that failed the run.
        failure_causing_statuses: Vec<String>,
    },
    /// Decision: the queue changed state.
    #[serde(rename = "queue.transitioned")]
    QueueTransitioned {
        /// The state before.
        from: QueueState,
        /// The state after.
        to: QueueState,
        /// Why the queue changed state.
        reason: Reason,
    },
    /// Decision: a new speculation window, the number of groups that the
    /// queue tests at the same time.
    #[serde(rename = "queue.window_changed")]
    QueueWindowChanged {
        /// The window before.
        before: u32,
        /// The window after.
        after: u32,
    },
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
        /// A webhook, the tick or a sync read the state of a PR.
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
    /// The item of the event, if any.
    pub item_id: Option<ItemId>,
    /// The group of the event, if any.
    pub group_id: Option<GroupId>,
    /// The run of the event, if any.
    pub run_id: Option<RunId>,
}

impl EventKind {
    /// The item, group and run that the event is about.
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

    /// The type of the event, as `events.type` stores it.
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

/// Who or what caused an event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Actor {
    /// CIHub itself: a job, the tick or the repair loop.
    System,
    /// The account that CIHub acts with on the SCM.
    Bot,
    /// A person.
    User {
        /// The person.
        user_id: UserId,
    },
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
    /// The kind of the actor, as `events.actor_kind` stores it.
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
    fn a_stored_submission_with_the_removed_direct_field_still_reads() {
        let kind = EventKind::ItemSubmitted {
            pr_number: 7,
            head_sha: "abc".into(),
            priority: Priority::MEDIUM,
            no_batch: false,
            submitted_via: SubmittedVia::Comment,
            author_id: UserId::from_ulid(ulid::Ulid(1)),
            additions: 1,
            deletions: 0,
            changed_files: 1,
            sender_permission: None,
            profile: None,
        };
        let mut json = serde_json::to_value(&kind).unwrap();
        json["payload"]["direct"] = true.into();

        assert_eq!(serde_json::from_value::<EventKind>(json).unwrap(), kind);
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

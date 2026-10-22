//! State of one queue, as `apply` builds it from the log. It serializes to
//! JSON for the snapshots of the log.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use cihub_domain::{
    CheckKind, CheckObservation, CheckReport, EventId, GroupId, ImpactedTargets, Item, ItemId,
    JobError, Mergeability, OrgId, PrState, Priority, QueueControl, QueueId, QueueMode, QueueState,
    ReviewDecision, RunId, RunStatus, SubmittedVia, TestGroup, TestRun,
};

/// An observation with the fact that produced it, so that a decision can
/// cite its cause.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observed<T> {
    pub value: T,
    pub fact_id: EventId,
    /// The time of the fact.
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
}

/// The last state of a PR that the SCM gave.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrObservation {
    pub url: String,
    pub state: PrState,
    pub head_sha: String,
    pub base_branch: String,
    pub draft: bool,
    pub labels: BTreeSet<String>,
    pub mergeability: Mergeability,
    pub review_decision: ReviewDecision,
    pub checks: Vec<CheckReport>,
    pub unmet_conditions: Vec<String>,
    pub required_checks: Vec<String>,
}

/// The impacted targets uploaded for one head of a PR.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetsUpload {
    pub head_sha: String,
    pub targets: ImpactedTargets,
}

/// Number of past heads of the target branch kept to recognise a late push.
pub(crate) const RECENT_HEADS: usize = 50;

/// What CIHub knows of the target branch.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetBranch {
    pub head: Option<Observed<String>>,
    /// Heads that a live group may use as `base_sha`: the head after the
    /// last push from outside the queue, and the heads that the queue merges
    /// made since.
    pub valid_bases: BTreeSet<String>,
    /// Past heads, the latest last.
    pub recent_heads: VecDeque<String>,
    /// SHAs that merge jobs reported and that no push has shown yet.
    pub own_shas: BTreeSet<String>,
}

/// A fact that waits for a decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Request {
    Submit {
        pr_number: u64,
        head_sha: String,
        priority: Priority,
        no_batch: bool,
        direct: bool,
        submitted_via: SubmittedVia,
        profile: Option<String>,
    },
    Cancel {
        item_id: ItemId,
        by_admin: bool,
    },
    Restart {
        item_id: ItemId,
        by_api: bool,
    },
    Assembled {
        run_id: RunId,
    },
    AssemblyFailed {
        run_id: RunId,
        error: JobError,
    },
    MergeSucceeded {
        item_id: ItemId,
        merged_sha: String,
    },
    MergeFailed {
        item_id: ItemId,
        direct: bool,
        error: JobError,
    },
    Control(QueueControl),
    AccessLost,
    AccessRestored,
}

/// What a decision acts on, to keep its cause.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum Subject {
    Item(ItemId),
    Group(GroupId),
    Run(RunId),
}

/// The state of a queue. Concluded entities leave it when no live group
/// cites them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub org_id: OrgId,
    pub queue_id: QueueId,
    pub target_branch: String,
    pub state: QueueState,
    /// The mode in force.
    pub mode: QueueMode,
    /// The mode of the settings; it comes into force after `switching_modes`.
    pub requested_mode: QueueMode,
    pub window: Option<u32>,
    pub events_applied: u64,
    pub target: TargetBranch,
    pub items: BTreeMap<ItemId, Item>,
    pub groups: BTreeMap<GroupId, TestGroup>,
    pub runs: BTreeMap<RunId, TestRun>,
    #[serde(with = "checks")]
    pub checks: BTreeMap<RunId, BTreeMap<(String, CheckKind), Observed<CheckObservation>>>,
    pub prs: BTreeMap<u64, Observed<PrObservation>>,
    pub targets: BTreeMap<u64, Observed<TargetsUpload>>,
    pub requests: BTreeMap<EventId, Request>,
    /// The fact that caused the last decision on each subject.
    #[serde(with = "pairs")]
    pub causes: BTreeMap<Subject, EventId>,
}

/// A map whose key is not a string, as a list of pairs: a JSON object has
/// only string keys.
mod pairs {
    use std::collections::BTreeMap;

    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub(super) fn serialize<K: Serialize, V: Serialize, S: Serializer>(
        map: &BTreeMap<K, V>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(map)
    }

    pub(super) fn deserialize<'de, K, V, D>(deserializer: D) -> Result<BTreeMap<K, V>, D::Error>
    where
        K: Deserialize<'de> + Ord,
        V: Deserialize<'de>,
        D: Deserializer<'de>,
    {
        Ok(Vec::<(K, V)>::deserialize(deserializer)?
            .into_iter()
            .collect())
    }
}

/// The checks of each run; the checks of a run are keyed by name and kind.
mod checks {
    use std::collections::BTreeMap;

    use cihub_domain::{CheckKind, CheckObservation, RunId};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::Observed;

    type Checks = BTreeMap<RunId, BTreeMap<(String, CheckKind), Observed<CheckObservation>>>;
    type Pairs = Vec<((String, CheckKind), Observed<CheckObservation>)>;

    pub(super) fn serialize<S: Serializer>(
        checks: &Checks,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let runs: BTreeMap<&RunId, Vec<_>> = checks
            .iter()
            .map(|(run, checks)| (run, checks.iter().collect()))
            .collect();
        runs.serialize(serializer)
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Checks, D::Error> {
        let runs = BTreeMap::<RunId, Pairs>::deserialize(deserializer)?;
        Ok(runs
            .into_iter()
            .map(|(run, checks)| (run, checks.into_iter().collect()))
            .collect())
    }
}

impl Snapshot {
    /// An empty queue in `running`.
    pub fn new(org_id: OrgId, queue_id: QueueId, target_branch: String, mode: QueueMode) -> Self {
        Self {
            org_id,
            queue_id,
            target_branch,
            state: QueueState::Running,
            mode,
            requested_mode: mode,
            window: None,
            events_applied: 0,
            target: TargetBranch::default(),
            items: BTreeMap::new(),
            groups: BTreeMap::new(),
            runs: BTreeMap::new(),
            checks: BTreeMap::new(),
            prs: BTreeMap::new(),
            targets: BTreeMap::new(),
            requests: BTreeMap::new(),
            causes: BTreeMap::new(),
        }
    }

    /// Admitted items that are not concluded, in queue order.
    pub fn queue_order(&self) -> Vec<&Item> {
        let mut items: Vec<_> = self
            .items
            .values()
            .filter(|i| i.position.is_some() && !i.state.is_terminal())
            .collect();
        items.sort_by_key(|i| i.position);
        items
    }

    /// The live item of a PR.
    pub fn current_item(&self, pr_number: u64) -> Option<&Item> {
        self.items
            .values()
            .find(|i| i.pr_number == pr_number && i.is_current && !i.state.is_terminal())
    }

    pub fn live_groups(&self) -> impl Iterator<Item = &TestGroup> {
        self.groups.values().filter(|g| !g.state.is_terminal())
    }

    /// The run of a group with the highest attempt.
    pub fn latest_run(&self, group_id: GroupId) -> Option<&TestRun> {
        self.runs
            .values()
            .filter(|r| r.group_id == group_id)
            .max_by_key(|r| r.attempt)
    }

    pub fn live_run(&self, group_id: GroupId) -> Option<&TestRun> {
        self.latest_run(group_id)
            .filter(|r| matches!(r.status, RunStatus::Assembling | RunStatus::InProgress))
    }

    pub fn cause(&self, subject: Subject) -> Option<EventId> {
        self.causes.get(&subject).copied()
    }
}

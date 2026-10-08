//! A small world around one queue: it writes facts as GitHub and the jobs
//! would, runs `plan` after each one, and keeps the log.

#![allow(dead_code, reason = "each test file uses a part of the world")]

use std::collections::BTreeSet;

use cihub_config::{Layer, QueueSettings, Setting};
use cihub_domain::{
    Actor, CheckConclusion, CheckKind, CheckReport, Event, EventId, EventKind, Item, ItemState,
    JobError, Mergeability, OrgId, PrState, Priority, QueueId, QueueMode, Reason,
    RequiredStatusesSource, ReviewDecision, RunId, RunStatus, SubmittedVia, UserId,
};
use cihub_mq_core::{Outcome, Plan, Snapshot, apply, check, plan};
use time::{Duration, OffsetDateTime};
use ulid::Ulid;

pub const REQUIRED: &str = "ci";

/// A small generator with a fixed seed, so that a test gives the same queue
/// each time.
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// `true` with the probability `percent` / 100.
    pub fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }

    /// A uniform value in (0, 1], with 32 bits.
    pub fn unit(&mut self) -> f64 {
        let value = f64::from(u32::try_from((self.next() >> 32) + 1).unwrap_or(u32::MAX));
        value / f64::from(u32::MAX)
    }
}

pub fn settings(edit: impl FnOnce(&mut Layer)) -> QueueSettings {
    let mut queue = Layer::default();
    edit(&mut queue);
    QueueSettings::resolve(&queue, &Layer::default()).unwrap()
}

pub fn window(n: u32) -> impl FnOnce(&mut Layer) {
    move |l| {
        l.window_floor = Some(Setting::new(n));
        l.window_ceiling = Some(Setting::new(n));
    }
}

pub fn empty(mode: QueueMode) -> Snapshot {
    Snapshot::new(
        OrgId::from_ulid(Ulid(1)),
        QueueId::from_ulid(Ulid(2)),
        "main".into(),
        mode,
    )
}

pub struct World {
    pub initial: Snapshot,
    pub snapshot: Snapshot,
    pub log: Vec<Event>,
    pub settings: QueueSettings,
    pub now: OffsetDateTime,
    /// The surfaces of every reconciliation, in order.
    pub surfaces: Vec<Vec<cihub_mq_core::Surface>>,
    facts: u128,
    target: u32,
}

impl World {
    pub fn new(settings: QueueSettings) -> Self {
        let snapshot = empty(settings.mode.value);
        let mut world = Self {
            initial: snapshot.clone(),
            snapshot,
            log: Vec::new(),
            settings,
            now: OffsetDateTime::UNIX_EPOCH + Duration::days(20_000),
            surfaces: Vec::new(),
            facts: 0,
            target: 0,
        };
        world.push_target(Actor::System);
        world
    }

    /// Writes a fact, then reconciles.
    pub fn fact(&mut self, actor: Actor, kind: EventKind) -> Outcome {
        self.facts += 1;
        self.now += Duration::seconds(1);
        let ms = u64::try_from(self.now.unix_timestamp_nanos() / 1_000_000).unwrap();
        let event = Event {
            id: EventId::from_ulid(Ulid::from_parts(ms, self.facts)),
            org_id: self.snapshot.org_id,
            queue_id: self.snapshot.queue_id,
            actor,
            occurred_at: self.now,
            cause_id: None,
            kind,
        };
        let outcome = apply(&mut self.snapshot, &event).unwrap();
        if outcome == Outcome::Applied {
            self.log.push(event);
        }
        self.reconcile();
        outcome
    }

    /// Runs `plan`, applies its decisions, and checks the snapshot.
    pub fn reconcile(&mut self) -> Plan {
        let plan = plan(&self.snapshot, &self.settings, self.now).unwrap();
        for event in &plan.decisions {
            assert_eq!(apply(&mut self.snapshot, event), Ok(Outcome::Applied));
            self.log.push(event.clone());
        }
        self.surfaces.push(plan.desired.surfaces.clone());
        let again = cihub_mq_core::plan(&self.snapshot, &self.settings, self.now).unwrap();
        assert!(again.decisions.is_empty(), "no fixed point: {again:#?}");
        assert_eq!(check(&self.snapshot, &self.settings), vec![]);
        plan
    }

    pub fn advance(&mut self, minutes: i64) {
        self.now += Duration::minutes(minutes);
        self.reconcile();
    }

    /// The latest item of a PR.
    pub fn item(&self, pr_number: u64) -> Option<&Item> {
        self.snapshot
            .items
            .values()
            .filter(|i| i.pr_number == pr_number)
            .max_by_key(|i| i.id)
    }

    pub fn state(&self, pr_number: u64) -> Option<ItemState> {
        self.item(pr_number).map(|i| i.state)
    }

    /// Submits the PR, then observes it open and mergeable.
    pub fn submit(&mut self, pr_number: u64, priority: Priority) -> Outcome {
        let outcome = self.submit_only(pr_number, 0, priority);
        self.observe(pr_number, |_| {});
        outcome
    }

    /// Submits `head(pr_number, version)` without an observation: the test
    /// chooses what the first observation says.
    pub fn submit_only(&mut self, pr_number: u64, version: u32, priority: Priority) -> Outcome {
        self.submit_with(pr_number, version, priority, false)
    }

    /// Submits the PR with `no_batch`, then observes it.
    pub fn submit_no_batch(&mut self, pr_number: u64) -> Outcome {
        let outcome = self.submit_with(pr_number, 0, Priority::MEDIUM, true);
        self.observe(pr_number, |_| {});
        outcome
    }

    fn submit_with(
        &mut self,
        pr_number: u64,
        version: u32,
        priority: Priority,
        no_batch: bool,
    ) -> Outcome {
        self.fact(
            Actor::System,
            EventKind::ItemSubmitted {
                pr_number,
                head_sha: head(pr_number, version),
                priority,
                no_batch,
                direct: false,
                submitted_via: SubmittedVia::Api,
                author_id: UserId::from_ulid(Ulid(1)),
                additions: 0,
                deletions: 0,
                changed_files: 0,
                sender_permission: None,
            },
        )
    }

    /// Observes the PR: open, mergeable, then `edit`.
    pub fn observe(&mut self, pr_number: u64, edit: impl FnOnce(&mut Pr)) -> Outcome {
        let mut pr = Pr {
            state: PrState::Open,
            head_sha: head(pr_number, 0),
            base_branch: "main".into(),
            draft: false,
            mergeability: Mergeability::Mergeable,
            unmet_conditions: Vec::new(),
            required_checks: Vec::new(),
            checks: Vec::new(),
        };
        edit(&mut pr);
        self.fact(
            Actor::System,
            EventKind::PrObserved {
                pr_number,
                state: pr.state,
                head_sha: pr.head_sha,
                base_branch: pr.base_branch,
                draft: pr.draft,
                labels: BTreeSet::new(),
                mergeability: pr.mergeability,
                review_decision: ReviewDecision::Approved,
                checks: pr.checks,
                unmet_conditions: pr.unmet_conditions,
                required_checks: pr.required_checks,
            },
        )
    }

    /// A new head of the target branch.
    pub fn push_target(&mut self, actor: Actor) -> Outcome {
        self.target += 1;
        let after = format!("target-{}", self.target);
        self.push_target_to(actor, after)
    }

    pub fn push_target_to(&mut self, actor: Actor, after: String) -> Outcome {
        let before = self.snapshot.target.head.as_ref().map(|h| h.value.clone());
        self.fact(
            actor,
            EventKind::TargetPushed {
                before,
                after,
                sender_id: Some(UserId::from_ulid(Ulid(1))),
            },
        )
    }

    /// The live run of the latest item of a PR.
    pub fn run(&self, pr_number: u64) -> Option<RunId> {
        let group = self.item(pr_number)?.group_id?;
        self.snapshot.live_run(group).map(|r| r.id)
    }

    /// Assembles every run that waits for it.
    pub fn assemble_all(&mut self) {
        let runs: Vec<RunId> = self
            .snapshot
            .runs
            .values()
            .filter(|r| r.status == RunStatus::Assembling)
            .map(|r| r.id)
            .collect();
        for run_id in runs {
            self.assemble(run_id);
        }
    }

    pub fn assemble(&mut self, run_id: RunId) -> Outcome {
        self.fact(
            Actor::System,
            EventKind::TestBranchAssembled {
                run_id,
                test_sha: format!("test-{run_id}"),
                draft_pr_number: None,
                required_statuses: vec![REQUIRED.into()],
                required_statuses_source: RequiredStatusesSource::RepoConfig,
            },
        )
    }

    pub fn fail_assembly(&mut self, run_id: RunId, error: JobError) -> Outcome {
        self.fact(
            Actor::System,
            EventKind::TestBranchAssemblyFailed {
                run_id,
                error,
                conflict_culprit: None,
            },
        )
    }

    /// Reports the required check on the live run of a PR.
    pub fn report(&mut self, pr_number: u64, conclusion: CheckConclusion) -> Outcome {
        let run_id = self.run(pr_number).unwrap();
        self.report_run(run_id, conclusion)
    }

    pub fn report_run(&mut self, run_id: RunId, conclusion: CheckConclusion) -> Outcome {
        self.fact(
            Actor::System,
            EventKind::CheckObserved {
                test_sha: format!("test-{run_id}"),
                check: CheckReport {
                    name: REQUIRED.into(),
                    kind: CheckKind::Check,
                    conclusion: Some(conclusion),
                    details_url: None,
                },
            },
        )
    }

    /// Merges every PR of the desired state, as the merge job does.
    pub fn merge_all(&mut self) -> usize {
        let merges = self.reconcile().desired.merges;
        let mut count = 0;
        for merge in &merges {
            self.target += 1;
            let sha = format!("target-{}", self.target);
            let outcome = self.fact(
                Actor::Bot,
                EventKind::MergeSucceeded {
                    item_id: merge.item_id,
                    merged_sha: sha.clone(),
                },
            );
            if outcome == Outcome::Applied {
                count += 1;
                self.push_target_to(Actor::Bot, sha);
            }
        }
        count
    }

    /// The CI: a run fails when it tests a bad PR, as a member or as a
    /// predecessor. Each round assembles, reports and merges, until no item
    /// is live; a round without a run moves the clock by one minute.
    pub fn drain(&mut self, bad: &BTreeSet<u64>) {
        self.drain_with(|_, prs| prs.iter().any(|pr| bad.contains(pr)));
    }

    /// The CI, with `fails` that receives the number of the run and the PRs
    /// that it tests.
    pub fn drain_with(&mut self, mut fails: impl FnMut(usize, &[u64]) -> bool) {
        let mut count = 0;
        for _ in 0..10_000 {
            self.assemble_all();
            let runs: Vec<RunId> = self
                .snapshot
                .runs
                .values()
                .filter(|r| r.status == RunStatus::InProgress)
                .map(|r| r.id)
                .collect();
            for run_id in &runs {
                let Some(run) = self.snapshot.runs.get(run_id) else {
                    continue;
                };
                if run.status != RunStatus::InProgress {
                    continue;
                }
                let group = &self.snapshot.groups[&run.group_id];
                let tested: Vec<u64> = group
                    .predecessor_ids
                    .iter()
                    .chain(&group.item_ids)
                    .filter_map(|id| self.snapshot.items.get(id))
                    .map(|item| item.pr_number)
                    .collect();
                count += 1;
                let conclusion = if fails(count, &tested) {
                    CheckConclusion::Failure
                } else {
                    CheckConclusion::Success
                };
                self.report_run(*run_id, conclusion);
            }
            self.merge_all();
            if self.snapshot.items.values().all(|i| i.state.is_terminal()) {
                break;
            }
            if runs.is_empty() {
                self.advance(1);
            }
        }
        assert!(
            self.snapshot.items.values().all(|i| i.state.is_terminal()),
            "the queue did not drain"
        );
    }

    /// How each PR ended, from the log: concluded items leave the snapshot.
    pub fn ended(&self) -> std::collections::BTreeMap<u64, ItemState> {
        let mut prs = std::collections::BTreeMap::new();
        let mut items = std::collections::BTreeMap::new();
        for event in &self.log {
            match &event.kind {
                EventKind::ItemSubmitted { pr_number, .. } => {
                    prs.insert(event.id, *pr_number);
                }
                EventKind::ItemTransitioned {
                    item_id,
                    from: None,
                    ..
                } => {
                    if let Some(pr) = event.cause_id.and_then(|c| prs.get(&c)) {
                        items.insert(*item_id, *pr);
                    }
                }
                _ => {}
            }
        }
        let mut ended = std::collections::BTreeMap::new();
        for event in &self.log {
            if let EventKind::ItemTransitioned { item_id, to, .. } = &event.kind
                && to.is_terminal()
                && let Some(pr) = items.get(item_id)
            {
                ended.insert(*pr, *to);
            }
        }
        ended
    }

    /// Replays the log on the initial snapshot.
    pub fn replay(&self) -> Snapshot {
        let mut snapshot = self.initial.clone();
        for event in &self.log {
            assert_eq!(apply(&mut snapshot, event), Ok(Outcome::Applied));
        }
        snapshot
    }
}

pub struct Pr {
    pub state: PrState,
    pub head_sha: String,
    pub base_branch: String,
    pub draft: bool,
    pub mergeability: Mergeability,
    pub unmet_conditions: Vec<String>,
    pub required_checks: Vec<String>,
    pub checks: Vec<CheckReport>,
}

pub fn head(pr_number: u64, version: u32) -> String {
    format!("head-{pr_number}-{version}")
}

/// The log has a decision that moves an item to `to` for `reason`.
pub fn transitioned(w: &World, to: ItemState, reason: Reason) -> bool {
    w.log.iter().any(|e| {
        matches!(e.kind, EventKind::ItemTransitioned { to: t, reason: r, .. } if t == to && r == reason)
    })
}

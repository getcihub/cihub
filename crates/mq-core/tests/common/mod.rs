//! A small world around one queue: it writes facts as GitHub and the jobs
//! would, runs `plan` after each one, and keeps the log.

#![allow(dead_code, reason = "each test file uses a part of the world")]

use std::collections::BTreeSet;

use cihub_config::{Layer, QueueSettings, Setting};
use cihub_domain::{
    Actor, CheckConclusion, CheckKind, CheckReport, Event, EventId, EventKind, Item, ItemState,
    JobError, Mergeability, OrgId, PrState, Priority, QueueId, QueueMode, RequiredStatusesSource,
    ReviewDecision, RunId, RunStatus, SubmittedVia,
};
use cihub_mq_core::{Outcome, Plan, Snapshot, apply, check, plan};
use time::{Duration, OffsetDateTime};
use ulid::Ulid;

pub const REQUIRED: &str = "ci";

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

    pub fn submit(&mut self, pr_number: u64, priority: Priority) -> Outcome {
        let outcome = self.fact(
            Actor::User {
                user_id: cihub_domain::UserId::from_ulid(ulid::Ulid(1)),
            },
            EventKind::ItemSubmitted {
                pr_number,
                head_sha: head(pr_number, 0),
                priority,
                no_batch: false,
                direct: false,
                submitted_via: SubmittedVia::Comment,
                author_id: cihub_domain::UserId::from_ulid(ulid::Ulid(1)),
                additions: 1,
                deletions: 0,
                changed_files: 1,
            },
        );
        self.observe(pr_number, |_| {});
        outcome
    }

    /// Observes the PR: open, mergeable, then `edit`.
    pub fn observe(&mut self, pr_number: u64, edit: impl FnOnce(&mut Pr)) -> Outcome {
        let mut pr = Pr {
            state: PrState::Open,
            head_sha: head(pr_number, 0),
            base_branch: "main".into(),
            draft: false,
            mergeability: Mergeability::Mergeable,
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
                checks: Vec::new(),
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
                sender_id: Some(cihub_domain::UserId::from_ulid(ulid::Ulid(1))),
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
}

pub fn head(pr_number: u64, version: u32) -> String {
    format!("head-{pr_number}-{version}")
}

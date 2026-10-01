//! Tests against Postgres: each `#[sqlx::test]` gets a new database.

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use cihub_config::{Layer, QueueSettings, Setting};
use cihub_domain::{
    Actor, BlockReason, CheckConclusion, CheckKind, CheckReport, ConflictCulprit, ConnectionId,
    Event, EventId, EventKind, ItemState, JobClass, MergeMethod, Mergeability, OrgId, PrState,
    Priority, QueueId, Reason, RepoId, RequiredStatusesSource, ReviewDecision, RunId, RunStatus,
    ScmAccountId, ScmInstanceId, SubmittedVia, UserId, temp_branch,
};
use cihub_github::{GithubApp, GithubConfig};
use cihub_mergequeue::{
    Clock, MergeQueueError, QueueLog, REFRESH_PRS, Reconciler, SNAPSHOT_EVERY, Translate,
    queue_status_context, rebuild, wake_uncovered,
};
use cihub_mq_core::plan;
use cihub_platform::{
    ConnectionBudget, Delivery, Executors, Job, begin_reconciliation, claim_jobs, enqueue_job,
    find_route, mark_dirty, run_budget_writer, run_maintenance, run_worker, store_delivery,
};
use cihub_scm::sim::{Fault, PrChange, Sim};
use cihub_scm::{
    ConnectionRef, PrMerge, RepoPermission, RepoRef, RequiredChecks, Scm, ScmDelivery, ScmError,
    ScmUser,
};
use cihub_store::{begin_system, create_event_partitions, deposit_fact};
use secrecy::SecretString;
use serde_json::Value;
use sqlx::{AssertSqlSafe, PgConnection, PgPool};
use time::{Date, Duration, Month, OffsetDateTime};
use tokio_util::sync::CancellationToken;
use ulid::Ulid;

const REQUIRED: &str = "ci";

fn org() -> OrgId {
    OrgId::from_ulid(Ulid(1))
}

fn queue() -> QueueId {
    QueueId::from_ulid(Ulid(2))
}

fn user() -> UserId {
    UserId::from_ulid(Ulid(3))
}

fn head(pr: u64) -> String {
    format!("head-{pr}")
}

fn instance() -> ScmInstanceId {
    ScmInstanceId::from_ulid(Ulid(10))
}

fn connection() -> ConnectionId {
    ConnectionId::from_ulid(Ulid(11))
}

/// The repository of the queue, as the simulator and the calls name it.
fn repo() -> RepoRef {
    RepoRef {
        connection: ConnectionRef {
            id: connection(),
            external_id: "1".into(),
        },
        external_id: "200".into(),
        namespace: "acme".into(),
        name: "app".into(),
    }
}

/// The time of the tests: a fixed clock, so that no lease expires and no
/// read comes due by itself.
fn clock() -> OffsetDateTime {
    OffsetDateTime::new_utc(
        Date::from_calendar_date(2026, Month::October, 1).unwrap(),
        time::Time::from_hms(10, 0, 0).unwrap(),
    )
}

async fn seed(pool: &PgPool) {
    create_event_partitions(
        pool,
        Date::from_calendar_date(2026, Month::October, 1).unwrap(),
    )
    .await
    .unwrap();
    let mut tx = begin_system(pool).await.unwrap();
    sqlx::raw_sql(AssertSqlSafe(format!(
        "INSERT INTO scm_instances (id, kind, base_url)
             VALUES ('{instance}', 'github', 'https://github.com');
         INSERT INTO users (id, scm_instance_id, external_id, login)
             VALUES ('{user}', '{instance}', '3', 'octocat');
         INSERT INTO orgs (id, slug) VALUES ('{org}', 'acme');
         INSERT INTO org_modules (org_id, module, enabled_at)
             VALUES ('{org}', 'mergequeue', '2026-10-01T00:00:00Z');
         INSERT INTO scm_accounts (id, org_id, scm_instance_id, external_id, path)
             VALUES ('{account}', '{org}', '{instance}', '1', 'acme');
         INSERT INTO scm_connections (id, org_id, scm_instance_id, scm_account_id, kind, external_id)
             VALUES ('{connection}', '{org}', '{instance}', '{account}', 'github_app', '1');
         INSERT INTO repos (id, org_id, scm_instance_id, connection_id, external_id, namespace,
                            name, default_branch)
             VALUES ('{repo}', '{org}', '{instance}', '{connection}', '200', 'acme', 'app',
                     'main');
         INSERT INTO queues (id, org_id, repo_id, target_branch)
             VALUES ('{queue}', '{org}', '{repo}', 'main');",
        instance = instance(),
        user = user(),
        org = org(),
        account = ScmAccountId::from_ulid(Ulid(13)),
        connection = connection(),
        repo = RepoId::from_ulid(Ulid(12)),
        queue = queue(),
    )))
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
}

/// One queue on Postgres: each fact is written, then reconciled, in its own
/// transaction, as the scheduler does.
struct World {
    pool: PgPool,
    log: QueueLog,
    settings: QueueSettings,
    now: OffsetDateTime,
    facts: u64,
    target: u32,
}

impl World {
    async fn new(pool: PgPool) -> Self {
        Self::at(pool, clock()).await
    }

    /// A world whose first fact comes just after `start`.
    async fn at(pool: PgPool, start: OffsetDateTime) -> Self {
        seed(&pool).await;
        create_event_partitions(&pool, start.date()).await.unwrap();
        let ui = Layer {
            failure_budget_retries: Some(Setting::new(0)),
            ..Layer::default()
        };
        let mut tx = begin_system(&pool).await.unwrap();
        let log = QueueLog::load(&mut tx, queue()).await.unwrap();
        tx.commit().await.unwrap();
        let mut world = Self {
            pool,
            log,
            settings: QueueSettings::resolve(&ui, &Layer::default(), &Layer::default()).unwrap(),
            now: start,
            facts: 0,
            target: 0,
        };
        world.push_target(Actor::System, None).await;
        world
    }

    /// Writes a fact, then the decisions of `plan`, and returns the events
    /// written.
    async fn fact(&mut self, actor: Actor, kind: EventKind) -> Vec<Event> {
        self.facts += 1;
        self.now += Duration::seconds(1);
        let ms = u64::try_from(self.now.unix_timestamp_nanos() / 1_000_000).unwrap();
        let event = Event {
            id: EventId::from_ulid(Ulid::from_parts(ms, u128::from(self.facts))),
            org_id: org(),
            queue_id: queue(),
            actor,
            occurred_at: self.now,
            cause_id: None,
            kind,
        };
        let mut tx = begin_system(&self.pool).await.unwrap();
        let mut written = self
            .log
            .write(&mut tx, vec![event], self.now)
            .await
            .unwrap();
        let plan = plan(&self.log.snapshot, &self.settings, self.now).unwrap();
        written.extend(
            self.log
                .write(&mut tx, plan.decisions, self.now)
                .await
                .unwrap(),
        );
        tx.commit().await.unwrap();
        written
    }

    async fn push_target(&mut self, actor: Actor, after: Option<String>) {
        let before = self
            .log
            .snapshot
            .target
            .head
            .as_ref()
            .map(|h| h.value.clone());
        let after = after.unwrap_or_else(|| {
            self.target += 1;
            format!("target-{}", self.target)
        });
        self.fact(
            actor,
            EventKind::TargetPushed {
                before,
                after,
                sender_id: user(),
            },
        )
        .await;
    }

    async fn submit(&mut self, pr: u64) {
        self.fact(
            Actor::User { user_id: user() },
            EventKind::ItemSubmitted {
                pr_number: pr,
                head_sha: head(pr),
                priority: Priority::MEDIUM,
                no_batch: false,
                direct: false,
                submitted_via: SubmittedVia::Comment,
                author_id: user(),
                additions: 1,
                deletions: 0,
                changed_files: 1,
            },
        )
        .await;
        self.observe(pr, PrState::Open).await;
    }

    async fn observe(&mut self, pr: u64, state: PrState) -> Vec<Event> {
        self.fact(
            Actor::System,
            EventKind::PrObserved {
                pr_number: pr,
                state,
                head_sha: head(pr),
                base_branch: "main".into(),
                draft: false,
                labels: BTreeSet::new(),
                mergeability: Mergeability::Mergeable,
                review_decision: ReviewDecision::Approved,
                checks: Vec::new(),
            },
        )
        .await
    }

    fn runs(&self, status: RunStatus) -> Vec<RunId> {
        let runs = self.log.snapshot.runs.values();
        runs.filter(|r| r.status == status).map(|r| r.id).collect()
    }

    async fn assemble_all(&mut self) {
        for run_id in self.runs(RunStatus::Assembling) {
            self.fact(
                Actor::System,
                EventKind::TestBranchAssembled {
                    run_id,
                    test_sha: format!("test-{run_id}"),
                    draft_pr_number: Some(9000),
                    required_statuses: vec![REQUIRED.into()],
                    required_statuses_source: RequiredStatusesSource::RepoConfig,
                },
            )
            .await;
        }
    }

    async fn report_all(&mut self, conclusion: CheckConclusion) {
        for run_id in self.runs(RunStatus::InProgress) {
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
            .await;
        }
    }

    /// Merges the PRs of the desired state, as the merge job does.
    async fn merge_all(&mut self) {
        let merges = plan(&self.log.snapshot, &self.settings, self.now)
            .unwrap()
            .desired
            .merges;
        for merge in merges {
            self.target += 1;
            let sha = format!("target-{}", self.target);
            let kind = EventKind::MergeSucceeded {
                item_id: merge.item_id,
                merged_sha: sha.clone(),
            };
            self.fact(Actor::Bot, kind).await;
            self.push_target(Actor::Bot, Some(sha)).await;
        }
    }
}

/// Every projection of `queue`, as JSON, in a stable order.
async fn projections(conn: &mut PgConnection) -> Vec<Value> {
    let mut rows = Vec::new();
    for query in [
        "SELECT to_jsonb(t) FROM queue_items t WHERE queue_id = $1 ORDER BY id",
        "SELECT to_jsonb(t) FROM test_groups t WHERE queue_id = $1 ORDER BY id",
        "SELECT to_jsonb(t) FROM test_runs t WHERE queue_id = $1 ORDER BY id",
        "SELECT to_jsonb(c) FROM check_observations c
         WHERE run_id IN (SELECT id FROM test_runs WHERE queue_id = $1)
         ORDER BY run_id, name, kind",
        "SELECT jsonb_build_object('state', state, 'mode', mode) FROM queues WHERE id = $1",
    ] {
        let found: Vec<Value> = sqlx::query_scalar(query)
            .bind(queue().to_string())
            .fetch_all(&mut *conn)
            .await
            .unwrap();
        rows.extend(found);
    }
    rows
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_replay_of_the_log_rebuilds_the_same_projections(pool: PgPool) {
    let mut w = World::new(pool.clone()).await;
    for pr in 1..=110 {
        w.submit(pr).await;
        if pr % 7 == 0 {
            w.observe(pr, PrState::Closed).await;
            continue;
        }
        w.assemble_all().await;
        let conclusion = if pr % 10 == 0 {
            CheckConclusion::Failure
        } else {
            CheckConclusion::Success
        };
        w.report_all(conclusion).await;
        w.merge_all().await;
    }
    assert!(w.log.seq > SNAPSHOT_EVERY, "{} events only", w.log.seq);

    let mut tx = begin_system(&pool).await.unwrap();
    let states: Vec<(String, i64)> = sqlx::query_as(
        "SELECT state::text, count(*) FROM queue_items WHERE queue_id = $1
         GROUP BY state ORDER BY state",
    )
    .bind(queue().to_string())
    .fetch_all(&mut *tx)
    .await
    .unwrap();
    assert_eq!(
        states,
        [
            ("cancelled".into(), 15),
            ("failed".into(), 10),
            ("merged".into(), 85)
        ]
    );
    let snapshots: Vec<i64> = sqlx::query_scalar("SELECT seq FROM queue_snapshots ORDER BY seq")
        .fetch_all(&mut *tx)
        .await
        .unwrap();
    assert_eq!(snapshots[0], SNAPSHOT_EVERY);

    // The log read back from its last snapshot is the log in memory.
    let before = projections(&mut tx).await;
    let loaded = QueueLog::load(&mut tx, queue()).await.unwrap();
    assert_eq!(loaded, w.log);

    // The replay of the whole log writes the same rows and snapshots.
    let rebuilt = rebuild(&mut tx, queue(), w.now).await.unwrap();
    assert_eq!(rebuilt, w.log);
    assert_eq!(projections(&mut tx).await, before);
    let again: Vec<i64> = sqlx::query_scalar("SELECT seq FROM queue_snapshots ORDER BY seq")
        .fetch_all(&mut *tx)
        .await
        .unwrap();
    assert_eq!(again, snapshots);
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_fact_that_changes_nothing_is_not_written(pool: PgPool) {
    let mut w = World::new(pool.clone()).await;
    w.submit(1).await;
    let seq = w.log.seq;
    assert!(w.observe(1, PrState::Open).await.is_empty());
    assert_eq!(w.log.seq, seq);

    let mut tx = begin_system(&pool).await.unwrap();
    let item: (String, String, i64, Option<i64>, String, Option<String>) = sqlx::query_as(
        "SELECT state::text, state_reason::text, seq, position, pr_author_id, submitted_by
         FROM queue_items WHERE queue_id = $1",
    )
    .bind(queue().to_string())
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(item.0, ItemState::Testing.as_str());
    assert_eq!(item.1, "test_run_started");
    assert!(item.2 > 0);
    assert!(item.3.is_some());
    assert_eq!(item.4, user().to_string());
    assert_eq!(item.5, Some(user().to_string()));
}

/// The clock of the loops of a [`SimQueue`]: the fixed time of [`clock`],
/// plus the time elapsed since the start of the test, so that a job retried
/// after its backoff comes due, plus the minutes that a test adds to it.
#[derive(Clone)]
struct TestClock {
    started: std::time::Instant,
    added_minutes: Arc<AtomicI64>,
}

impl TestClock {
    fn new() -> Self {
        #[expect(
            clippy::disallowed_methods,
            reason = "the one place where the simulator tests read the clock"
        )]
        let started = std::time::Instant::now();
        Self {
            started,
            added_minutes: Arc::default(),
        }
    }

    fn now(&self) -> OffsetDateTime {
        clock()
            + self.started.elapsed()
            + Duration::minutes(self.added_minutes.load(Ordering::SeqCst))
    }

    fn advance(&self, minutes: i64) {
        self.added_minutes.fetch_add(minutes, Ordering::SeqCst);
    }

    fn shared(&self) -> Clock {
        let clock = self.clone();
        Arc::new(move || clock.now())
    }
}

/// The queue of the tests on the simulator, with the scheduler and the
/// worker of the server running against it.
struct SimQueue {
    pool: PgPool,
    sim: Arc<Sim>,
    clock: TestClock,
    deliveries: Arc<Mutex<HashMap<String, ScmDelivery>>>,
    sent: usize,
    cancel: CancellationToken,
    loops: Vec<tokio::task::JoinHandle<()>>,
}

fn alice() -> ScmUser {
    ScmUser {
        external_id: "21".into(),
        login: "alice".into(),
    }
}

fn bob() -> ScmUser {
    ScmUser {
        external_id: "22".into(),
        login: "bob".into(),
    }
}

/// The translation of a stored delivery of the simulator: its payload names
/// it.
fn translate(deliveries: &Arc<Mutex<HashMap<String, ScmDelivery>>>) -> Translate {
    let deliveries = deliveries.clone();
    Arc::new(move |_, payload| {
        let id = payload["id"].as_str().unwrap_or_default();
        deliveries
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or_else(|| format!("unknown delivery {id}").into())
    })
}

impl SimQueue {
    async fn new(pool: PgPool) -> Self {
        seed(&pool).await;
        let sim = Arc::new(Sim::new());
        let clock = TestClock::new();
        let deliveries = Arc::new(Mutex::new(HashMap::new()));
        let scm: Arc<dyn Scm> = sim.clone();
        let module = cihub_mergequeue::module(
            pool.clone(),
            scm,
            translate(&deliveries),
            "scheduler".into(),
            clock.shared(),
        )
        .unwrap();
        let mut executors = Executors::default();
        (module.executors)(&mut executors);
        let cancel = CancellationToken::new();
        let worker_clock = clock.clone();
        let worker = run_worker(
            pool.clone(),
            "worker".into(),
            Arc::new(executors),
            move || worker_clock.now(),
            std::time::Duration::from_millis(50),
            cancel.clone(),
        );
        let scheduler = (module.scheduler)(cancel.clone());
        let loops = vec![
            tokio::spawn(async move { worker.await.unwrap() }),
            tokio::spawn(async move { scheduler.await.unwrap() }),
        ];
        Self {
            pool,
            sim,
            clock,
            deliveries,
            sent: 0,
            cancel,
            loops,
        }
    }

    /// Stores the deliveries of the simulator, routed as the web role does.
    async fn deliver(&mut self) {
        let mut tx = begin_system(&self.pool).await.unwrap();
        for delivery in self.sim.take_deliveries() {
            self.sent += 1;
            let id = format!("delivery-{}", self.sent);
            let route = find_route(&mut tx, instance(), &delivery).await.unwrap();
            self.deliveries.lock().unwrap().insert(id.clone(), delivery);
            let stored = Delivery {
                scm_instance_id: instance(),
                delivery_id: id.clone(),
                org_id: route.org_id,
                connection_id: route.connection_id,
                queue_id: route.queue_id,
                event: "sim".into(),
                action: None,
                payload: serde_json::json!({ "id": id }),
                received_at: self.clock.now(),
            };
            store_delivery(&mut tx, &stored).await.unwrap();
        }
        tx.commit().await.unwrap();
    }

    /// Wakes the queue, as a change would: after a move of the clock.
    async fn wake(&self) {
        let mut tx = begin_system(&self.pool).await.unwrap();
        mark_dirty(&mut tx, queue()).await.unwrap();
        tx.commit().await.unwrap();
    }

    /// Merges the PRs of the desired state, as the merge job of lot 7 will:
    /// the merge on the simulator, then its fact. Returns their number.
    async fn merge_desired(&self) -> usize {
        let mut tx = begin_system(&self.pool).await.unwrap();
        let log = QueueLog::load(&mut tx, queue()).await.unwrap();
        tx.commit().await.unwrap();
        let settings = cihub_mergequeue::default_settings().unwrap();
        let merges = plan(&log.snapshot, &settings, self.clock.now())
            .unwrap()
            .desired
            .merges;
        for merge in &merges {
            let request = PrMerge {
                number: merge.pr_number,
                head_sha: merge.head_sha.clone(),
                method: MergeMethod::Squash,
                title: format!("PR {}", merge.pr_number),
                message: String::new(),
            };
            let merged_sha = self
                .sim
                .merge_pr(&repo(), &request, JobClass::Merge)
                .await
                .unwrap();
            let fact = Event {
                id: EventId::from_ulid(Ulid::from_parts(
                    u64::try_from(self.clock.now().unix_timestamp()).unwrap(),
                    u128::from(merge.pr_number) + 1_000_000,
                )),
                org_id: org(),
                queue_id: queue(),
                actor: Actor::Bot,
                occurred_at: self.clock.now(),
                cause_id: None,
                kind: EventKind::MergeSucceeded {
                    item_id: merge.item_id,
                    merged_sha,
                },
            };
            let mut tx = begin_system(&self.pool).await.unwrap();
            deposit_fact(&mut tx, &fact, self.clock.now())
                .await
                .unwrap();
            mark_dirty(&mut tx, queue()).await.unwrap();
            tx.commit().await.unwrap();
        }
        merges.len()
    }

    /// The runs in progress: identifier, test SHA and PRs tested.
    async fn runs_in_progress(&self) -> Vec<(RunId, String, Vec<u64>)> {
        let mut tx = begin_system(&self.pool).await.unwrap();
        let rows: Vec<(String, String, Vec<i64>)> = sqlx::query_as(
            "SELECT id, test_sha, tested_pr_numbers FROM test_runs
             WHERE status = 'in_progress' ORDER BY started_at, id",
        )
        .fetch_all(&mut *tx)
        .await
        .unwrap();
        rows.into_iter()
            .map(|(id, sha, prs)| {
                let prs = prs.into_iter().map(|n| u64::try_from(n).unwrap()).collect();
                (id.parse().unwrap(), sha, prs)
            })
            .collect()
    }

    /// Waits until every delivery, fact and job is done.
    async fn settle(&self) {
        for _ in 0..800 {
            let mut tx = begin_system(&self.pool).await.unwrap();
            let busy: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM webhook_inbox WHERE processed_at IS NULL)
                     OR EXISTS (SELECT 1 FROM queue_inbox WHERE processed_at IS NULL)
                     OR EXISTS (SELECT 1 FROM jobs WHERE state IN ('queued', 'running'))
                     OR EXISTS (SELECT 1 FROM queues WHERE dirty)",
            )
            .fetch_one(&mut *tx)
            .await
            .unwrap();
            if !busy {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        let mut tx = begin_system(&self.pool).await.unwrap();
        let left: Vec<(String, String)> = sqlx::query_as(
            "SELECT 'delivery', delivery_id FROM webhook_inbox WHERE processed_at IS NULL
             UNION ALL SELECT 'fact', type::text FROM queue_inbox WHERE processed_at IS NULL
             UNION ALL SELECT 'job', idempotency_key || ' ' || state::text || ' '
                          || coalesce(last_error, '') FROM jobs WHERE state IN ('queued', 'running')
             UNION ALL SELECT 'dirty', id FROM queues WHERE dirty",
        )
        .fetch_all(&mut *tx)
        .await
        .unwrap();
        panic!("the queue did not settle: {left:?}");
    }

    async fn stop(self) {
        self.cancel.cancel();
        for running in self.loops {
            running.await.unwrap();
        }
    }
}

/// What a test does to a PR of the simulator before its submission.
type Setup = Box<dyn Fn(&Sim, u64, &str)>;

/// Opens the PR of `branch` from bob, with a file of its own.
fn open_pr(sim: &Sim, root: &str, n: u64) -> (u64, String) {
    let branch = format!("pr-{n}");
    let path = format!("f{n}.txt");
    let head = sim.commit(&repo(), root, &[(&path, Some("x"))]).unwrap();
    sim.push(&repo(), &branch, &head, &bob()).unwrap();
    (sim.open_pr(&repo(), &branch, "main", &bob()).unwrap(), head)
}

fn check(name: &str, passed: bool) -> CheckReport {
    CheckReport {
        name: name.into(),
        kind: CheckKind::Status,
        conclusion: Some(if passed {
            CheckConclusion::Success
        } else {
            CheckConclusion::Failure
        }),
        details_url: None,
    }
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn twenty_prs_reach_pending_or_not_ready_with_their_reason(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let root = sim.add_repo(&repo(), "main", &[("a.txt", "a")]);
    let own = queue_status_context("main");
    let rules = RequiredChecks {
        names: BTreeSet::from([REQUIRED.to_owned(), own]),
        up_to_date: false,
    };
    sim.set_rules(&repo(), "main", rules).unwrap();
    sim.set_permission(&repo(), &alice(), RepoPermission::Write)
        .unwrap();

    // What each PR is, and what the queue must say of it.
    let ready = |_: &Sim, _: u64, _: &str| {};
    let failing = |sim: &Sim, _: u64, head: &str| {
        sim.report_check(&repo(), head, check(REQUIRED, false))
            .unwrap();
    };
    let review = |decision| {
        move |sim: &Sim, n: u64, _: &str| {
            sim.change_pr(&repo(), n, PrChange::Review(decision))
                .unwrap();
        }
    };
    let forced = |mergeability| {
        move |sim: &Sim, n: u64, _: &str| {
            let change = PrChange::Mergeability(Some(mergeability));
            sim.change_pr(&repo(), n, change).unwrap();
        }
    };
    let cases: Vec<(Setup, ItemState, Vec<BlockReason>)> = vec![
        (Box::new(ready), ItemState::Pending, vec![]),
        (Box::new(ready), ItemState::Pending, vec![]),
        (Box::new(ready), ItemState::Pending, vec![]),
        (Box::new(ready), ItemState::Pending, vec![]),
        (Box::new(ready), ItemState::Pending, vec![]),
        (Box::new(ready), ItemState::Pending, vec![]),
        (
            Box::new(failing),
            ItemState::NotReady,
            vec![BlockReason::Undetermined],
        ),
        (
            Box::new(failing),
            ItemState::NotReady,
            vec![BlockReason::Undetermined],
        ),
        (
            Box::new(review(ReviewDecision::ChangesRequested)),
            ItemState::NotReady,
            vec![BlockReason::ChangesRequested],
        ),
        (
            Box::new(review(ReviewDecision::ChangesRequested)),
            ItemState::NotReady,
            vec![BlockReason::ChangesRequested],
        ),
        (
            Box::new(review(ReviewDecision::ReviewRequired)),
            ItemState::NotReady,
            vec![BlockReason::Undetermined],
        ),
        (
            Box::new(review(ReviewDecision::ReviewRequired)),
            ItemState::NotReady,
            vec![BlockReason::Undetermined],
        ),
        (
            Box::new(forced(Mergeability::Conflict)),
            ItemState::NotReady,
            vec![BlockReason::MergeConflict],
        ),
        (
            Box::new(forced(Mergeability::Conflict)),
            ItemState::NotReady,
            vec![BlockReason::MergeConflict],
        ),
        (
            Box::new(forced(Mergeability::Behind)),
            ItemState::NotReady,
            vec![BlockReason::BehindBase],
        ),
        (
            Box::new(forced(Mergeability::Behind)),
            ItemState::NotReady,
            vec![BlockReason::BehindBase],
        ),
        (
            Box::new(forced(Mergeability::Unknown)),
            ItemState::NotReady,
            vec![BlockReason::Undetermined],
        ),
        (
            Box::new(forced(Mergeability::Unknown)),
            ItemState::NotReady,
            vec![BlockReason::Undetermined],
        ),
    ];
    let mut want = BTreeMap::new();
    for (n, (setup, state, blocks)) in (1..).zip(&cases) {
        let (number, head) = open_pr(&sim, &root, n);
        sim.report_check(&repo(), &head, check(REQUIRED, true))
            .unwrap();
        setup(&sim, number, &head);
        sim.comment(&repo(), number, &alice(), "LGTM\n/cihub merge")
            .unwrap();
        want.insert(number, (*state, blocks.clone()));
    }
    // Two PRs by the label, ready.
    for n in 19..=20 {
        let (number, head) = open_pr(&sim, &root, n);
        sim.report_check(&repo(), &head, check(REQUIRED, true))
            .unwrap();
        sim.add_label(
            &repo(),
            number,
            "cihub-merge-queue-submit",
            JobClass::Interactive,
        )
        .await
        .unwrap();
        want.insert(number, (ItemState::Pending, vec![]));
    }
    // A command of an account that cannot write submits nothing.
    let (refused, _) = open_pr(&sim, &root, 21);
    sim.comment(&repo(), refused, &bob(), "/cihub merge")
        .unwrap();

    q.deliver().await;
    q.settle().await;

    let mut tx = begin_system(&pool).await.unwrap();
    let rows: Vec<(i64, String, Value, String)> = sqlx::query_as(
        "SELECT pr_number, state::text, block_reasons, submitted_via::text FROM queue_items
         WHERE queue_id = $1 ORDER BY pr_number",
    )
    .bind(queue().to_string())
    .fetch_all(&mut *tx)
    .await
    .unwrap();
    let got: BTreeMap<u64, (ItemState, Vec<BlockReason>)> = rows
        .iter()
        .map(|(n, state, blocks, _)| {
            let state = state.parse().unwrap();
            let blocks = serde_json::from_value(blocks.clone()).unwrap();
            (u64::try_from(*n).unwrap(), (state, blocks))
        })
        .collect();
    assert_eq!(got, want);
    let vias: Vec<&str> = rows.iter().map(|r| r.3.as_str()).collect();
    assert_eq!(vias.iter().filter(|v| **v == "label").count(), 2);
    let reasons: Vec<String> =
        sqlx::query_scalar("SELECT DISTINCT state_reason::text FROM queue_items ORDER BY 1")
            .fetch_all(&mut *tx)
            .await
            .unwrap();
    assert_eq!(reasons, ["merge_item_now_ready", "submitted"]);
    drop(tx);

    // The replay of the log gives the same items.
    let mut tx = begin_system(&pool).await.unwrap();
    let before = projections(&mut tx).await;
    rebuild(&mut tx, queue(), clock()).await.unwrap();
    assert_eq!(projections(&mut tx).await, before);
    drop(tx);

    // The connection loses its access: the queue pauses (AGENTS.md D103).
    sim.set_access("1", false);
    q.deliver().await;
    q.settle().await;
    let mut tx = begin_system(&pool).await.unwrap();
    let state: String = sqlx::query_scalar("SELECT state::text FROM queues WHERE id = $1")
        .bind(queue().to_string())
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(state, "paused");
    drop(tx);
    q.stop().await;
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_reconciliation_stopped_before_its_commit_writes_nothing(pool: PgPool) {
    seed(&pool).await;
    let sim = Sim::new();
    let deliveries = Arc::new(Mutex::new(HashMap::new()));
    let root = sim.add_repo(&repo(), "main", &[("a.txt", "a")]);
    let (number, _) = open_pr(&sim, &root, 1);
    sim.take_deliveries();
    sim.add_label(
        &repo(),
        number,
        "cihub-merge-queue-submit",
        JobClass::Interactive,
    )
    .await
    .unwrap();
    let mut tx = begin_system(&pool).await.unwrap();
    for (n, delivery) in sim.take_deliveries().into_iter().enumerate() {
        let id = format!("delivery-{n}");
        let route = find_route(&mut tx, instance(), &delivery).await.unwrap();
        deliveries.lock().unwrap().insert(id.clone(), delivery);
        let stored = Delivery {
            scm_instance_id: instance(),
            delivery_id: id.clone(),
            org_id: route.org_id,
            connection_id: route.connection_id,
            queue_id: route.queue_id,
            event: "sim".into(),
            action: None,
            payload: serde_json::json!({ "id": id }),
            received_at: clock(),
        };
        store_delivery(&mut tx, &stored).await.unwrap();
    }
    // A fact given back by a job, and the push of the target.
    let fact = Event {
        id: EventId::from_ulid(Ulid(500)),
        org_id: org(),
        queue_id: queue(),
        actor: Actor::System,
        occurred_at: clock(),
        cause_id: None,
        kind: EventKind::TargetPushed {
            before: None,
            after: root.clone(),
            sender_id: user(),
        },
    };
    cihub_store::deposit_fact(&mut tx, &fact, clock())
        .await
        .unwrap();
    cihub_platform::mark_dirty(&mut tx, queue()).await.unwrap();
    tx.commit().await.unwrap();

    let reconciler = Reconciler {
        settings: Arc::new(cihub_mergequeue::default_settings().unwrap()),
        translate: translate(&deliveries),
    };
    let counts = |pool: PgPool| async move {
        let mut tx = begin_system(&pool).await.unwrap();
        let counts: (i64, i64, i64, i64, bool) = sqlx::query_as(
            "SELECT (SELECT count(*) FROM events),
                    (SELECT count(*) FROM jobs),
                    (SELECT count(*) FROM webhook_inbox WHERE processed_at IS NULL),
                    (SELECT count(*) FROM queue_inbox WHERE processed_at IS NULL),
                    (SELECT dirty FROM queues)",
        )
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        counts
    };
    let before = counts(pool.clone()).await;
    assert_eq!(before, (0, 0, 1, 1, true));

    // The process stops before the commit: the transaction rolls back.
    let mut tx = begin_reconciliation(&pool, queue()).await.unwrap().unwrap();
    reconciler
        .reconcile_in(&mut tx, queue(), clock())
        .await
        .unwrap();
    drop(tx);
    assert_eq!(counts(pool.clone()).await, before);

    // The next reconciliation does the whole work once.
    reconciler.reconcile(&pool, queue(), clock()).await.unwrap();
    let (events, jobs, deliveries_left, facts_left, dirty) = counts(pool.clone()).await;
    assert_eq!(
        (events, jobs, deliveries_left, facts_left, dirty),
        (1, 1, 0, 0, false)
    );
    let mut tx = begin_system(&pool).await.unwrap();
    let key: String = sqlx::query_scalar("SELECT idempotency_key FROM jobs")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(key, "submit_pr:delivery-0:1");
}

/// A GitHub that gives an installation token, then answers every call with
/// a rate limit spent until `reset`.
async fn rate_limited_github(reset: OffsetDateTime) -> String {
    use axum::body::Body;
    use axum::http::{Response, StatusCode, Uri};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let router = axum::Router::new().fallback(move |uri: Uri| async move {
        let response = Response::builder().header("content-type", "application/json");
        if uri.path().ends_with("/access_tokens") {
            let expires_at = (reset + Duration::hours(1))
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap();
            let body = serde_json::json!({ "token": "test", "expires_at": expires_at });
            return response
                .status(StatusCode::CREATED)
                .body(Body::from(body.to_string()))
                .unwrap();
        }
        response
            .status(StatusCode::TOO_MANY_REQUESTS)
            .header("x-ratelimit-limit", "5000")
            .header("x-ratelimit-remaining", "0")
            .header("x-ratelimit-reset", reset.unix_timestamp().to_string())
            .body(Body::from(r#"{"message":"API rate limit exceeded"}"#))
            .unwrap()
    });
    tokio::spawn(async move { axum::serve(listener, router).await });
    url
}

/// The budgets that changed in `app`, as `scm_connections` keeps them; the
/// same mapping as `cihub-server`.
fn changed_budgets(app: &GithubApp) -> Vec<(ConnectionId, ConnectionBudget)> {
    app.take_changed_budgets()
        .into_iter()
        .map(|(connection, budget)| {
            let rate = budget.lowest();
            let recorded = ConnectionBudget {
                mode: budget.mode,
                mode_reason: budget.mode_reason,
                rate_remaining: rate.map(|rate| rate.remaining),
                rate_limit: rate.map(|rate| rate.limit),
                rate_reset_at: rate.map(|rate| rate.reset_at),
                paused_until: budget.paused_until,
                consecutive_failures: budget.failures,
            };
            (connection, recorded)
        })
        .collect()
}

fn read_job(n: u64) -> Job {
    Job {
        idempotency_key: format!("refresh_prs:{}-test-{n}:1", queue()),
        job_type: REFRESH_PRS.into(),
        class: JobClass::Interactive,
        org_id: Some(org()),
        connection: Some((instance(), connection())),
        queue_id: Some(queue()),
        payload: serde_json::json!({ "prs": [n] }),
        attempts: 0,
    }
}

/// The lot 4 chain, end to end (`AGENTS.md` D70, D77): an executor calls
/// GitHub through the connection, GitHub answers 429, the pause reaches
/// `scm_connections`, and no job of the connection goes out before the reset.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_rate_limit_pauses_the_jobs_of_the_connection_until_its_reset(pool: PgPool) {
    seed(&pool).await;
    let now = clock();
    let reset = now + Duration::minutes(15);
    let config = GithubConfig {
        url: rate_limited_github(reset).await,
        app_id: "1".into(),
        private_key: SecretString::from(include_str!("../../github/tests/test-only-key.pem")),
    };
    let app = Arc::new(GithubApp::new(&config, Arc::new(clock)).unwrap());
    let scm: Arc<dyn Scm> = app.clone();
    let module = cihub_mergequeue::module(
        pool.clone(),
        scm,
        translate(&Arc::default()),
        "scheduler".into(),
        Arc::new(clock),
    )
    .unwrap();
    let mut executors = Executors::default();
    (module.executors)(&mut executors);

    let mut tx = begin_system(&pool).await.unwrap();
    enqueue_job(&mut tx, &read_job(1), now, now).await.unwrap();
    tx.commit().await.unwrap();

    let cancel = CancellationToken::new();
    let urgent = app.budget_urgent();
    let budgets = app.clone();
    let writer = tokio::spawn(run_budget_writer(
        pool.clone(),
        move || changed_budgets(&budgets),
        urgent,
        cancel.clone(),
    ));
    let worker = tokio::spawn(run_worker(
        pool.clone(),
        "worker".into(),
        Arc::new(executors),
        clock,
        std::time::Duration::from_millis(50),
        cancel.clone(),
    ));

    // The pause reaches the table at once, without the period of the writer.
    let mut paused_until = None;
    for _ in 0..200 {
        let mut tx = begin_system(&pool).await.unwrap();
        paused_until = sqlx::query_scalar("SELECT paused_until FROM scm_connections")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        if paused_until.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(paused_until, Some(reset));
    cancel.cancel();
    worker.await.unwrap().unwrap();
    writer.await.unwrap();

    // The read waits for its retry, and no job of the connection goes out
    // before the reset; after it, the jobs go out again.
    let mut tx = begin_system(&pool).await.unwrap();
    let (state, error): (String, String) =
        sqlx::query_as("SELECT state::text, last_error FROM jobs")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert_eq!(state, "queued");
    assert!(error.contains("rate limit"), "{error}");
    enqueue_job(&mut tx, &read_job(2), now, now).await.unwrap();
    assert!(
        claim_jobs(&mut tx, "worker", reset - Duration::seconds(1), 10)
            .await
            .unwrap()
            .is_empty()
    );
    let after = claim_jobs(&mut tx, "worker", reset, 10).await.unwrap();
    assert_eq!(after.len(), 2);
}

/// Opens the PR of `branch` from bob, with `files` on `parent`.
fn open_pr_with(sim: &Sim, parent: &str, n: u64, files: &[(&str, Option<&str>)]) -> (u64, String) {
    let branch = format!("pr-{n}");
    let head = sim.commit(&repo(), parent, files).unwrap();
    sim.push(&repo(), &branch, &head, &bob()).unwrap();
    (sim.open_pr(&repo(), &branch, "main", &bob()).unwrap(), head)
}

/// A repository on the simulator whose queue knows the head of `main`,
/// requires `ci`, and lets alice submit. Returns the head of `main`.
fn target_repo(sim: &Sim, files: &[(&str, &str)]) -> String {
    let root = sim.add_repo(&repo(), "main", files);
    let main = sim
        .commit(&repo(), &root, &[("README.md", Some("cihub"))])
        .unwrap();
    sim.push(&repo(), "main", &main, &alice()).unwrap();
    let rules = RequiredChecks {
        names: BTreeSet::from([REQUIRED.to_owned()]),
        up_to_date: false,
    };
    sim.set_rules(&repo(), "main", rules).unwrap();
    sim.set_permission(&repo(), &alice(), RepoPermission::Write)
        .unwrap();
    main
}

/// The rows of `test_runs`, in the order of the runs.
type RunRow = (
    String,
    Option<String>,
    String,
    Option<String>,
    Vec<String>,
    Vec<i64>,
    Vec<String>,
    Option<String>,
    Option<Value>,
    String,
    Option<i64>,
);

async fn test_runs(conn: &mut PgConnection) -> Vec<RunRow> {
    sqlx::query_as(
        "SELECT id, test_sha, status::text, failure_reason::text, head_shas, tested_pr_numbers,
                required_statuses, required_statuses_source::text, conflict_culprit, test_branch,
                draft_pr_number
         FROM test_runs ORDER BY started_at, id",
    )
    .fetch_all(conn)
    .await
    .unwrap()
}

/// The exit criterion of lot 6 (`docs/LOTS.md`): ten PRs, two with a red CI,
/// one in conflict with a PR ahead of it, one whose CI never reports. Each
/// PR is tested on top of its predecessors, each run ends with its reason,
/// the PRs behind a failed one restart without it, and the merge job of lot 7
/// is played by the test.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn ten_prs_are_tested_on_top_of_their_predecessors(pool: PgPool) {
    const FAILING: [u64; 2] = [3, 8];
    const CONFLICTING: u64 = 6;
    const SILENT: u64 = 9;
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let config = format!(
        "version: 1\nqueues:\n  - name: main\n    branch: main\n    required_statuses: [{REQUIRED}]\n"
    );
    let main = target_repo(&sim, &[("a.txt", "a"), (".cihub.yaml", &config)]);
    // The PRs are opened first, so that their numbers are 1 to 10: the draft
    // PRs of the queue come after them.
    let mut heads: HashMap<String, u64> = HashMap::new();
    for n in 1..=10 {
        let path = format!("f{n}.txt");
        let mut files = vec![(path.as_str(), Some("x"))];
        if n == CONFLICTING - 1 {
            files.push(("shared.txt", Some("five")));
        }
        if n == CONFLICTING {
            files.push(("shared.txt", Some("six")));
        }
        let (number, head) = open_pr_with(&sim, &main, n, &files);
        assert_eq!(number, n);
        sim.report_check(&repo(), &head, check(REQUIRED, true))
            .unwrap();
        heads.insert(head, n);
    }
    // One submission at a time: the queue order is the order of the PRs.
    for n in 1..=10 {
        sim.comment(&repo(), n, &alice(), "/cihub merge").unwrap();
        q.deliver().await;
        q.settle().await;
    }

    // The CI of the simulator: each round, the merge job plays, then the CI
    // reports on the runs in progress; the silent PR never reports, and its
    // runs time out when the clock moves.
    let mut rounds = 0;
    loop {
        q.deliver().await;
        q.settle().await;
        let merged = q.merge_desired().await;
        let runs = q.runs_in_progress().await;
        let mut reported = 0;
        for (_, test_sha, prs) in &runs {
            if prs.contains(&SILENT) {
                continue;
            }
            let passed = !prs.iter().any(|pr| FAILING.contains(pr));
            sim.report_check(&repo(), test_sha, check(REQUIRED, passed))
                .unwrap();
            reported += 1;
        }
        if merged + reported == 0 {
            if runs.is_empty() {
                break;
            }
            q.clock.advance(301);
            q.wake().await;
        }
        rounds += 1;
        assert!(rounds < 40, "the queue does not conclude");
    }

    let mut tx = begin_system(&pool).await.unwrap();
    let items: Vec<(i64, String, String, Option<String>)> = sqlx::query_as(
        "SELECT pr_number, state::text, state_reason::text, verified_by_run_id
         FROM queue_items WHERE is_current ORDER BY pr_number",
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap();
    let summary: Vec<(i64, &str, &str)> = items
        .iter()
        .map(|(n, state, reason, _)| (*n, state.as_str(), reason.as_str()))
        .collect();
    assert_eq!(
        summary,
        [
            (1, "merged", "merge_graph_merge"),
            (2, "merged", "merge_graph_merge"),
            (3, "failed", "test_run_required_status_failed"),
            (4, "merged", "merge_graph_merge"),
            (5, "merged", "merge_graph_merge"),
            (6, "failed", "start_test_run_merge_conflict"),
            (7, "merged", "merge_graph_merge"),
            (8, "failed", "test_run_required_status_failed"),
            (9, "failed", "test_run_timeout"),
            (10, "merged", "merge_graph_merge"),
        ]
    );

    let runs = test_runs(&mut tx).await;
    let pr_of = |sha: &String| heads[sha];
    // An assembled run has its required statuses from the file, and its
    // test commit holds the change of every head merged in it.
    for (_, test_sha, _, _, head_shas, _, required, source, ..) in &runs {
        let Some(test_sha) = test_sha else {
            continue;
        };
        assert_eq!(required, &[REQUIRED.to_owned()]);
        assert_eq!(source.as_deref(), Some("repo_config"));
        for sha in head_shas {
            let path = format!("f{}.txt", pr_of(sha));
            let file = sim
                .file(&repo(), test_sha, &path, JobClass::Repair)
                .await
                .unwrap();
            assert!(file.is_some(), "{path} is not in {test_sha}");
        }
    }
    // The run that proved a merged PR tested it on top of merged PRs only.
    let merged: BTreeSet<i64> = summary
        .iter()
        .filter(|(_, state, _)| *state == "merged")
        .map(|(n, ..)| *n)
        .collect();
    for (n, _, _, verified) in &items {
        if !merged.contains(n) {
            continue;
        }
        let verified = verified.as_ref().expect("a merged item has its run");
        let run = runs.iter().find(|r| r.0 == *verified).unwrap();
        let tested: Vec<i64> = run
            .4
            .iter()
            .map(|sha| i64::try_from(pr_of(sha)).unwrap())
            .collect();
        assert!(
            tested.iter().all(|pr| merged.contains(pr)),
            "PR {n} was proved on top of {tested:?}"
        );
        assert_eq!(run.5, vec![*n]);
    }
    // The conflict names the PR ahead that the member conflicts with.
    let conflicts: Vec<&RunRow> = runs
        .iter()
        .filter(|r| r.3.as_deref() == Some("start_test_run_merge_conflict"))
        .collect();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].5, vec![i64::try_from(CONFLICTING).unwrap()]);
    assert_eq!(
        conflicts[0].8,
        Some(
            serde_json::to_value(ConflictCulprit::QueuePr {
                pr_number: CONFLICTING - 1
            })
            .unwrap()
        )
    );
    // The silent PR timed out twice: its run, then the retry of the failure
    // budget; a run restarted behind a failed PR was interrupted before.
    let silent: Vec<&str> = runs
        .iter()
        .filter(|r| r.5 == vec![i64::try_from(SILENT).unwrap()])
        .map(|r| r.2.as_str())
        .collect();
    assert_eq!(silent.iter().filter(|s| **s == "timed_out").count(), 2);
    assert!(
        silent
            .iter()
            .all(|s| matches!(*s, "timed_out" | "interrupted")),
        "{silent:?}"
    );
    // The PRs behind a failed PR restarted without it: at least the PR behind
    // the silent one, tested on top of it until its timeout.
    let restarts: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM events
         WHERE type = 'item.transitioned' AND payload->>'reason' = $1",
    )
    .bind(Reason::PrerequisiteMergeItemFailed.as_str())
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert!(restarts >= 1, "{restarts} restarts");
    // Every report on a test SHA is kept, with the required ones marked: the
    // red CI of the two failing PRs, at least twice each (the budget retries
    // once, and a group restarted behind a failed PR starts a new budget).
    let reports: Vec<(Vec<i64>, String, String)> = sqlx::query_as(
        "SELECT t.tested_pr_numbers, t.status::text, c.conclusion::text
         FROM check_observations c JOIN test_runs t ON t.id = c.run_id
         WHERE c.required AND c.name = $1
         ORDER BY c.conclusion, t.tested_pr_numbers, t.started_at",
    )
    .bind(REQUIRED)
    .fetch_all(&mut *tx)
    .await
    .unwrap();
    let mut failures: BTreeMap<&[i64], usize> = BTreeMap::new();
    for (prs, status, conclusion) in &reports {
        if conclusion == "failure" {
            assert_eq!(status, "failed", "{prs:?}");
            *failures.entry(prs.as_slice()).or_default() += 1;
        }
    }
    let failing: Vec<&[i64]> = failures.keys().copied().collect();
    assert_eq!(failing, [&[3][..], &[8][..]]);
    assert!(failures.values().all(|n| *n >= 2), "{failures:?}");
    let passed = reports.iter().filter(|r| r.2 == "success").count();
    assert!(passed >= 6, "{reports:?}");
    drop(tx);

    // The branches of every run are gone and its draft PR is closed.
    for (_, _, _, _, _, _, _, _, _, test_branch, draft) in &runs {
        assert_eq!(sim.branch(&repo(), test_branch), None, "{test_branch}");
        let temp = temp_branch(test_branch).unwrap();
        assert_eq!(sim.branch(&repo(), &temp), None, "{temp}");
        if let Some(number) = draft {
            let number = u64::try_from(*number).unwrap();
            let pr = sim
                .pull_request(&repo(), number, JobClass::Repair)
                .await
                .unwrap();
            assert_eq!(pr.state, PrState::Closed, "draft PR {number}");
        }
    }

    // A report that comes after the end of a run is stored without a fact.
    let mut tx = begin_system(&pool).await.unwrap();
    let events_before: i64 = sqlx::query_scalar("SELECT count(*) FROM events")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    drop(tx);
    let proved = items[0].3.as_ref().unwrap();
    let run = runs.iter().find(|r| r.0 == *proved).unwrap();
    let test_sha = run.1.as_ref().unwrap();
    q.clock.advance(1);
    sim.report_check(&repo(), test_sha, check("late", true))
        .unwrap();
    q.deliver().await;
    q.settle().await;
    let mut tx = begin_system(&pool).await.unwrap();
    let late: (String, bool) = sqlx::query_as(
        "SELECT conclusion::text, required FROM check_observations WHERE run_id = $1 AND name = 'late'",
    )
    .bind(proved)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(late, ("success".into(), false));
    let events_after: i64 = sqlx::query_scalar("SELECT count(*) FROM events")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(events_after, events_before);

    // The replay of the log gives the same projections, late report included.
    let before = projections(&mut tx).await;
    rebuild(&mut tx, queue(), q.clock.now()).await.unwrap();
    assert_eq!(projections(&mut tx).await, before);
    drop(tx);
    q.stop().await;
}

/// Without a configuration file, the required statuses come from the rules
/// of the target branch; a branch protection that then refuses the test
/// branch fails the run with `start_test_run_permissions_block_test_branch`.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_protected_test_branch_fails_the_run(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let main = target_repo(&sim, &[("a.txt", "a")]);
    let submit = |n: u64| {
        let path = format!("f{n}.txt");
        let (number, head) = open_pr_with(&sim, &main, n, &[(&path, Some("x"))]);
        sim.report_check(&repo(), &head, check(REQUIRED, true))
            .unwrap();
        sim.comment(&repo(), number, &alice(), "/cihub merge")
            .unwrap();
    };
    // The first PR meets the protection; the second, submitted after it,
    // assembles.
    submit(1);
    sim.fail(Fault {
        call: Some("set_branch"),
        error: ScmError::Forbidden("Protected branch update failed".into()),
        times: 1,
    });
    q.deliver().await;
    q.settle().await;
    submit(2);
    q.deliver().await;
    q.settle().await;
    let (_, test_sha, _) = q.runs_in_progress().await.remove(0);
    sim.report_check(&repo(), &test_sha, check(REQUIRED, true))
        .unwrap();
    q.deliver().await;
    q.settle().await;

    let mut tx = begin_system(&pool).await.unwrap();
    let items: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT pr_number, state::text, state_reason::text FROM queue_items WHERE is_current
         ORDER BY pr_number",
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap();
    assert_eq!(
        items,
        [
            (
                1,
                "failed".into(),
                "start_test_run_permissions_block_test_branch".into()
            ),
            (2, "tests_passed".into(), "test_run_passed".into()),
        ]
    );
    let runs = test_runs(&mut tx).await;
    assert_eq!(runs.len(), 2);
    let (_, test_sha, status, reason, ..) = &runs[0];
    assert_eq!((test_sha.as_deref(), status.as_str()), (None, "failed"));
    assert_eq!(
        reason.as_deref(),
        Some("start_test_run_permissions_block_test_branch")
    );
    let (_, test_sha, status, _, _, _, required, source, ..) = &runs[1];
    assert_eq!((test_sha.is_some(), status.as_str()), (true, "succeeded"));
    assert_eq!(required, &[REQUIRED.to_owned()]);
    assert_eq!(source.as_deref(), Some("branch_protection"));
    // Both runs ended: the failed one and the successful one are cleaned.
    let cleaned: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM jobs WHERE type = 'cleanup_test_branch' AND state = 'done'",
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(cleaned, 2);
    drop(tx);
    q.stop().await;
}

async fn partition_exists(pool: &PgPool, name: &str) -> bool {
    let mut tx = begin_system(pool).await.unwrap();
    sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
        .bind(name)
        .fetch_one(&mut *tx)
        .await
        .unwrap()
}

/// `AGENTS.md` D106: an old partition goes only once a snapshot covers its
/// events, and the log of the queue goes on from the snapshot.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn an_old_partition_goes_once_a_snapshot_covers_it(pool: PgPool) {
    let may = OffsetDateTime::new_utc(
        Date::from_calendar_date(2026, Month::May, 10).unwrap(),
        time::Time::from_hms(10, 0, 0).unwrap(),
    );
    let mut w = World::at(pool.clone(), may).await;
    w.submit(1).await;
    assert!(w.log.seq < SNAPSHOT_EVERY, "no snapshot by count");
    let today = clock();

    // No snapshot covers the events of May: the partition stays.
    run_maintenance(&pool, today).await.unwrap();
    assert!(partition_exists(&pool, "events_2026_05").await);

    // The pass of the scheduler wakes the queue, whose reconciliation keeps a
    // snapshot; then the partition goes.
    assert_eq!(wake_uncovered(&pool, today).await.unwrap(), 1);
    let reconciler = Reconciler {
        settings: Arc::new(cihub_mergequeue::default_settings().unwrap()),
        translate: translate(&Arc::default()),
    };
    reconciler.reconcile(&pool, queue(), today).await.unwrap();
    assert_eq!(wake_uncovered(&pool, today).await.unwrap(), 0);
    run_maintenance(&pool, today).await.unwrap();
    assert!(!partition_exists(&pool, "events_2026_05").await);

    // The queue loads from its snapshot, and its log goes on after it.
    let mut tx = begin_system(&pool).await.unwrap();
    let snapshot: i64 = sqlx::query_scalar("SELECT max(seq) FROM queue_snapshots")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    let mut log = QueueLog::load(&mut tx, queue()).await.unwrap();
    assert_eq!(log.seq, snapshot);
    assert_eq!(log.uncovered_since, None);
    let pushed = Event {
        id: EventId::from_ulid(Ulid(900)),
        org_id: org(),
        queue_id: queue(),
        actor: Actor::System,
        occurred_at: today,
        cause_id: None,
        kind: EventKind::TargetPushed {
            before: None,
            after: "target-new".into(),
            sender_id: user(),
        },
    };
    log.write(&mut tx, vec![pushed], today).await.unwrap();
    assert_eq!(log.seq, snapshot + 1);

    // The whole log is gone: a replay refuses to start from nothing.
    let err = rebuild(&mut tx, queue(), today).await.unwrap_err();
    assert!(matches!(err, MergeQueueError::LogGap { .. }), "{err:?}");
}

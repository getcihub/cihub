//! Tests against Postgres: each `#[sqlx::test]` gets a new database.

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use cihub_api_types::QueueAccess;
use cihub_config::{Layer, QueueSettings, Setting};
use cihub_domain::{
    Actor, ApiScope, ApiTokenId, BlockReason, CheckConclusion, CheckKind, CheckReport,
    ConflictCulprit, ConnectionId, ConnectionKind, DirectMergeMode, Event, EventId, EventKind,
    ItemState, JobClass, MembershipRole, Mergeability, OrgId, Permission, PrState, Priority,
    QueueControl, QueueId, Reason, RepoId, RequiredStatusesSource, ReviewDecision, RunId,
    RunStatus, SessionId, Strategy, SubmittedVia, UserId, temp_branch,
};
use cihub_github::{GithubApp, GithubConfig};
use cihub_mergequeue::{
    Clock, MergeQueueError, QueueLog, REFRESH_PRS, Reconciler, SNAPSHOT_EVERY, Translate,
    queue_status_context, rebuild, request_config_checks, wake_uncovered,
};
use cihub_mq_core::plan;
use cihub_platform::{
    ConnectionBudget, Delivery, Executors, Job, SYNC_INSTALLATION, SYNC_INSTALLATIONS, SYNC_REPOS,
    Subscription, begin_reconciliation, claim_jobs, deliver_all, enqueue_job, find_route,
    mark_dirty, request_installations_sync, request_installations_sync_at_start, run_budget_writer,
    run_maintenance, run_router, run_worker, store_delivery, sync_installation_executor,
    sync_installations_executor, sync_repos_executor, sync_repos_job,
};
use cihub_scm::sim::{Fault, PrChange, Reviews, Sim};
use cihub_scm::{
    ConnectionRef, RepoRef, RequiredChecks, Scm, ScmAccount, ScmDelivery, ScmError, ScmEvent,
    ScmUser,
};
use cihub_store::{begin_org, begin_system, create_event_partitions};
use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;
use sqlx::{AssertSqlSafe, PgConnection, PgPool};
use time::format_description::well_known::Rfc3339;
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
        "INSERT INTO users (id, external_id, login) VALUES ('{user}', '3', 'octocat');
         INSERT INTO orgs (id, slug) VALUES ('{org}', 'acme');
         INSERT INTO scm_connections (id, org_id, account_external_id, kind, external_id,
                                      synced_at, permissions)
             VALUES ('{connection}', '{org}', '1', 'github_app', '1', '{synced_at}', '{{}}');
         INSERT INTO repos (id, org_id, connection_id, external_id, namespace, name, default_branch)
             VALUES ('{repo}', '{org}', '{connection}', '200', 'acme', 'app', 'main');
         INSERT INTO queues (id, org_id, repo_id, target_branch)
             VALUES ('{queue}', '{org}', '{repo}', 'main');",
        user = user(),
        org = org(),
        connection = connection(),
        repo = RepoId::from_ulid(Ulid(12)),
        queue = queue(),
        synced_at = clock(),
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

        let layer = Layer {
            failure_budget_retries: Some(Setting::new(0)),
            direct_merge_mode: Some(Setting::new(DirectMergeMode::Off)),
            ..Layer::default()
        };
        let mut tx = begin_system(&pool).await.unwrap();
        let log = QueueLog::load(&mut tx, queue()).await.unwrap();

        tx.commit().await.unwrap();

        let mut world = Self {
            pool,
            log,
            settings: QueueSettings::resolve(&layer, &Layer::default()).unwrap(),
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
        let mut written = self.log.write(&mut tx, vec![event]).await.unwrap();
        let plan = plan(&self.log.snapshot, &self.settings, self.now).unwrap();

        written.extend(self.log.write(&mut tx, plan.decisions).await.unwrap());
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
                sender_id: Some(user()),
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
                sender_permission: None,
                profile: None,
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
                unmet_conditions: Vec::new(),
                required_checks: Vec::new(),
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
        "SELECT jsonb_build_object('state', state, 'mode', mode,
                                   'state_changed_at', state_changed_at,
                                   'state_changed_by', state_changed_by,
                                   'target_head_sha', target_head_sha, 'window_size', window_size)
         FROM queues WHERE id = $1",
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

    let pause = EventKind::QueueControlRequested {
        control: QueueControl::Pause,
    };
    w.fact(Actor::User { user_id: user() }, pause).await;

    assert!(w.log.seq > SNAPSHOT_EVERY, "{} events only", w.log.seq);

    let mut tx = begin_system(&pool).await.unwrap();
    let queue_row: (
        String,
        Option<OffsetDateTime>,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT state::text, state_changed_at, state_changed_by, target_head_sha
             FROM queues WHERE id = $1",
    )
    .bind(queue().to_string())
    .fetch_one(&mut *tx)
    .await
    .unwrap();

    assert_eq!(
        queue_row,
        (
            "paused".into(),
            Some(w.now),
            Some(user().to_string()),
            Some(format!("target-{}", w.target))
        ),
        "the person who asked for the pause, not CIHub that decided it"
    );

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

    let last_snapshot = w.log.seq / SNAPSHOT_EVERY * SNAPSHOT_EVERY;
    let snapshot_seq: Option<i64> =
        sqlx::query_scalar("SELECT snapshot_seq FROM queues WHERE id = $1")
            .bind(queue().to_string())
            .fetch_one(&mut *tx)
            .await
            .unwrap();

    assert_eq!(snapshot_seq, Some(last_snapshot));

    let before = projections(&mut tx).await;
    let loaded = QueueLog::load(&mut tx, queue()).await.unwrap();

    assert_eq!(loaded, w.log);

    let rebuilt = rebuild(&mut tx, queue()).await.unwrap();
    assert_eq!(rebuilt, w.log);
    assert_eq!(projections(&mut tx).await, before);

    let snapshot_seq_after_rebuild: Option<i64> =
        sqlx::query_scalar("SELECT snapshot_seq FROM queues WHERE id = $1")
            .bind(queue().to_string())
            .fetch_one(&mut *tx)
            .await
            .unwrap();

    assert_eq!(snapshot_seq_after_rebuild, Some(last_snapshot));
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_fact_that_changes_nothing_is_not_written(pool: PgPool) {
    let mut w = World::new(pool.clone()).await;
    w.submit(1).await;

    let seq = w.log.seq;
    assert_eq!(w.observe(1, PrState::Open).await, []);
    assert_eq!(w.log.seq, seq);

    let mut tx = begin_system(&pool).await.unwrap();
    let item: (String, String, Option<i64>, String, Option<String>) = sqlx::query_as(
        "SELECT state::text, state_reason::text, position, pr_author_id, submitted_by
         FROM queue_items WHERE queue_id = $1",
    )
    .bind(queue().to_string())
    .fetch_one(&mut *tx)
    .await
    .unwrap();

    assert_eq!(item.0, ItemState::Testing.as_str());
    assert_eq!(item.1, "test_run_started");
    assert!(item.2.is_some());
    assert_eq!(item.3, user().to_string());
    assert_eq!(item.4, Some(user().to_string()));
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
    /// The routes of the module, until a test serves them.
    api: Option<cihub_platform::ModuleApi>,
    /// What the notifier runs; a test delivers the logs to them.
    subscriptions: Vec<Subscription>,
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

fn router_translate(
    deliveries: &Arc<Mutex<HashMap<String, ScmDelivery>>>,
) -> impl Fn(&str, &Value) -> Result<ScmDelivery, std::fmt::Error> + Send + Sync + 'static {
    let deliveries = deliveries.clone();
    move |_, payload| {
        let id = payload["id"].as_str().unwrap_or_default();
        deliveries
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or(std::fmt::Error)
    }
}

impl SimQueue {
    async fn new(pool: PgPool) -> Self {
        seed(&pool).await;

        let sim = Arc::new(Sim::new());
        let account = ScmAccount {
            external_id: "1".into(),
            path: "acme".into(),
        };

        sim.add_installation("1", account, &[("contents", "write")]);
        sim.take_deliveries();

        let clock = TestClock::new();
        let deliveries = Arc::new(Mutex::new(HashMap::new()));
        let scm: Arc<dyn Scm> = sim.clone();
        let module = cihub_mergequeue::module(
            pool.clone(),
            scm.clone(),
            translate(&deliveries),
            "scheduler".into(),
            clock.shared(),
            Some("https://cihub.example.com/".into()),
        );

        let mut executors = Executors::default();
        (module.executors)(&mut executors);

        let kind = ConnectionKind::GithubApp;
        executors.register(SYNC_REPOS, sync_repos_executor(scm.clone()));

        executors.register(
            SYNC_INSTALLATION,
            sync_installation_executor(scm.clone(), kind),
        );

        executors.register(SYNC_INSTALLATIONS, sync_installations_executor(scm, kind));

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
        let router_clock = clock.clone();
        let router = run_router(
            pool.clone(),
            router_translate(&deliveries),
            move || router_clock.now(),
            std::time::Duration::from_millis(50),
            cancel.clone(),
        );
        let scheduler = (module.scheduler)(cancel.clone());
        let api = Some(module.api);
        let subscriptions = module.subscriptions;
        let loops = vec![
            tokio::spawn(async move { worker.await.unwrap() }),
            tokio::spawn(async move { router.await.unwrap() }),
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
            api,
            subscriptions,
        }
    }

    /// Delivers the logs to the subscriptions, as the notifier does.
    async fn notify(&self) {
        for subscription in &self.subscriptions {
            deliver_all(&self.pool, subscription).await.unwrap();
        }
    }

    /// Serves the API of the module behind the web role, and returns its
    /// address and a token of the organization with every scope.
    async fn serve_api(&mut self) -> (String, String) {
        let api = self.api.take().unwrap();

        serve_api(&self.pool, api, self.cancel.clone()).await
    }

    /// Stores the deliveries of the simulator, routed as the web role does.
    async fn deliver(&mut self) {
        let mut tx = begin_system(&self.pool).await.unwrap();
        for delivery in self.sim.take_deliveries() {
            self.sent += 1;
            let id = format!("delivery-{}", self.sent);
            let route = find_route(&mut tx, &delivery).await.unwrap();
            self.deliveries.lock().unwrap().insert(id.clone(), delivery);
            let stored = Delivery {
                delivery_id: id.clone(),
                org_id: route.org_id,
                connection_id: route.connection_id,
                queue_id: route.queue_id,
                event: "sim".into(),
                payload: serde_json::json!({ "id": id }),
                received_at: self.clock.now(),
            };
            store_delivery(&mut tx, &stored).await.unwrap();
        }
        tx.commit().await.unwrap();
    }

    /// Stores the delivery `id` again, routed as the web role does, and
    /// returns `false` when the inbox still has it.
    async fn deliver_again(&self, id: &str) -> bool {
        let delivery = self.deliveries.lock().unwrap()[id].clone();
        let mut tx = begin_system(&self.pool).await.unwrap();
        let route = find_route(&mut tx, &delivery).await.unwrap();
        let again = Delivery {
            delivery_id: id.to_owned(),
            org_id: route.org_id,
            connection_id: route.connection_id,
            queue_id: route.queue_id,
            event: "sim".into(),
            payload: serde_json::json!({ "id": id }),
            received_at: self.clock.now(),
        };
        let stored = store_delivery(&mut tx, &again).await.unwrap();

        tx.commit().await.unwrap();

        stored
    }

    /// Delivers the deliveries of the simulator, then processes each one
    /// again, one at a time: the log of the queue and the jobs stay the same,
    /// except for a PR opened, which is in the queue now and asks one read of
    /// the PR, keyed by the delivery, and the `pr.observed` facts of that
    /// read. Returns the events of the deliveries.
    async fn deliver_and_process_again(&mut self) -> Vec<ScmEvent> {
        let first = self.sent + 1;
        self.deliver().await;
        self.settle().await;

        let mut events = Vec::new();
        for n in first..=self.sent {
            let id = format!("delivery-{n}");
            let event = self.deliveries.lock().unwrap()[&id].event.clone();
            let (log, mut jobs) = self.log_and_jobs().await;
            assert!(self.deliver_again(&id).await, "{id}");
            self.settle().await;
            let (log_again, jobs_again) = self.log_and_jobs().await;

            let is_opened_pr = matches!(event, ScmEvent::PrChanged { opened: true, .. });
            if is_opened_pr {
                jobs.push(format!("refresh_prs:{}-{id}:1", queue()));
                jobs.sort();
            }
            assert_eq!(jobs_again, jobs, "{id}");
            assert_eq!(log_again[..log.len()], log[..], "{id}");
            let added = &log_again[log.len()..];
            assert_eq!(!added.is_empty(), is_opened_pr, "{id}: {added:?}");
            assert!(
                added.iter().all(|(_, kind)| kind == "pr.observed"),
                "{id}: {added:?}"
            );

            events.push(event);
        }

        events
    }

    /// The types of the events of the queue in order, and the keys of the jobs.
    async fn log_and_jobs(&self) -> (Vec<(i64, String)>, Vec<String>) {
        let mut tx = begin_system(&self.pool).await.unwrap();
        let log =
            sqlx::query_as("SELECT seq, type::text FROM events WHERE queue_id = $1 ORDER BY seq")
                .bind(queue().to_string())
                .fetch_all(&mut *tx)
                .await
                .unwrap();
        let jobs = sqlx::query_scalar("SELECT idempotency_key FROM jobs ORDER BY 1")
            .fetch_all(&mut *tx)
            .await
            .unwrap();

        (log, jobs)
    }

    async fn send(&mut self) {
        let mut tx = begin_system(&self.pool).await.unwrap();
        for delivery in self.sim.take_deliveries() {
            self.sent += 1;
            let id = format!("delivery-{}", self.sent);
            self.deliveries.lock().unwrap().insert(id.clone(), delivery);
            let stored = Delivery {
                delivery_id: id.clone(),
                org_id: None,
                connection_id: None,
                queue_id: None,
                event: "sim".into(),
                payload: serde_json::json!({ "id": id }),
                received_at: self.clock.now(),
            };
            store_delivery(&mut tx, &stored).await.unwrap();
        }
        tx.commit().await.unwrap();
    }

    async fn request(&self, job: &Job) {
        let mut tx = begin_system(&self.pool).await.unwrap();
        let now = self.clock.now();

        enqueue_job(&mut tx, job, now, now).await.unwrap();
        tx.commit().await.unwrap();
    }

    async fn access(&self) -> (String, Vec<String>) {
        let mut tx = begin_system(&self.pool).await.unwrap();
        let state: String = sqlx::query_scalar("SELECT state::text FROM queues WHERE id = $1")
            .bind(queue().to_string())
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        let facts = sqlx::query_scalar(
            "SELECT type::text FROM events
             WHERE queue_id = $1 AND type IN ('access.lost', 'access.restored') ORDER BY seq",
        )
        .bind(queue().to_string())
        .fetch_all(&mut *tx)
        .await
        .unwrap();

        (state, facts)
    }

    /// Wakes the queue, as a change would: after a move of the clock.
    async fn wake(&self) {
        let mut tx = begin_system(&self.pool).await.unwrap();
        mark_dirty(&mut tx, queue()).await.unwrap();
        tx.commit().await.unwrap();
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

    /// Waits until every delivery, fact and job is done; a check of the
    /// configuration that waits for its moment of the hour is not work yet.
    async fn settle(&self) {
        for _ in 0..800 {
            let mut tx = begin_system(&self.pool).await.unwrap();
            let busy: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM webhook_inbox)
                     OR EXISTS (SELECT 1 FROM queue_inbox)
                     OR EXISTS (SELECT 1 FROM jobs
                                WHERE state = 'running'
                                   OR (state = 'queued'
                                       AND (type <> 'check_config' OR run_after <= $1)))
                     OR EXISTS (SELECT 1 FROM queues WHERE dirty)",
            )
            .bind(self.clock.now())
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
            "SELECT 'delivery', delivery_id FROM webhook_inbox
             UNION ALL SELECT 'fact', type::text FROM queue_inbox
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

/// Serves `api` behind the web role until `cancel`, and returns its address
/// and a token of the organization with every scope.
async fn serve_api(
    pool: &PgPool,
    api: cihub_platform::ModuleApi,
    cancel: CancellationToken,
) -> (String, String) {
    let secret = cihub_store::new_api_token_secret().unwrap();
    let scopes = [
        ApiScope::QueuesRead,
        ApiScope::QueuesWrite,
        ApiScope::PrsWrite,
    ];
    let token = cihub_store::NewApiToken {
        id: ApiTokenId::from_ulid(Ulid(40)),
        org_id: org(),
        name: "test",
        scopes: &scopes,
        expires_at: None,
    };
    let mut tx = begin_system(pool).await.unwrap();
    cihub_store::create_api_token(&mut tx, &token, &secret)
        .await
        .unwrap();

    tx.commit().await.unwrap();

    let webhooks = cihub_web::GithubWebhooks {
        secret: SecretString::from("unused"),
    };
    let fixed = || OffsetDateTime::from_unix_timestamp(1_790_000_000).unwrap();
    let dashboard = cihub_web::Dashboard {
        scm: Arc::new(Sim::new()),
        public_url: None,
        news: cihub_platform::QueueNewsFeed::start(pool.clone(), cancel.clone())
            .await
            .unwrap(),
    };
    let router = cihub_web::router(pool.clone(), fixed, webhooks, vec![api], dashboard);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();

    tokio::spawn(cihub_web::serve(listener, router, cancel));

    (addr, secret.expose_secret().to_owned())
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

/// Whether `body` is the comment that CIHub edits in place: neither a
/// command of a user nor the answer to a refused command.
fn is_status_comment(body: &str) -> bool {
    !body.starts_with("/cihub") && !body.starts_with("> ")
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
    let root = sim.add_repo(
        &repo(),
        "main",
        &[("a.txt", "a"), (".cihub.yaml", TEST_PATH)],
    );
    let own = queue_status_context("main");
    let rules = RequiredChecks {
        names: BTreeSet::from([REQUIRED.to_owned(), own]),
        up_to_date: false,
    };

    sim.set_rules(&repo(), "main", rules).unwrap();

    sim.set_permission(&repo(), &alice(), Permission::Write)
        .unwrap();

    // The push makes the queue read its file.
    let main = sim
        .commit(&repo(), &root, &[("README.md", Some("cihub"))])
        .unwrap();
    sim.push(&repo(), "main", &main, &alice()).unwrap();
    q.deliver().await;
    q.settle().await;

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
    let failing_ci = vec![
        BlockReason::FailingRequiredCheck {
            check: REQUIRED.into(),
        },
        BlockReason::QueueConditionsUnmet {
            conditions: vec![format!("check-success = {REQUIRED}")],
        },
    ];
    let cases: Vec<(Setup, ItemState, Vec<BlockReason>)> = vec![
        (Box::new(ready), ItemState::Pending, vec![]),
        (Box::new(ready), ItemState::Pending, vec![]),
        (Box::new(ready), ItemState::Pending, vec![]),
        (Box::new(ready), ItemState::Pending, vec![]),
        (Box::new(ready), ItemState::Pending, vec![]),
        (Box::new(ready), ItemState::Pending, vec![]),
        (Box::new(failing), ItemState::NotReady, failing_ci.clone()),
        (Box::new(failing), ItemState::NotReady, failing_ci),
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
    let testing = rows.iter().filter(|r| r.1 == "testing").count();

    assert!(testing <= 2, "{testing} items in test");

    let got: BTreeMap<u64, (ItemState, Vec<BlockReason>)> = rows
        .iter()
        .map(|(n, state, blocks, _)| {
            let state = match state.parse().unwrap() {
                ItemState::Testing => ItemState::Pending,
                state => state,
            };
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
    let expected: &[&str] = if testing == 0 {
        &["merge_item_now_ready", "submitted"]
    } else {
        &["merge_item_now_ready", "submitted", "test_run_started"]
    };

    assert_eq!(reasons, expected);
    drop(tx);

    let mut tx = begin_system(&pool).await.unwrap();
    let before = projections(&mut tx).await;

    rebuild(&mut tx, queue()).await.unwrap();
    assert_eq!(projections(&mut tx).await, before);
    drop(tx);

    sim.set_access("1", false);
    q.send().await;
    q.settle().await;
    assert_eq!(q.access().await.0, "paused");
    q.stop().await;
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_lost_access_pauses_the_queue_with_or_without_its_webhook(pool: PgPool) {
    let mut q = SimQueue::new(pool).await;
    let sim = q.sim.clone();

    sim.add_repo(&repo(), "main", &[("a.txt", "a")]);
    sim.take_deliveries();

    let lost = || "access.lost".to_owned();
    let restored = || "access.restored".to_owned();

    sim.set_access("1", false);
    q.send().await;
    q.settle().await;
    assert_eq!(q.access().await, ("paused".into(), vec![lost()]));

    sim.set_access("1", true);
    q.send().await;
    q.settle().await;

    assert_eq!(
        q.access().await,
        ("running".into(), vec![lost(), restored()])
    );

    sim.set_access("1", false);
    sim.take_deliveries();

    let mut tx = begin_system(&q.pool).await.unwrap();
    request_installations_sync(&mut tx, q.clock.now())
        .await
        .unwrap();

    tx.commit().await.unwrap();
    q.settle().await;

    let expected = vec![lost(), restored(), lost()];
    assert_eq!(q.access().await, ("paused".into(), expected));

    sim.set_access("1", true);
    sim.take_deliveries();

    let mut tx = begin_system(&q.pool).await.unwrap();
    request_installations_sync_at_start(&mut tx, q.clock.now())
        .await
        .unwrap();

    tx.commit().await.unwrap();
    q.settle().await;

    let expected = vec![lost(), restored(), lost(), restored()];
    assert_eq!(q.access().await, ("running".into(), expected));

    sim.set_access("1", false);
    sim.take_deliveries();

    q.request(&sync_repos_job(
        org(),
        connection(),
        "after-the-lost-webhook",
    ))
    .await;

    q.settle().await;

    let expected = vec![lost(), restored(), lost(), restored(), lost()];
    assert_eq!(q.access().await, ("paused".into(), expected));
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
        let route = find_route(&mut tx, &delivery).await.unwrap();
        deliveries.lock().unwrap().insert(id.clone(), delivery);
        let stored = Delivery {
            delivery_id: id.clone(),
            org_id: route.org_id,
            connection_id: route.connection_id,
            queue_id: route.queue_id,
            event: "sim".into(),
            payload: serde_json::json!({ "id": id }),
            received_at: clock(),
        };
        store_delivery(&mut tx, &stored).await.unwrap();
    }

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
            sender_id: Some(user()),
        },
    };

    cihub_store::deposit_fact(&mut tx, &fact, clock())
        .await
        .unwrap();

    cihub_platform::mark_dirty(&mut tx, queue()).await.unwrap();
    tx.commit().await.unwrap();

    let reconciler = Reconciler {
        translate: translate(&deliveries),
        dashboard_url: None,
    };
    let counts = |pool: PgPool| async move {
        let mut tx = begin_system(&pool).await.unwrap();
        let counts: (i64, i64, i64, i64, bool) = sqlx::query_as(
            "SELECT (SELECT count(*) FROM events),
                    (SELECT count(*) FROM jobs),
                    (SELECT count(*) FROM webhook_inbox),
                    (SELECT count(*) FROM queue_inbox),
                    (SELECT dirty FROM queues)",
        )
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        counts
    };
    let before = counts(pool.clone()).await;

    assert_eq!(before, (0, 0, 1, 1, true));

    let mut tx = begin_reconciliation(&pool, queue()).await.unwrap().unwrap();
    reconciler
        .reconcile_in(&mut tx, queue(), clock())
        .await
        .unwrap();

    drop(tx);
    assert_eq!(counts(pool.clone()).await, before);

    reconciler.reconcile(&pool, queue(), &clock).await.unwrap();

    let (events, jobs, deliveries_left, facts_left, dirty) = counts(pool.clone()).await;
    assert_eq!(
        (events, jobs, deliveries_left, facts_left, dirty),
        (1, 2, 0, 0, false)
    );

    let mut tx = begin_system(&pool).await.unwrap();
    let keys: Vec<String> = sqlx::query_scalar("SELECT idempotency_key FROM jobs ORDER BY 1")
        .fetch_all(&mut *tx)
        .await
        .unwrap();

    assert_eq!(
        keys,
        [
            format!("load_config:{}-{root}:1", queue()),
            "submit_pr:delivery-0:1".to_owned()
        ]
    );
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
/// same mapping as the `worker` role of `cihub-cli`.
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
        connection: Some(connection()),
        queue_id: Some(queue()),
        payload: serde_json::json!({ "prs": [n] }),
        attempts: 0,
    }
}

/// The lot 4 chain, end to end: an executor calls GitHub through the
/// connection, GitHub answers 429, the pause reaches `scm_connections`, and
/// no job of the connection goes out before the reset.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_rate_limit_pauses_the_jobs_of_the_connection_until_its_reset(pool: PgPool) {
    seed(&pool).await;

    let now = clock();
    let reset = now + Duration::minutes(15);
    let config = GithubConfig {
        url: rate_limited_github(reset).await,
        app_id: "1".into(),
        private_key: SecretString::from(include_str!("../../github/tests/test-only-key.pem")),
        sign_in: None,
    };
    let app = Arc::new(GithubApp::new(&config, Arc::new(clock)).unwrap());
    let scm: Arc<dyn Scm> = app.clone();
    let module = cihub_mergequeue::module(
        pool.clone(),
        scm,
        translate(&Arc::default()),
        "scheduler".into(),
        Arc::new(clock),
        None,
    );
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

    let mut tx = begin_system(&pool).await.unwrap();
    let (state, error): (String, String) =
        sqlx::query_as("SELECT state::text, last_error FROM jobs")
            .fetch_one(&mut *tx)
            .await
            .unwrap();

    assert_eq!(state, "queued");
    assert!(error.contains("rate limit"), "{error}");
    enqueue_job(&mut tx, &read_job(2), now, now).await.unwrap();

    assert_eq!(
        claim_jobs(&mut tx, "worker", reset - Duration::seconds(1), 10)
            .await
            .unwrap(),
        []
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

/// A file whose queue tests every PR: these tests follow the test path.
const TEST_PATH: &str = "version: 1\nqueues:\n  - branch: main\n    direct_merge_mode: off\n";

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

    sim.set_permission(&repo(), &alice(), Permission::Write)
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
        "SELECT id, test_sha, status::text,
                CASE WHEN status IN ('failed', 'timed_out') THEN status_reason::text END,
                head_shas, tested_pr_numbers,
                required_statuses, required_statuses_source::text, conflict_culprit, test_branch,
                draft_pr_number
         FROM test_runs ORDER BY started_at, id",
    )
    .fetch_all(conn)
    .await
    .unwrap()
}

/// The exit criterion of lot 6 (`specs/LOTS.md`): ten PRs, two with a red CI,
/// one in conflict with a PR ahead of it, one whose CI never reports. Each
/// PR is tested on top of its predecessors, each run ends with its reason,
/// the PRs behind a failed one restart without it, and the merge job merges
/// the proved PRs.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn ten_prs_are_tested_on_top_of_their_predecessors(pool: PgPool) {
    const FAILING: [u64; 2] = [3, 8];
    const CONFLICTING: u64 = 6;
    const SILENT: u64 = 9;

    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let config = format!(
        "version: 1\nqueues:\n  - branch: main\n    required_statuses: [{REQUIRED}]\n    direct_merge_mode: off\n"
    );
    let main = target_repo(&sim, &[("a.txt", "a"), (".cihub.yaml", &config)]);
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

    for n in 1..=10 {
        sim.comment(&repo(), n, &alice(), "/cihub merge").unwrap();
        q.deliver().await;
        q.settle().await;
    }

    let mut rounds = 0;
    loop {
        q.deliver().await;
        q.settle().await;
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
        if reported == 0 {
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

    let restarts: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM events
         WHERE type = 'item.transitioned' AND payload->>'reason' = $1",
    )
    .bind(Reason::PrerequisiteMergeItemFailed.as_str())
    .fetch_one(&mut *tx)
    .await
    .unwrap();

    assert!(restarts >= 1, "{restarts} restarts");

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

    q.deliver().await;
    q.settle().await;

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

    let before = projections(&mut tx).await;
    rebuild(&mut tx, queue()).await.unwrap();
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
    let main = target_repo(&sim, &[("a.txt", "a"), (".cihub.yaml", TEST_PATH)]);
    let submit = |n: u64| {
        let path = format!("f{n}.txt");
        let (number, head) = open_pr_with(&sim, &main, n, &[(&path, Some("x"))]);
        sim.report_check(&repo(), &head, check(REQUIRED, true))
            .unwrap();
        sim.comment(&repo(), number, &alice(), "/cihub merge")
            .unwrap();
    };

    q.deliver().await;
    q.settle().await;
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
            (2, "merged".into(), "merge_graph_merge".into()),
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

/// The state, the block reasons and the sender permission of the live item
/// of each PR.
async fn admissions(pool: &PgPool) -> Vec<(i64, String, serde_json::Value, Option<String>)> {
    let mut tx = begin_system(pool).await.unwrap();
    sqlx::query_as(
        "SELECT pr_number, state::text, block_reasons, sender_permission::text
         FROM queue_items WHERE is_current ORDER BY pr_number",
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap()
}

/// A PR waits in `not_ready` with the `queue_conditions` that it does not
/// meet, and enters the queue once it meets them; the files are read only
/// because a condition uses them.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_pr_waits_for_its_queue_conditions(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let config = format!(
        "version: 1\nqueues:\n  - branch: main\n    required_statuses: [{REQUIRED}]\n    direct_merge_mode: off\n    queue_conditions:\n      - label = ready\n      - sender-permission >= write\n      - \"#files <= 1\"\n"
    );
    let main = target_repo(&sim, &[("a.txt", "a"), (".cihub.yaml", &config)]);

    q.deliver().await;
    q.settle().await;

    let (small, small_head) = open_pr_with(&sim, &main, 1, &[("f1.txt", Some("x"))]);
    let (large, large_head) = open_pr_with(
        &sim,
        &main,
        2,
        &[("f2.txt", Some("x")), ("g2.txt", Some("x"))],
    );

    for (number, head) in [(small, &small_head), (large, &large_head)] {
        sim.report_check(&repo(), head, check(REQUIRED, true))
            .unwrap();
        sim.comment(&repo(), number, &alice(), "/cihub merge")
            .unwrap();
    }

    q.deliver().await;
    q.settle().await;

    let unmet = |conditions: &[&str]| serde_json::json!([{ "kind": "queue_conditions_unmet", "conditions": conditions }]);
    let write = Some("write".to_owned());

    assert_eq!(
        admissions(&pool).await,
        [
            (
                1,
                "not_ready".into(),
                unmet(&["label = ready"]),
                write.clone()
            ),
            (
                2,
                "not_ready".into(),
                unmet(&["label = ready", "#files <= 1"]),
                write.clone()
            ),
        ]
    );

    for number in [small, large] {
        sim.add_label(&repo(), number, "ready", JobClass::Interactive)
            .await
            .unwrap();
    }

    q.deliver().await;
    q.settle().await;

    let states = admissions(&pool).await;
    assert_eq!(states[0].0, 1);
    assert_ne!(states[0].1, "not_ready", "{states:?}");
    assert_eq!(states[0].2, serde_json::json!([]));
    assert_eq!(
        states[1],
        (2, "not_ready".into(), unmet(&["#files <= 1"]), write)
    );

    let reads = sim
        .calls()
        .iter()
        .filter(|call| call.name == "pr_files")
        .count();

    assert!(reads >= 2, "{reads}");
    assert!(
        !sim.calls()
            .iter()
            .any(|call| call.name == "unresolved_threads")
    );

    q.stop().await;
}

/// The queue, the priority and whether the item left `not_ready`, of the
/// current item of each PR.
async fn submitted_items(pool: &PgPool) -> Vec<(i64, String, i16, bool)> {
    let mut tx = begin_system(pool).await.unwrap();
    let items: Vec<(i64, String, i16, String)> = sqlx::query_as(
        "SELECT i.pr_number, q.target_branch, i.priority_value, i.state::text
         FROM queue_items i JOIN queues q ON q.id = i.queue_id
         WHERE i.is_current ORDER BY i.pr_number",
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap();

    drop(tx);

    items
        .into_iter()
        .map(|(n, queue, priority, state)| (n, queue, priority, state != "not_ready"))
        .collect()
}

/// A PR enters the queue of its base branch, with the priority of the first
/// rule that it meets unless the command gives one. A PR whose CI still runs
/// waits in `not_ready` for its `queue_conditions`.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_pr_enters_the_queue_of_its_branch_with_the_priority_of_its_rule(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let config = format!(
        "version: 1
queues:
  - branch: main
    strategy: batched
    batching: {{ size: 5, max_wait_minutes: 1 }}
    queue_conditions:
      - check-success = {REQUIRED}
      - \"#approved-reviews-by >= 1\"
priority_rules:
  - name: urgent
    conditions:
      - label = urgent
    priority: urgent
"
    );
    let main = target_repo(&sim, &[("a.txt", "a"), (".cihub.yaml", &config)]);

    q.deliver().await;
    q.settle().await;

    let cases: [(bool, &[&str], &str); 4] = [
        (false, &[], "/cihub merge"),
        (true, &[], "/cihub merge"),
        (true, &["urgent"], "/cihub merge"),
        (true, &["urgent"], "/cihub merge --priority=low"),
    ];

    for (n, (green, labels, command)) in (1..).zip(cases) {
        let (number, head) = open_pr_with(&sim, &main, n, &[(&format!("f{n}.txt"), Some("x"))]);
        if green {
            sim.report_check(&repo(), &head, check(REQUIRED, true))
                .unwrap();
            let reviews = Reviews {
                approved_by: BTreeSet::from([alice().login]),
                ..Reviews::default()
            };
            sim.set_reviews(&repo(), number, reviews).unwrap();
        }
        for label in labels {
            sim.add_label(&repo(), number, label, JobClass::Interactive)
                .await
                .unwrap();
        }
        sim.comment(&repo(), number, &alice(), command).unwrap();
    }

    q.deliver().await;
    q.settle().await;

    assert_eq!(
        submitted_items(&pool).await,
        [
            (1, "main".into(), 100, false),
            (2, "main".into(), 100, true),
            (3, "main".into(), 0, true),
            (4, "main".into(), 200, true),
        ]
    );

    q.stop().await;
}

/// A push that the routing gave to no queue, as the branch had none yet,
/// waits while a merge of the queue of the branch is in flight, then asks it
/// to read its head.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_push_without_a_queue_waits_for_a_merge_in_flight(pool: PgPool) {
    seed(&pool).await;

    let mut tx = begin_system(&pool).await.unwrap();

    let sim = Sim::new();
    let root = sim.add_repo(&repo(), "main", &[("a.txt", "a")]);
    let head = sim.commit(&repo(), &root, &[("b.txt", Some("b"))]).unwrap();

    sim.take_deliveries();
    sim.push(&repo(), "main", &head, &alice()).unwrap();

    let deliveries = Arc::new(Mutex::new(HashMap::new()));
    for (n, delivery) in sim.take_deliveries().into_iter().enumerate() {
        let id = format!("push-{n}");
        let route = find_route(&mut tx, &delivery).await.unwrap();
        deliveries.lock().unwrap().insert(id.clone(), delivery);
        let stored = Delivery {
            delivery_id: id.clone(),
            org_id: route.org_id,
            connection_id: route.connection_id,
            queue_id: None,
            event: "sim".into(),
            payload: serde_json::json!({ "id": id }),
            received_at: clock(),
        };
        store_delivery(&mut tx, &stored).await.unwrap();
    }

    let merge = Job {
        idempotency_key: "merge_pr:in-flight:1".into(),
        job_type: "merge_pr".into(),
        class: JobClass::Merge,
        org_id: Some(org()),
        connection: Some(connection()),
        queue_id: Some(queue()),
        payload: serde_json::json!({}),
        attempts: 0,
    };

    enqueue_job(&mut tx, &merge, clock() + Duration::hours(1), clock())
        .await
        .unwrap();

    tx.commit().await.unwrap();

    let reconciler = Reconciler {
        translate: translate(&deliveries),
        dashboard_url: None,
    };
    let state = |pool: PgPool| async move {
        let mut tx = begin_system(&pool).await.unwrap();
        sqlx::query_as::<_, (i64, i64)>(
            "SELECT (SELECT count(*) FROM webhook_inbox),
                    (SELECT count(*) FROM jobs WHERE type = 'refresh_target')",
        )
        .fetch_one(&mut *tx)
        .await
        .unwrap()
    };

    cihub_mergequeue::pass_connection_deliveries(&pool, &reconciler, clock())
        .await
        .unwrap();

    assert_eq!(state(pool.clone()).await, (1, 0), "the push waits");

    let mut tx = begin_system(&pool).await.unwrap();
    sqlx::query(
        "UPDATE jobs SET state = 'done', finished_at = now()
         WHERE idempotency_key = 'merge_pr:in-flight:1'",
    )
    .execute(&mut *tx)
    .await
    .unwrap();

    tx.commit().await.unwrap();

    cihub_mergequeue::pass_connection_deliveries(&pool, &reconciler, clock())
        .await
        .unwrap();

    assert_eq!(state(pool.clone()).await, (0, 1));
}

/// With `resubmit_on_push`, a push on a PR in the queue cancels its item and
/// submits the new head, with the same priority.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_push_submits_the_pr_again(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let config = format!(
        "version: 1\nqueues:\n  - branch: main\n    required_statuses: [{REQUIRED}]\n    direct_merge_mode: off\n    resubmit_on_push: true\n"
    );
    let main = target_repo(&sim, &[("a.txt", "a"), (".cihub.yaml", &config)]);

    q.deliver().await;
    q.settle().await;

    let (number, head) = open_pr_with(&sim, &main, 1, &[("f1.txt", Some("x"))]);
    sim.report_check(&repo(), &head, check(REQUIRED, true))
        .unwrap();

    sim.comment(&repo(), number, &alice(), "/cihub merge --priority=high")
        .unwrap();

    q.deliver().await;
    q.settle().await;

    let pushed = sim
        .commit(&repo(), &head, &[("f1.txt", Some("y"))])
        .unwrap();

    sim.push(&repo(), "pr-1", &pushed, &bob()).unwrap();

    sim.report_check(&repo(), &pushed, check(REQUIRED, true))
        .unwrap();

    q.deliver().await;
    q.settle().await;

    let mut tx = begin_system(&pool).await.unwrap();
    let items: Vec<(String, String, String, i16, bool)> = sqlx::query_as(
        "SELECT pr_head_sha, state::text, state_reason::text, priority_value, is_current
         FROM queue_items WHERE pr_number = $1 ORDER BY state_changed_at, id",
    )
    .bind(i64::try_from(number).unwrap())
    .fetch_all(&mut *tx)
    .await
    .unwrap();

    drop(tx);
    assert_eq!(items.len(), 2, "{items:?}");
    assert_eq!(
        (
            &items[0].0,
            items[0].1.as_str(),
            items[0].2.as_str(),
            items[0].4
        ),
        (&head, "cancelled", "pr_pushed_to", false)
    );
    assert_eq!((&items[1].0, items[1].3, items[1].4), (&pushed, 10, true));
    assert_ne!(items[1].1, "cancelled");
    q.stop().await;
}

/// A check of the CI that has not concluded yet.
fn running(name: &str) -> CheckReport {
    CheckReport {
        conclusion: None,
        ..check(name, true)
    }
}

/// The keys of the `refresh_prs` jobs, in order.
async fn refresh_keys(pool: &PgPool) -> Vec<String> {
    let mut tx = begin_system(pool).await.unwrap();
    sqlx::query_scalar(
        "SELECT idempotency_key FROM jobs WHERE type = 'refresh_prs' ORDER BY idempotency_key",
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap()
}

/// The state of the live item of the PR `number`.
async fn item_state(pool: &PgPool, number: u64) -> String {
    let mut tx = begin_system(pool).await.unwrap();
    sqlx::query_scalar("SELECT state::text FROM queue_items WHERE pr_number = $1 AND is_current")
        .bind(i64::try_from(number).unwrap())
        .fetch_one(&mut *tx)
        .await
        .unwrap()
}

/// A check that concludes on the head of a waiting item asks its queue to
/// read the PR at once, before any read point; a check in progress and a
/// replayed delivery ask nothing.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_concluded_check_reads_a_waiting_pr_at_once(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let main = target_repo(&sim, &[("a.txt", "a"), (".cihub.yaml", TEST_PATH)]);

    q.deliver().await;
    q.settle().await;

    let (number, head) = open_pr_with(&sim, &main, 1, &[("f1.txt", Some("x"))]);
    sim.comment(&repo(), number, &alice(), "/cihub merge")
        .unwrap();

    q.deliver().await;
    q.settle().await;
    assert_eq!(item_state(&pool, number).await, "not_ready");

    let before = refresh_keys(&pool).await;

    sim.report_check(&repo(), &head, running(REQUIRED)).unwrap();
    q.deliver().await;
    q.settle().await;
    assert_eq!(refresh_keys(&pool).await, before, "a check in progress");
    assert_eq!(item_state(&pool, number).await, "not_ready");

    sim.report_check(&repo(), &head, check(REQUIRED, true))
        .unwrap();

    q.deliver().await;

    let delivery = format!("delivery-{}", q.sent);
    q.settle().await;

    let mut after = before.clone();
    after.push(format!("refresh_prs:{}-{delivery}:1", queue()));
    after.sort();
    assert_eq!(refresh_keys(&pool).await, after);
    assert_eq!(item_state(&pool, number).await, "testing", "admitted");
    assert!(
        q.clock.now() < clock() + Duration::minutes(1),
        "no read point came due"
    );

    assert!(q.deliver_again(&delivery).await);

    let reconciler = Reconciler {
        translate: translate(&q.deliveries),
        dashboard_url: None,
    };

    cihub_mergequeue::pass_connection_deliveries(&pool, &reconciler, q.clock.now())
        .await
        .unwrap();

    q.settle().await;
    assert_eq!(refresh_keys(&pool).await, after);
    q.stop().await;
}

/// A delivery stored and processed again once the inbox deleted it writes no
/// wrong fact and asks no job twice: the change of a PR, a `/cihub merge`
/// command, a check on the head of a PR and on the test SHA of a live run,
/// and the push and the change of the PR that merge it.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_delivery_processed_again_writes_no_wrong_fact(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let main = target_repo(&sim, &[("a.txt", "a"), (".cihub.yaml", TEST_PATH)]);

    q.deliver().await;
    q.settle().await;

    let (number, head) = open_pr_with(&sim, &main, 1, &[("f1.txt", Some("x"))]);
    sim.report_check(&repo(), &head, check(REQUIRED, true))
        .unwrap();

    sim.comment(&repo(), number, &alice(), "/cihub merge")
        .unwrap();

    let submitted = q.deliver_and_process_again().await;
    assert!(
        submitted
            .iter()
            .any(|event| matches!(event, ScmEvent::PrChanged { opened: true, .. }))
    );
    assert!(
        submitted
            .iter()
            .any(|event| matches!(event, ScmEvent::Commented { .. }))
    );
    assert!(
        submitted
            .iter()
            .any(|event| matches!(event, ScmEvent::CheckChanged { sha, .. } if *sha == head))
    );

    let (_, test_sha, _) = q.runs_in_progress().await.remove(0);
    sim.report_check(&repo(), &test_sha, running(REQUIRED))
        .unwrap();

    let reported = q.deliver_and_process_again().await;
    assert!(
        reported
            .iter()
            .any(|event| matches!(event, ScmEvent::CheckChanged { sha, .. } if *sha == test_sha))
    );
    assert_eq!(item_state(&pool, number).await, "testing");

    sim.report_check(&repo(), &test_sha, check(REQUIRED, true))
        .unwrap();

    q.deliver_and_process_again().await;

    let merged = q.deliver_and_process_again().await;
    assert!(
        merged
            .iter()
            .any(|event| matches!(event, ScmEvent::Pushed { branch, .. } if branch == "main"))
    );
    assert_eq!(item_state(&pool, number).await, "merged");
    q.stop().await;
}

/// A check that concludes on the head of an admitted item, or on a commit
/// that is the head of no item, asks for no read.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_concluded_check_elsewhere_reads_nothing(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let main = target_repo(&sim, &[("a.txt", "a"), (".cihub.yaml", TEST_PATH)]);

    q.deliver().await;
    q.settle().await;

    let (number, head) = open_pr_with(&sim, &main, 1, &[("f1.txt", Some("x"))]);
    sim.report_check(&repo(), &head, check(REQUIRED, true))
        .unwrap();

    sim.comment(&repo(), number, &alice(), "/cihub merge")
        .unwrap();

    let (_, other) = open_pr_with(&sim, &main, 2, &[("f2.txt", Some("x"))]);
    q.deliver().await;
    q.settle().await;
    assert_eq!(item_state(&pool, number).await, "testing", "admitted");

    let before = refresh_keys(&pool).await;

    sim.report_check(&repo(), &head, check("lint", false))
        .unwrap();

    sim.report_check(&repo(), &other, check(REQUIRED, true))
        .unwrap();

    q.deliver().await;
    q.settle().await;
    assert_eq!(refresh_keys(&pool).await, before);
    q.stop().await;
}

/// A PR takes the first profile whose `conditions` it meets, and merges with
/// the method of that profile; another PR merges with the method of the
/// queue.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_pr_merges_with_the_settings_of_its_profile(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let config = format!(
        "version: 1
queues:
  - branch: main
    required_statuses: [{REQUIRED}]
    merge_method: squash
    profiles:
      - name: dependencies
        conditions:
          - label = deps
        merge_method: merge_commit
"
    );
    let main = target_repo(&sim, &[("a.txt", "a"), (".cihub.yaml", &config)]);

    q.deliver().await;
    q.settle().await;

    let mut base = main;
    let mut heads = Vec::new();
    for (n, labels) in [(1, &["deps"][..]), (2, &[][..])] {
        let (number, head) = open_pr_with(&sim, &base, n, &[(&format!("f{n}.txt"), Some("x"))]);
        sim.report_check(&repo(), &head, check(REQUIRED, true))
            .unwrap();
        for label in labels {
            sim.add_label(&repo(), number, label, JobClass::Interactive)
                .await
                .unwrap();
        }
        sim.comment(&repo(), number, &alice(), "/cihub merge")
            .unwrap();

        q.deliver().await;
        q.settle().await;

        base = sim
            .branch_head(&repo(), "main", JobClass::Repair)
            .await
            .unwrap()
            .unwrap();
        heads.push(head);
    }

    let mut tx = begin_system(&pool).await.unwrap();
    let items: Vec<(i64, String, Option<String>)> = sqlx::query_as(
        "SELECT pr_number, state::text, profile FROM queue_items WHERE is_current
         ORDER BY pr_number",
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap();

    drop(tx);

    assert_eq!(
        items,
        [
            (1, "merged".into(), Some("dependencies".into())),
            (2, "merged".into(), None),
        ]
    );
    let repo = repo();
    let mut kept = Vec::new();
    for head in &heads {
        kept.push(
            sim.is_ancestor(&repo, head, &base, JobClass::Repair)
                .await
                .unwrap(),
        );
    }
    assert_eq!(
        kept,
        [true, false],
        "a merge commit keeps the head, a squash does not"
    );

    q.stop().await;
}

/// A command that names a profile that the queue does not have is refused,
/// with the cause, and gives no item.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_command_with_an_unknown_profile_is_refused(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let main = target_repo(&sim, &[("a.txt", "a")]);

    q.deliver().await;
    q.settle().await;

    let (number, head) = open_pr_with(&sim, &main, 1, &[("f1.txt", Some("x"))]);
    sim.report_check(&repo(), &head, check(REQUIRED, true))
        .unwrap();

    sim.comment(&repo(), number, &alice(), "/cihub merge --profile=fast")
        .unwrap();

    q.deliver().await;
    q.settle().await;

    assert_eq!(current_items(&pool).await, []);
    assert!(
        sim.comments(&repo(), number)
            .iter()
            .any(|body| body.contains("the queue of `main` has no profile `fast`.")),
        "{:?}",
        sim.comments(&repo(), number)
    );

    q.stop().await;
}

/// Without a file and without a check that the branch requires, a green PR
/// that contains the head of the target does not merge directly: its test
/// fails with `start_test_run_no_required_statuses`, as nothing would gate
/// the merge.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_branch_without_a_required_check_merges_nothing(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let main = target_repo(&sim, &[("a.txt", "a")]);
    let no_rules = RequiredChecks {
        names: BTreeSet::new(),
        up_to_date: false,
    };

    sim.set_rules(&repo(), "main", no_rules).unwrap();
    q.deliver().await;
    q.settle().await;

    let (number, head) = open_pr_with(&sim, &main, 1, &[("f1.txt", Some("x"))]);
    sim.report_check(&repo(), &head, check(REQUIRED, true))
        .unwrap();

    sim.comment(&repo(), number, &alice(), "/cihub merge")
        .unwrap();

    q.deliver().await;
    q.settle().await;

    let pr = sim
        .pull_request(&repo(), number, JobClass::Repair)
        .await
        .unwrap();

    assert_ne!(pr.state, PrState::Merged);
    assert_eq!(
        current_items(&pool).await,
        [(
            i64::try_from(number).unwrap(),
            "failed".into(),
            "start_test_run_no_required_statuses".into()
        )]
    );

    q.stop().await;
}

/// A submission while the queue drains gives an item that is cancelled at
/// once, concluded, and not the live item of its PR.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_draining_queue_refuses_a_submission(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let main = target_repo(&sim, &[("a.txt", "a")]);

    q.deliver().await;
    q.settle().await;

    let drain = Event {
        id: EventId::from_ulid(Ulid(700)),
        org_id: org(),
        queue_id: queue(),
        actor: Actor::System,
        occurred_at: q.clock.now(),
        cause_id: None,
        kind: EventKind::QueueControlRequested {
            control: cihub_domain::QueueControl::Drain,
        },
    };
    let mut tx = begin_system(&pool).await.unwrap();
    cihub_store::deposit_fact(&mut tx, &drain, q.clock.now())
        .await
        .unwrap();

    mark_dirty(&mut tx, queue()).await.unwrap();
    tx.commit().await.unwrap();
    q.settle().await;

    let (number, head) = open_pr_with(&sim, &main, 1, &[("f1.txt", Some("x"))]);
    sim.report_check(&repo(), &head, check(REQUIRED, true))
        .unwrap();

    sim.comment(&repo(), number, &alice(), "/cihub merge")
        .unwrap();

    q.deliver().await;
    q.settle().await;

    let mut tx = begin_system(&pool).await.unwrap();
    let items: Vec<(String, String, bool)> =
        sqlx::query_as("SELECT state::text, state_reason::text, is_current FROM queue_items")
            .fetch_all(&mut *tx)
            .await
            .unwrap();

    drop(tx);

    assert_eq!(
        items,
        [("cancelled".into(), "queue_draining".into(), false)]
    );

    q.stop().await;
}

/// A command on a PR that is merged, closed or a draft is refused with its
/// reason, and gives no item. The refusal mentions the author of the command.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_pr_that_is_not_open_is_not_submitted(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let main = target_repo(&sim, &[("a.txt", "a")]);

    q.deliver().await;
    q.settle().await;

    for n in 1..=3 {
        assert_eq!(open_green_pr(&sim, &main, n), n);
    }

    let by_hand = cihub_scm::PrMerge {
        number: 1,
        head_sha: sim
            .pull_request(&repo(), 1, JobClass::Repair)
            .await
            .unwrap()
            .head_sha,
        tested_sha: String::new(),
        method: cihub_domain::MergeMethod::Squash,
        title: "merged by hand".into(),
        message: String::new(),
    };
    sim.merge_pr(&repo(), &by_hand, JobClass::Interactive)
        .await
        .unwrap();
    sim.change_pr(&repo(), 2, PrChange::Close).unwrap();
    sim.change_pr(&repo(), 3, PrChange::Draft(true)).unwrap();
    q.deliver().await;
    q.settle().await;

    for n in 1..=3 {
        sim.comment(&repo(), n, &alice(), "/cihub merge").unwrap();
    }
    q.deliver().await;
    q.settle().await;

    let mut tx = begin_system(&pool).await.unwrap();
    let (items,): (i64,) = sqlx::query_as("SELECT count(*) FROM queue_items")
        .fetch_one(&mut *tx)
        .await
        .unwrap();

    drop(tx);

    assert_eq!(items, 0);
    for (n, state) in [(1, "merged"), (2, "closed"), (3, "a draft")] {
        let comments = sim.comments(&repo(), n);
        let refusal = format!(
            "> /cihub merge\n\n@alice, the queue refused this command: this PR is {state}.\n"
        );
        assert!(comments.contains(&refusal), "PR {n}: {comments:?}");
    }

    q.stop().await;
}

/// The queues of the repository, as (target branch, whether it keeps a
/// file, config SHA), and the queue of each current item.
async fn queues_and_items(
    pool: &PgPool,
) -> (Vec<(String, bool, Option<String>)>, Vec<(i64, String)>) {
    let mut tx = begin_system(pool).await.unwrap();
    let queues = sqlx::query_as(
        "SELECT target_branch, config_text IS NOT NULL, config_sha FROM queues
         ORDER BY target_branch",
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap();
    let items = sqlx::query_as(
        "SELECT i.pr_number, q.target_branch FROM queue_items i JOIN queues q ON q.id = i.queue_id
         WHERE i.is_current ORDER BY i.pr_number",
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap();

    drop(tx);

    (queues, items)
}

/// A repository without a queue: the seeded queue is removed before any
/// event.
async fn without_queue(pool: &PgPool) {
    let mut tx = begin_system(pool).await.unwrap();
    sqlx::query("DELETE FROM queues WHERE id = $1")
        .bind(queue().to_string())
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
}

/// Without a file, the first command on the default branch creates its
/// queue; a label, or a command on another branch, creates none.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_command_creates_the_queue_of_the_default_branch(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    without_queue(&pool).await;

    let sim = q.sim.clone();
    let main = target_repo(&sim, &[("a.txt", "a")]);

    sim.push(&repo(), "release", &main, &alice()).unwrap();

    let fix = sim
        .commit(&repo(), &main, &[("fix.txt", Some("x"))])
        .unwrap();

    sim.push(&repo(), "fix", &fix, &bob()).unwrap();

    let release_pr = sim.open_pr(&repo(), "fix", "release", &bob()).unwrap();
    q.deliver().await;
    q.settle().await;

    let (labelled, head) = open_pr_with(&sim, &main, 1, &[("f1.txt", Some("x"))]);
    sim.report_check(&repo(), &head, check(REQUIRED, true))
        .unwrap();

    sim.add_label(&repo(), labelled, "ready-to-merge", JobClass::Interactive)
        .await
        .unwrap();

    q.deliver().await;
    q.settle().await;
    assert_eq!(queues_and_items(&pool).await, (Vec::new(), Vec::new()));

    sim.comment(&repo(), labelled, &alice(), "/cihub merge")
        .unwrap();

    let (other, head) = open_pr_with(&sim, &main, 2, &[("f2.txt", Some("x"))]);
    sim.report_check(&repo(), &head, check(REQUIRED, true))
        .unwrap();

    sim.comment(&repo(), other, &alice(), "/cihub merge")
        .unwrap();

    sim.comment(&repo(), release_pr, &alice(), "/cihub merge")
        .unwrap();

    q.deliver().await;
    q.settle().await;

    assert_eq!(
        queues_and_items(&pool).await,
        (
            vec![("main".into(), false, Some(main))],
            vec![
                (i64::try_from(labelled).unwrap(), "main".into()),
                (i64::try_from(other).unwrap(), "main".into())
            ]
        )
    );

    let refused = sim.comments(&repo(), release_pr);
    assert!(
        refused
            .iter()
            .any(|body| body.contains("no queue serves the branch `release`")),
        "{refused:?}"
    );

    q.stop().await;
}

/// The first command on a branch that a pattern of the file matches creates
/// the queue of that branch, with the file, so that the command reads its
/// conditions.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_command_creates_the_queue_that_the_file_names(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    without_queue(&pool).await;

    let sim = q.sim.clone();
    let config = "version: 1
queues:
  - branch: \"release*\"
    strategy: sequential
";
    let main = target_repo(&sim, &[("a.txt", "a"), (".cihub.yaml", config)]);

    sim.push(&repo(), "release", &main, &alice()).unwrap();
    let head = sim
        .commit(&repo(), &main, &[("f1.txt", Some("x"))])
        .unwrap();

    sim.push(&repo(), "pr-1", &head, &bob()).unwrap();
    let number = sim.open_pr(&repo(), "pr-1", "release", &bob()).unwrap();
    sim.report_check(&repo(), &head, check(REQUIRED, true))
        .unwrap();

    sim.comment(&repo(), number, &alice(), "/cihub merge")
        .unwrap();

    q.deliver().await;
    q.settle().await;

    assert_eq!(
        queues_and_items(&pool).await,
        (
            vec![("release".into(), true, Some(main))],
            vec![(1, "release".into())]
        )
    );

    q.stop().await;
}

/// With `direct_merge_mode: always`, a green PR that contains the head of
/// the target merges without a test branch; a PR that does not contain it
/// goes through the tests.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn an_up_to_date_pr_merges_directly(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let config = format!(
        "version: 1\nqueues:\n  - branch: main\n    required_statuses: [{REQUIRED}]\n    direct_merge_mode: always\n"
    );
    let main = target_repo(&sim, &[("a.txt", "a"), (".cihub.yaml", &config)]);

    q.deliver().await;
    q.settle().await;

    let (up_to_date, head) = open_pr_with(&sim, &main, 1, &[("f1.txt", Some("x"))]);
    sim.report_check(&repo(), &head, check(REQUIRED, true))
        .unwrap();

    sim.comment(&repo(), up_to_date, &alice(), "/cihub merge")
        .unwrap();

    q.deliver().await;
    q.settle().await;

    let (behind, head) = open_pr_with(&sim, &main, 2, &[("f2.txt", Some("x"))]);
    sim.report_check(&repo(), &head, check(REQUIRED, true))
        .unwrap();

    sim.comment(&repo(), behind, &alice(), "/cihub merge")
        .unwrap();

    q.deliver().await;
    q.settle().await;

    let mut tx = begin_system(&pool).await.unwrap();
    let items: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT pr_number, state::text, state_reason::text FROM queue_items
         WHERE is_current ORDER BY pr_number",
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap();
    let runs: Vec<Vec<i64>> = sqlx::query_scalar("SELECT tested_pr_numbers FROM test_runs")
        .fetch_all(&mut *tx)
        .await
        .unwrap();

    drop(tx);

    assert_eq!(
        items[0],
        (1, "merged".into(), "merge_graph_direct_merge".into())
    );
    assert_eq!(items[1].0, 2);
    assert_ne!(items[1].1, "merged", "{items:?}");
    assert_eq!(runs, [vec![2]], "only the PR behind its target is tested");
    q.stop().await;
}

/// Each PR shows its item: one comment of the queue, edited in place, the
/// status of the queue on its head, and the state labels; a merged PR
/// keeps its last state.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn each_pr_shows_its_item_in_one_comment_a_status_and_labels(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let config = format!(
        "version: 1\nqueues:\n  - branch: main\n    required_statuses: [{REQUIRED}]\n    direct_merge_mode: always\n    state_labels_enabled: true\n"
    );
    let main = target_repo(&sim, &[("a.txt", "a"), (".cihub.yaml", &config)]);

    q.deliver().await;
    q.settle().await;

    let (first, first_head) = open_pr_with(&sim, &main, 1, &[("f1.txt", Some("x"))]);
    sim.report_check(&repo(), &first_head, check(REQUIRED, true))
        .unwrap();

    sim.comment(&repo(), first, &alice(), "/cihub merge")
        .unwrap();

    q.deliver().await;
    q.settle().await;

    let (second, second_head) = open_pr_with(&sim, &main, 2, &[("f2.txt", Some("x"))]);
    sim.report_check(&repo(), &second_head, check(REQUIRED, true))
        .unwrap();

    sim.comment(&repo(), second, &alice(), "/cihub merge")
        .unwrap();

    q.deliver().await;
    q.settle().await;

    let own = cihub_mergequeue::queue_status_context("main");
    let shown = |number: u64| {
        sim.comments(&repo(), number)
            .into_iter()
            .filter(|body| is_status_comment(body))
            .collect::<Vec<_>>()
    };
    let status = |head: String| {
        let sim = sim.clone();
        let own = own.clone();
        async move {
            sim.checks(&repo(), &head, JobClass::Repair)
                .await
                .unwrap()
                .into_iter()
                .find(|check| check.name == own)
                .map(|check| check.conclusion)
        }
    };
    let labels = |number: u64| {
        let sim = sim.clone();
        async move {
            let pr = sim
                .pull_request(&repo(), number, JobClass::Repair)
                .await
                .unwrap();
            pr.labels
                .into_iter()
                .filter(|label| label.starts_with("cihub:"))
                .collect::<Vec<_>>()
        }
    };
    let merged = shown(first);

    assert_eq!(merged.len(), 1, "one comment, edited in place: {merged:?}");
    assert!(
        merged[0].starts_with("✅ **Merged** into `main` by the queue"),
        "{merged:?}"
    );

    for row in [
        "⏳ Waiting: the PR was submitted to the queue | `@alice` |",
        "✅ Merged: the queue merged the PR without a test branch, as ",
    ] {
        assert!(merged[0].contains(row), "{row}: {merged:?}");
    }

    assert_eq!(
        status(first_head).await,
        Some(Some(CheckConclusion::Success))
    );
    assert_eq!(labels(first).await, Vec::<String>::new());

    let testing = shown(second);
    assert_eq!(testing.len(), 1, "{testing:?}");
    assert!(
        testing[0].starts_with("🧪 **Testing** · position 1 in the queue for `main`"),
        "{testing:?}"
    );
    assert!(testing[0].contains(" in #"), "{testing:?}");
    assert!(!testing[0].contains("cihub-merge/"), "{testing:?}");
    assert!(
        testing[0].contains("| 🧪 Test branch ready: the CI runs in #"),
        "{testing:?}"
    );
    assert_eq!(status(second_head).await, Some(None), "pending");
    assert_eq!(labels(second).await, ["cihub:testing"]);
    q.stop().await;
}

/// `/cihub restart` and `/cihub cancel`: an account with `write` restarts the
/// tests of an item in test, then cancels it; an account without it gets a
/// comment that mentions it and says why.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_comment_restarts_or_cancels_the_item_of_its_pr(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let config = format!(
        "version: 1\nqueues:\n  - branch: main\n    required_statuses: [{REQUIRED}]\n    direct_merge_mode: off\n"
    );
    let main = target_repo(&sim, &[("a.txt", "a"), (".cihub.yaml", &config)]);

    q.deliver().await;
    q.settle().await;

    let (number, head) = open_pr_with(&sim, &main, 1, &[("f1.txt", Some("x"))]);
    sim.report_check(&repo(), &head, check(REQUIRED, true))
        .unwrap();

    sim.comment(&repo(), number, &alice(), "/cihub merge")
        .unwrap();

    q.deliver().await;
    q.settle().await;

    let item = |pool: PgPool| async move {
        let mut tx = begin_system(&pool).await.unwrap();
        sqlx::query_as::<_, (String, String)>(
            "SELECT state::text, state_reason::text FROM queue_items WHERE pr_number = 1",
        )
        .fetch_one(&mut *tx)
        .await
        .unwrap()
    };

    assert_eq!(item(pool.clone()).await.0, "testing");

    sim.comment(&repo(), number, &bob(), "/cihub cancel")
        .unwrap();

    q.deliver().await;
    q.settle().await;
    assert_eq!(item(pool.clone()).await.0, "testing", "bob cannot write");

    let refusal = "> /cihub cancel\n\n@bob, the queue refused this command: \
                   the command needs the write permission on the repository.\n";
    let refusals: Vec<String> = sim
        .comments(&repo(), number)
        .into_iter()
        .filter(|body| body == refusal)
        .collect();

    assert_eq!(refusals.len(), 1, "{refusals:?}");

    sim.comment(&repo(), number, &alice(), "/cihub restart")
        .unwrap();

    q.deliver().await;
    q.settle().await;

    let mut tx = begin_system(&pool).await.unwrap();
    let restarts: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM events WHERE payload->>'reason' = 'tests_restarted_by_user'",
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    let runs: i64 = sqlx::query_scalar("SELECT count(*) FROM test_runs")
        .fetch_one(&mut *tx)
        .await
        .unwrap();

    drop(tx);

    assert!(
        restarts > 0,
        "the decisions of the restart carry its reason"
    );
    assert_eq!(runs, 2, "a new run");
    assert_eq!(item(pool.clone()).await.0, "testing");

    sim.comment(&repo(), number, &alice(), "/cihub cancel")
        .unwrap();

    q.deliver().await;
    q.settle().await;

    assert_eq!(
        item(pool.clone()).await,
        ("cancelled".into(), "user_requested".into())
    );
    assert_eq!(
        sim.acknowledged_comments(&repo(), number),
        ["/cihub merge", "/cihub restart", "/cihub cancel"]
    );

    q.stop().await;
}

/// The comment shows the history of the PR in the queue: the login of a user
/// who sent a command is in a code span, and the rendering in `pr_surfaces` is
/// the one that the SCM shows.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_comment_shows_the_history_of_the_pr(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let config = format!(
        "version: 1\nqueues:\n  - branch: main\n    required_statuses: [{REQUIRED}]\n    direct_merge_mode: off\n"
    );
    let main = target_repo(&sim, &[("a.txt", "a"), (".cihub.yaml", &config)]);

    q.deliver().await;
    q.settle().await;

    let (number, head) = open_pr_with(&sim, &main, 1, &[("f1.txt", Some("x"))]);
    sim.report_check(&repo(), &head, check(REQUIRED, true))
        .unwrap();

    for command in ["/cihub merge", "/cihub restart"] {
        sim.comment(&repo(), number, &alice(), command).unwrap();
        q.deliver().await;
        q.settle().await;
    }

    let mut tx = begin_system(&pool).await.unwrap();
    let desired: String =
        sqlx::query_scalar("SELECT desired->>'comment' FROM pr_surfaces WHERE pr_number = 1")
            .fetch_one(&mut *tx)
            .await
            .unwrap();

    drop(tx);
    assert!(desired.contains("<details><summary>History ("), "{desired}");

    for row in [
        "⏳ Waiting: the PR was submitted to the queue | `@alice` |",
        "🕓 Queued: a user restarted the tests | `@alice` |",
    ] {
        assert!(desired.contains(row), "{row}: {desired}");
    }

    let shown: Vec<String> = sim
        .comments(&repo(), number)
        .into_iter()
        .filter(|body| is_status_comment(body))
        .collect();

    assert_eq!(shown, [desired], "the SCM shows the last rendering");
    q.stop().await;
}

/// The history covers every item of the PR, in the order of the log: a PR
/// cancelled then submitted again shows both.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_history_covers_every_item_of_the_pr(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let config = format!(
        "version: 1\nqueues:\n  - branch: main\n    required_statuses: [{REQUIRED}]\n    direct_merge_mode: off\n"
    );
    let main = target_repo(&sim, &[("a.txt", "a"), (".cihub.yaml", &config)]);

    q.deliver().await;
    q.settle().await;

    let (number, head) = open_pr_with(&sim, &main, 1, &[("f1.txt", Some("x"))]);
    sim.report_check(&repo(), &head, check(REQUIRED, true))
        .unwrap();

    for command in ["/cihub merge", "/cihub cancel", "/cihub merge"] {
        sim.comment(&repo(), number, &alice(), command).unwrap();
        q.deliver().await;
        q.settle().await;
    }

    let mut tx = begin_system(&pool).await.unwrap();
    let items: i64 = sqlx::query_scalar("SELECT count(*) FROM queue_items WHERE pr_number = 1")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    let desired: String =
        sqlx::query_scalar("SELECT desired->>'comment' FROM pr_surfaces WHERE pr_number = 1")
            .fetch_one(&mut *tx)
            .await
            .unwrap();

    drop(tx);
    assert_eq!(items, 2);

    let submitted = "⏳ Waiting: the PR was submitted to the queue | `@alice` |";
    let cancelled = "⚪ Cancelled: a user asked for it | `@alice` |";
    let rows: Vec<&str> = desired
        .lines()
        .filter(|line| line.contains(submitted) || line.contains(cancelled))
        .collect();

    assert_eq!(rows.len(), 3, "{desired}");
    assert!(rows[0].contains(submitted), "{desired}");
    assert!(rows[1].contains(cancelled), "{desired}");
    assert!(rows[2].contains(submitted), "{desired}");
    q.stop().await;
}

/// Sends `method path` with `token` and a JSON `body`, and returns the
/// status and the JSON of the answer.
async fn call(
    addr: &str,
    token: &str,
    method: &str,
    path: &str,
    body: Option<serde_json::Value>,
) -> (u16, serde_json::Value) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let body = body.map(|b| b.to_string()).unwrap_or_default();
    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let request = format!(
        "{method} /api/v1{path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\
         Authorization: Bearer {token}\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\n\r\n{body}",
        body.len()
    );

    stream.write_all(request.as_bytes()).await.unwrap();

    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();

    let (head, body) = response.split_once("\r\n\r\n").unwrap();
    let status = head.split(' ').nth(1).unwrap().parse().unwrap();

    (
        status,
        serde_json::from_str(body).unwrap_or(serde_json::Value::Null),
    )
}

/// Every entry of the list at `path`, read one page of one entry at a time:
/// the pages hold the list in order, each entry once.
async fn every_page(addr: &str, token: &str, path: &str, entries: &str) -> Vec<Value> {
    let separator = if path.contains('?') { '&' } else { '?' };
    let mut every = Vec::new();
    let mut from = String::new();

    loop {
        let page_path = format!("{path}{separator}limit=1{from}");
        let (status, page) = call(addr, token, "GET", &page_path, None).await;
        assert_eq!(status, 200, "{page_path}: {page}");
        every.extend(page[entries].as_array().unwrap().iter().cloned());
        let Some(cursor) = page["cursor"].as_str() else {
            return every;
        };
        from = format!("&from={cursor}");
    }
}

/// The API v1 reads and changes a queue, named by the organization, the
/// repository and the queue: a submission, its item, its timeline and its run,
/// the graph, a restart and a cancel by PR number on the current item, a
/// pause, a resync and the audit log.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_api_reads_and_changes_a_queue(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let config = format!(
        "version: 1\nqueues:\n  - branch: main\n    required_statuses: [{REQUIRED}]\n    direct_merge_mode: off\n"
    );
    let main = target_repo(&sim, &[("a.txt", "a"), (".cihub.yaml", &config)]);

    q.deliver().await;
    q.settle().await;

    let (addr, token) = q.serve_api().await;
    let queue = "/orgs/acme/repos/app/queues/main";
    let call = |method: &'static str, path: String, body: Option<serde_json::Value>| {
        let (addr, token) = (addr.clone(), token.clone());
        async move { call(&addr, &token, method, &path, body).await }
    };

    let (number, head) = open_pr_with(&sim, &main, 1, &[("f1.txt", Some("x"))]);
    let pr_path = format!("/orgs/acme/repos/app/prs/{number}");
    let prs = "/orgs/acme/prs?repo=app&queue=main";

    sim.report_check(&repo(), &head, check(REQUIRED, true))
        .unwrap();

    q.deliver().await;
    q.settle().await;

    let submitted = call(
        "POST",
        format!("{pr_path}/submit"),
        Some(serde_json::json!({ "priority": "high" })),
    )
    .await;

    assert_eq!(
        submitted,
        (202, Value::Null),
        "no body: the queue acts later"
    );
    q.settle().await;

    let (status, twice) = call(
        "POST",
        format!("{pr_path}/submit"),
        Some(serde_json::json!({})),
    )
    .await;
    assert_eq!(
        (status, &twice["code"]),
        (409, &serde_json::json!("pr_already_queued")),
        "the PR waits in the queue: {twice}"
    );

    sqlx::query(
        "UPDATE users SET avatar_url = 'https://avatars.example.com/u/2' WHERE login = 'bob'",
    )
    .execute(&pool)
    .await
    .unwrap();
    let (status, listed) = call("GET", prs.to_owned(), None).await;
    let (live_run, _, _) = q.runs_in_progress().await.remove(0);
    let pr = &listed["prs"][0];

    assert_eq!(status, 200, "{listed}");
    assert_eq!(
        (&listed["count"], &listed["has_more"], &listed["cursor"]),
        (&1.into(), &false.into(), &Value::Null)
    );
    assert_eq!(pr["number"], number);
    assert_eq!((&pr["repo"], &pr["queue"]), (&"app".into(), &"main".into()));
    assert_eq!(pr["state"], "testing");
    assert_eq!(pr["priority"], 10);
    assert_eq!(
        pr["title"], "pr-1",
        "the simulator names a PR after its branch"
    );
    assert_eq!(
        pr["author"],
        serde_json::json!({ "login": "bob", "avatar_url": "https://avatars.example.com/u/2" })
    );
    assert_eq!(pr["run_id"], live_run.to_string().as_str());
    assert_eq!(pr["no_batch"], false);
    assert_eq!(pr["submitted_via"], "api");
    assert_eq!(
        pr["submitted_by"],
        Value::Null,
        "a token stands for no person"
    );

    let live = call("GET", format!("{prs}&state=pending,testing"), None).await;
    let merged = call("GET", format!("{prs}&state=merged"), None).await;
    let unknown = call("GET", format!("{prs}&state=testing,lost"), None).await;
    let without_repo = call("GET", "/orgs/acme/prs?queue=main".into(), None).await;
    let me = call("GET", "/orgs/acme/prs?author=@me".into(), None).await;
    let bob = call("GET", "/orgs/acme/prs?author=BOB".into(), None).await;

    assert_eq!(live.1["prs"].as_array().unwrap().len(), 1, "{}", live.1);
    assert_eq!(merged.1["prs"], serde_json::json!([]));
    assert_eq!(
        (unknown.0, &unknown.1["code"]),
        (400, &serde_json::json!("invalid_request"))
    );
    assert_eq!(without_repo.1["code"], "queue_needs_repo");
    assert_eq!(
        me.1["code"], "me_needs_session",
        "a token stands for no person"
    );
    assert_eq!(bob.1["count"], 1, "the case of a login does not matter");

    let item_id = pr["id"].as_str().unwrap().to_owned();
    let (_, read) = call("GET", pr_path.clone(), None).await;
    let (_, events) = call("GET", format!("{pr_path}/events"), None).await;

    assert_eq!(read["id"], item_id.as_str());
    assert_eq!(events["events"][0]["type"], "item.transitioned");
    assert_eq!(events["events"][0]["item_id"], item_id.as_str());

    let (_, graph) = call("GET", format!("{queue}/graph"), None).await;
    let run = &graph["runs"][0];

    assert_eq!(graph["nodes"][0]["pr_number"], number);
    assert_eq!(graph["target_head_sha"], main.as_str());
    assert_eq!(run["run_id"], live_run.to_string().as_str());
    assert_eq!(run["item_ids"], serde_json::json!([item_id]));
    assert_eq!(run["predecessor_ids"], serde_json::json!([]));
    assert_eq!(
        (&run["attempt"], &run["status"]),
        (&1.into(), &"in_progress".into())
    );

    let (run_id, _, _) = q.runs_in_progress().await.remove(0);
    let (status, run) = call("GET", format!("/orgs/acme/runs/{run_id}"), None).await;

    assert_eq!(status, 200, "{run}");
    assert_eq!(run["tested_pr_numbers"], serde_json::json!([number]));
    assert_eq!(run["status"], "in_progress");

    let restart = call("POST", format!("{pr_path}/restart"), None).await;
    assert_eq!(restart.0, 202);
    q.settle().await;

    let cancel = call("POST", format!("{pr_path}/cancel"), None).await;
    assert_eq!(cancel.0, 202);
    q.settle().await;

    let (_, pr) = call("GET", pr_path.clone(), None).await;
    assert_eq!(pr["state"], "cancelled");
    assert_eq!(pr["state_reason"], "admin_requested");
    assert_eq!(pr["state_description"], "an administrator asked for it");

    let (_, events) = call("GET", format!("{pr_path}/events"), None).await;
    let asked = events["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|event| event["type"] == "item.cancel_requested")
        .unwrap();

    assert_eq!(asked["actor"], Value::Null, "a token stands for no person");
    assert_eq!(asked["description"], "a cancellation was asked");
    assert_eq!(
        every_page(&addr, &token, &format!("{pr_path}/events"), "events").await,
        events["events"].as_array().unwrap().clone(),
        "the pages of the events hold them all, in order"
    );

    let (status, again) = call("POST", format!("{pr_path}/cancel"), None).await;
    assert_eq!(
        (status, &again["code"]),
        (409, &serde_json::json!("item_ended"))
    );

    let paused = call(
        "PATCH",
        queue.to_owned(),
        Some(serde_json::json!({ "state": "paused" })),
    )
    .await;

    assert_eq!(paused, (202, Value::Null));
    q.settle().await;

    let (_, read) = call("GET", queue.to_owned(), None).await;
    assert_eq!(read["state"], "paused");
    assert_eq!(
        read["state_changed_by"],
        Value::Null,
        "a token stands for no person"
    );
    assert_eq!(read["repo"], "app");
    assert_eq!(read["strategy"], "pipelined");
    assert_eq!(read["target_head_sha"], main.as_str());
    assert_eq!(read["access"], "ok");
    assert_eq!(read["has_config_file"], true);
    assert_eq!(read["counts"]["testing"], 0);
    assert_eq!(read["live_runs"], 0);
    assert_eq!(
        read["window"],
        serde_json::json!({ "current": 2, "floor": 2, "ceiling": 20 })
    );
    assert_eq!(read["settings"]["merge_method"], "squash");
    assert_eq!(
        read["settings"]["enqueue_label"],
        "cihub-merge-queue-submit"
    );
    assert_eq!(
        read["config_check"],
        serde_json::json!({ "findings": [], "checked_at": null })
    );

    let (_, queues) = call("GET", "/orgs/acme/repos/app/queues".into(), None).await;
    assert_eq!(queues["queues"].as_array().unwrap().len(), 1);
    assert_eq!(queues["count"], 1);

    let before_resync = sim.calls().len();
    assert_eq!(call("POST", format!("{queue}/resync"), None).await.0, 202);
    q.settle().await;

    let (status, checked) = call("GET", queue.to_owned(), None).await;
    let calls = sim.calls();
    let (_, read_again) = call("GET", queue.to_owned(), None).await;

    assert_eq!(status, 200, "{checked}");
    assert_eq!(
        checked["config_check"]["findings"][0]["code"], "update_restriction_missing",
        "the simulator restricts no update"
    );
    assert!(
        calls[before_resync..]
            .iter()
            .any(|call| call.name == "check_setup"),
        "the resync checks the configuration"
    );
    assert_eq!(read_again["config_check"], checked["config_check"]);
    assert_eq!(
        sim.calls().len(),
        calls.len(),
        "a read calls the SCM zero times"
    );

    let (_, audit) = call("GET", "/orgs/acme/audit".into(), None).await;
    let actions: Vec<&str> = audit["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["action"].as_str().unwrap())
        .collect();

    assert_eq!(
        actions,
        [
            "queue.resync",
            "queue.state_requested",
            "item.cancel_requested",
            "item.restart_requested"
        ]
    );
    assert_eq!(audit["entries"][2]["target"], item_id.as_str());
    assert_eq!(
        audit["entries"][2]["after"],
        serde_json::json!({ "queue_id": crate::queue().to_string(), "pr_number": number })
    );

    let (_, first) = call("GET", "/orgs/acme/audit?limit=3".into(), None).await;
    let next = first["cursor"].as_str().unwrap();
    let (_, rest) = call("GET", format!("/orgs/acme/audit?limit=3&from={next}"), None).await;
    let foreign = call("GET", format!("{prs}&from={next}"), None).await;

    assert_eq!(
        (&first["has_more"], &first["count"]),
        (&true.into(), &4.into())
    );
    assert_eq!(rest["entries"][0]["action"], "item.restart_requested");
    assert_eq!(
        (&rest["has_more"], &rest["cursor"]),
        (&false.into(), &Value::Null)
    );
    assert_eq!(
        foreign.1["code"], "invalid_cursor",
        "a cursor of another list is refused"
    );

    let client =
        cihub_client::Client::new(&format!("http://{addr}"), SecretString::from(token.clone()))
            .unwrap();
    let repo_path = cihub_client::RepoPath {
        org: "acme",
        repo: "app",
    };
    let queues = client.queues(repo_path).await.unwrap().queues;

    assert_eq!(queues.len(), 1);

    let read = client.pr(repo_path, number).await.unwrap();
    assert_eq!(read.state, ItemState::Cancelled);

    let refused = client.restart(repo_path, number).await.unwrap_err();
    assert!(
        matches!(&refused, cihub_client::ClientError::Api(problem) if problem.code == cihub_api_types::ErrorCode::ItemNotInTest),
        "{refused}"
    );

    // A target branch that no longer exists is a finding of the check.
    sim.delete_branch(&repo(), "main", JobClass::Interactive)
        .await
        .unwrap();
    q.clock.advance(1);
    assert_eq!(call("POST", format!("{queue}/resync"), None).await.0, 202);
    q.settle().await;

    let (_, checked) = call("GET", queue.to_owned(), None).await;
    assert_eq!(
        checked["config_check"]["findings"],
        serde_json::json!([{
            "code": "target_branch_missing",
            "message": "the branch `main` no longer exists: the queue cannot merge"
        }])
    );

    q.stop().await;
}

/// An item gives the title of the PR at its last read, and the person who
/// submitted it; the page of the queue calls the SCM zero times (principle
/// VII), and a replay of the log gives the same answers (principle I).
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_page_of_a_queue_follows_its_prs_and_survives_a_replay(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let config = format!(
        "version: 1\nqueues:\n  - branch: main\n    required_statuses: [{REQUIRED}]\n    direct_merge_mode: off\n"
    );
    let main = target_repo(&sim, &[("a.txt", "a"), (".cihub.yaml", &config)]);

    q.deliver().await;
    q.settle().await;

    let (addr, token) = q.serve_api().await;
    let prs = "/orgs/acme/prs?repo=app&queue=main";
    let (number, head) = open_pr_with(&sim, &main, 1, &[("f1.txt", Some("x"))]);

    sim.report_check(&repo(), &head, check(REQUIRED, true))
        .unwrap();
    sim.comment(&repo(), number, &alice(), "/cihub merge")
        .unwrap();
    q.deliver().await;
    q.settle().await;

    let (_, submitted) = call(&addr, &token, "GET", prs, None).await;
    let pr = &submitted["prs"][0];

    assert_eq!(pr["title"], "pr-1");
    assert_eq!(pr["submitted_by"], "alice");
    assert_eq!(pr["submitted_via"], "comment");
    assert!(pr["run_id"].is_string(), "{pr}");

    let events = format!("/orgs/acme/repos/app/prs/{number}/events");
    let read = call(&addr, &token, "GET", &events, None).await.1;
    let first = &read["events"][0];

    assert_eq!(first["actor"], "alice", "the actor of the cause: {read}");
    assert_eq!(first["description"], "the PR was submitted to the queue");

    let page = format!("https://cihub.example.com/orgs/acme/repos/app/queues/main/prs/{number}");
    let comment = sim.comments(&repo(), number).pop().unwrap_or_default();
    assert!(
        comment.contains(&format!("[Open in CIHub]({page})")),
        "{comment}"
    );

    sim.change_pr(&repo(), number, PrChange::Title("Fix the login redirect"))
        .unwrap();
    q.deliver().await;
    q.settle().await;

    let (_, renamed) = call(&addr, &token, "GET", prs, None).await;
    assert_eq!(renamed["prs"][0]["title"], "Fix the login redirect");

    // The reads of the cards and of the page of the queue.
    let page = || async {
        let queue = "/orgs/acme/repos/app/queues/main";
        let reads = [
            "/orgs/acme/queues".to_owned(),
            "/orgs/acme/prs?author=bob&sort=position".to_owned(),
            format!("/orgs/acme/repos/app/prs/{number}"),
            events.clone(),
            queue.to_owned(),
            format!("{queue}/graph"),
            format!(
                "{prs}&state=not_ready,pending,testing,pending_failure,tests_passed&sort=position&limit=100"
            ),
        ];
        let mut answers = Vec::new();
        for path in reads {
            answers.push(call(&addr, &token, "GET", &path, None).await.1);
        }
        answers
    };
    let calls = sim.calls().len();
    let before = page().await;

    assert_eq!(sim.calls().len(), calls, "a page calls the SCM zero times");

    let mut tx = begin_system(&pool).await.unwrap();
    rebuild(&mut tx, queue()).await.unwrap();
    tx.commit().await.unwrap();

    assert_eq!(page().await, before);
    q.stop().await;
}

/// `GET /orgs/{org}/prs?author=` gives the current items of a person with
/// the PRs ahead of each one and an estimate, and the queue gives the median
/// duration of its runs.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_person_follows_their_prs_until_the_merge(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let main = target_repo(&sim, &[("a.txt", "a"), (".cihub.yaml", TEST_PATH)]);

    q.deliver().await;
    q.settle().await;

    let (addr, token) = q.serve_api().await;
    let get = |path: String| {
        let (addr, token) = (addr.clone(), token.clone());
        async move { call(&addr, &token, "GET", &path, None).await.1 }
    };

    let mut numbers = Vec::new();

    for n in 1..=2 {
        numbers.push(submit_new_pr(&sim, &main, n));
        q.deliver().await;
        q.settle().await;
    }

    let (first, second) = (numbers[0], numbers[1]);
    let mine = get("/orgs/acme/prs?author=BOB&sort=position".into()).await;
    let items: Vec<(u64, Value)> = mine["prs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|pr| (pr["number"].as_u64().unwrap(), pr["ahead"].clone()))
        .collect();

    assert_eq!(mine["prs"][0]["repo"], "app");
    assert_eq!(mine["prs"][0]["queue"], "main");
    assert_eq!(
        items,
        [
            (first, serde_json::json!([])),
            (
                second,
                serde_json::json!([{ "pr_number": first, "state": "testing" }])
            ),
        ],
        "the login is read whatever its case"
    );
    assert_eq!(
        get("/orgs/acme/prs?author=alice".into()).await["prs"],
        serde_json::json!([]),
        "alice submitted the PRs of bob"
    );

    let (_, test_sha, _) = q.runs_in_progress().await.remove(0);
    sim.report_check(&repo(), &test_sha, check(REQUIRED, true))
        .unwrap();
    q.deliver().await;
    q.settle().await;
    q.deliver().await;
    q.settle().await;

    let mine = get("/orgs/acme/prs?author=bob&sort=position".into()).await;
    let queue = "/orgs/acme/repos/app/queues/main";
    let prs = "/orgs/acme/prs?repo=app&queue=main";
    let merged = format!("{prs}&state=merged&state_changed_since=2026-10-01T09:00:00Z");
    let later = format!("{prs}&state=merged&state_changed_since=2026-10-02T00:00:00Z");

    assert_eq!(
        mine["prs"][1]["state"], "merged",
        "an ended PR comes after the live ones: {mine}"
    );
    assert_eq!(mine["prs"][1]["ahead"], serde_json::json!([]));
    assert_eq!(mine["prs"][0]["ahead"], serde_json::json!([]));
    assert!(get(queue.to_owned()).await["median_run_seconds"].is_u64());
    assert!(
        mine["prs"][0]["merges_in_seconds"].is_u64(),
        "a tested PR merges in about the rest of its run: {mine}"
    );
    assert_eq!(mine["prs"][1]["merges_in_seconds"], Value::Null);
    assert_eq!(get(merged).await["prs"].as_array().unwrap().len(), 1);
    assert_eq!(get(later).await["prs"], serde_json::json!([]));

    // A PR that is not ready has every admitted PR ahead of it; a cancelled PR
    // leaves the PRs that are live or merged.
    let mine_live = "/orgs/acme/prs?author=bob&sort=position\
                     &state=not_ready,pending,testing,pending_failure,tests_passed,merged";
    let (third, _) = open_pr_with(&sim, &main, 3, &[("f3.txt", Some("x"))]);
    sim.comment(&repo(), third, &alice(), "/cihub merge")
        .unwrap();
    q.deliver().await;
    q.settle().await;

    let mine = get(mine_live.into()).await;
    assert_eq!(mine["prs"][1]["state"], "not_ready", "{mine}");
    assert_eq!(
        mine["prs"][1]["ahead"],
        serde_json::json!([{ "pr_number": second, "state": "testing" }])
    );

    // Each filter writes its own SQL: the organization alone, a repository
    // alone, a state, an author.
    let filters = [
        ("", 3),
        ("repo=app&", 3),
        ("state=merged&", 1),
        ("author=bob&", 3),
    ];
    for (filter, count) in filters {
        for sort in ["newest", "position"] {
            let path = format!("/orgs/acme/prs?{filter}sort={sort}");
            let whole = get(path.clone()).await;
            let paged = every_page(&addr, &token, &path, "prs").await;

            assert_eq!(&paged, whole["prs"].as_array().unwrap(), "{path}");
            assert_eq!(whole["count"], count, "{path}");
        }
    }

    let cancel = format!("/orgs/acme/repos/app/prs/{second}/cancel");
    assert_eq!(call(&addr, &token, "POST", &cancel, None).await.0, 202);
    q.settle().await;

    let mine = get(mine_live.into()).await;
    let numbers: Vec<u64> = mine["prs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|pr| pr["number"].as_u64().unwrap())
        .collect();
    assert_eq!(numbers, [third, first]);
    assert_eq!(mine["prs"][0]["ahead"], serde_json::json!([]));
    q.stop().await;
}

/// A reconciliation that writes the log tells the pages that its queue
/// changed, with the last sequence number of the log.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_reconciliation_that_writes_the_log_tells_the_pages(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let main = target_repo(&sim, &[("a.txt", "a")]);
    let feed = cihub_platform::QueueNewsFeed::start(pool.clone(), q.cancel.clone())
        .await
        .unwrap();
    let mut news = feed.subscribe();

    submit_new_pr(&sim, &main, 1);
    q.deliver().await;
    q.settle().await;

    let heard = tokio::time::timeout(std::time::Duration::from_secs(5), news.recv())
        .await
        .unwrap()
        .unwrap();

    let cihub_platform::QueueNews::Changed(change) = heard else {
        panic!("{heard:?}");
    };
    assert_eq!((change.org_id, change.queue_id), (org(), queue()));
    assert!(change.seq > 0, "{change:?}");
    q.stop().await;
}

/// The graph gives each live run with the items that it tests on top of
/// their predecessors, the oldest run first.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_graph_gives_each_run_on_top_of_its_predecessors(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let config = format!(
        "version: 1\nqueues:\n  - branch: main\n    required_statuses: [{REQUIRED}]\n    direct_merge_mode: off\n    \
         failure_budget: {{ retries: 0 }}\n    pending_failure_depth: 1\n"
    );
    let main = target_repo(&sim, &[("a.txt", "a"), (".cihub.yaml", &config)]);

    q.deliver().await;
    q.settle().await;

    let (addr, token) = q.serve_api().await;

    for n in 1..=2 {
        let (number, head) = open_pr_with(&sim, &main, n, &[(&format!("f{n}.txt"), Some("x"))]);
        sim.report_check(&repo(), &head, check(REQUIRED, true))
            .unwrap();
        sim.comment(&repo(), number, &alice(), "/cihub merge")
            .unwrap();
        q.deliver().await;
        q.settle().await;
    }

    let queue = "/orgs/acme/repos/app/queues/main";
    let (_, graph) = call(&addr, &token, "GET", &format!("{queue}/graph"), None).await;
    let first = graph["nodes"][0]["item_id"].clone();
    let second = graph["nodes"][1]["item_id"].clone();
    let runs: Vec<(Value, Value)> = graph["runs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|run| (run["item_ids"].clone(), run["predecessor_ids"].clone()))
        .collect();

    assert_eq!(
        runs,
        [
            (serde_json::json!([first]), serde_json::json!([])),
            (serde_json::json!([second]), serde_json::json!([first])),
        ]
    );

    // The second run fails while the first one tests: its PR waits for it, and
    // the graph keeps the failed run.
    let (_, test_sha, _) = q.runs_in_progress().await.remove(1);
    sim.report_check(&repo(), &test_sha, check(REQUIRED, false))
        .unwrap();
    q.deliver().await;
    q.settle().await;

    let (_, graph) = call(&addr, &token, "GET", &format!("{queue}/graph"), None).await;

    assert_eq!(graph["nodes"][1]["state"], "pending_failure", "{graph}");
    assert_eq!(graph["runs"][1]["status"], "failed");
    assert_eq!(graph["runs"][1]["item_ids"], serde_json::json!([second]));
    q.stop().await;
}

/// Cancel and restart by PR number act on the current item of the PR, not on
/// an earlier one.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn cancel_and_restart_by_pr_number_act_on_the_current_item(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let config = format!(
        "version: 1\nqueues:\n  - branch: main\n    required_statuses: [{REQUIRED}]\n    direct_merge_mode: off\n"
    );
    let main = target_repo(&sim, &[("a.txt", "a"), (".cihub.yaml", &config)]);

    q.deliver().await;
    q.settle().await;

    let (addr, token) = q.serve_api().await;
    let pr_path = "/orgs/acme/repos/app/prs";
    let call = |method: &'static str, path: String, body: Option<serde_json::Value>| {
        let (addr, token) = (addr.clone(), token.clone());
        async move { call(&addr, &token, method, &path, body).await }
    };
    let (number, head) = open_pr_with(&sim, &main, 1, &[("f1.txt", Some("x"))]);
    let submit = || Some(serde_json::json!({}));

    sim.report_check(&repo(), &head, check(REQUIRED, true))
        .unwrap();
    q.deliver().await;
    q.settle().await;

    assert_eq!(
        call("POST", format!("{pr_path}/{number}/submit"), submit())
            .await
            .0,
        202
    );
    q.settle().await;
    let cancel = call("POST", format!("{pr_path}/{number}/cancel"), None).await;
    assert_eq!(cancel.0, 202, "{}", cancel.1);
    q.settle().await;
    assert_eq!(
        call("POST", format!("{pr_path}/{number}/submit"), submit())
            .await
            .0,
        202
    );
    q.settle().await;

    let restart = call("POST", format!("{pr_path}/{number}/restart"), None).await;
    assert_eq!(
        restart.0, 202,
        "the earlier item is cancelled: {}",
        restart.1
    );
    q.settle().await;

    let cancel = call("POST", format!("{pr_path}/{number}/cancel"), None).await;
    assert_eq!(cancel.0, 202, "{}", cancel.1);
    q.settle().await;

    let (_, read) = call("GET", format!("{pr_path}/{number}"), None).await;
    let (_, events) = call("GET", format!("{pr_path}/{number}/events"), None).await;
    let mut submissions: Vec<&str> = events["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["item_id"].as_str().unwrap())
        .collect();
    submissions.dedup();

    assert_eq!(read["state"], "cancelled");
    assert_eq!(
        submissions.len(),
        2,
        "the events of the PR hold its two submissions: {events}"
    );
    assert_eq!(submissions[1], read["id"]);

    let mut tx = begin_system(&pool).await.unwrap();
    let restarted: Vec<String> = sqlx::query_scalar(
        "SELECT item_id FROM events WHERE type = 'item.restart_requested' ORDER BY seq",
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap();

    drop(tx);
    assert_eq!(restarted, [read["id"].as_str().unwrap()]);
    q.stop().await;
}

/// The URL names the organization: a token answers 404 on another
/// organization, as on an unknown repository, queue, PR, run or action.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_api_answers_not_found_outside_the_organization_of_the_token(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let mut tx = begin_system(&pool).await.unwrap();

    sqlx::query("INSERT INTO orgs (id, slug) VALUES ($1, 'other')")
        .bind(OrgId::from_ulid(Ulid(50)).to_string())
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let (addr, token) = q.serve_api().await;
    let run = RunId::from_ulid(Ulid(51));
    let missing = [
        ("GET", "/orgs/other".to_owned(), "org_not_found"),
        ("GET", "/orgs/other/repos".to_owned(), "org_not_found"),
        ("GET", "/orgs/other/queues".to_owned(), "org_not_found"),
        ("GET", "/orgs/nobody/queues".to_owned(), "org_not_found"),
        ("GET", "/orgs/other/prs".to_owned(), "org_not_found"),
        (
            "GET",
            "/orgs/other/repos/app/queues".to_owned(),
            "org_not_found",
        ),
        (
            "GET",
            "/orgs/other/repos/app/queues/main".to_owned(),
            "org_not_found",
        ),
        (
            "POST",
            "/orgs/other/repos/app/prs/1/cancel".to_owned(),
            "org_not_found",
        ),
        ("GET", "/orgs/other/audit".to_owned(), "org_not_found"),
        ("GET", format!("/orgs/other/runs/{run}"), "org_not_found"),
        (
            "GET",
            "/orgs/nobody/repos/app/queues/main".to_owned(),
            "org_not_found",
        ),
        (
            "GET",
            "/orgs/acme/repos/unknown".to_owned(),
            "repo_not_found",
        ),
        (
            "GET",
            "/orgs/acme/repos/unknown/queues".to_owned(),
            "repo_not_found",
        ),
        (
            "GET",
            "/orgs/acme/prs?repo=unknown".to_owned(),
            "repo_not_found",
        ),
        (
            "GET",
            "/orgs/acme/repos/app/queues/unknown".to_owned(),
            "queue_not_found",
        ),
        (
            "GET",
            "/orgs/acme/prs?repo=app&queue=unknown".to_owned(),
            "queue_not_found",
        ),
        (
            "GET",
            "/orgs/acme/repos/app/prs/7".to_owned(),
            "pr_not_found",
        ),
        (
            "GET",
            "/orgs/acme/repos/app/prs/7/events".to_owned(),
            "pr_not_found",
        ),
        (
            "POST",
            "/orgs/acme/repos/app/prs/7/cancel".to_owned(),
            "pr_not_found",
        ),
        (
            "POST",
            "/orgs/acme/repos/app/prs/7/restart".to_owned(),
            "pr_not_found",
        ),
        (
            "POST",
            "/orgs/acme/repos/app/prs/7/merge".to_owned(),
            "route_not_found",
        ),
        (
            "POST",
            "/orgs/acme/repos/app/queues/main/delete".to_owned(),
            "route_not_found",
        ),
        ("GET", format!("/orgs/acme/runs/{run}"), "run_not_found"),
    ];

    for (method, path, code) in missing {
        let (status, problem) = call(&addr, &token, method, &path, None).await;

        assert_eq!(
            (status, &problem["code"]),
            (404, &serde_json::json!(code)),
            "{method} {path}: {problem}"
        );
    }

    let (status, found) = call(
        &addr,
        &token,
        "GET",
        "/orgs/ACME/repos/APP/queues/main",
        None,
    )
    .await;
    assert_eq!(
        status, 200,
        "the SCM does not tell names apart by their case: {found}"
    );

    let (_, document) = call(&addr, &token, "GET", "/openapi.json", None).await;
    let paths: Vec<&String> = document["paths"].as_object().unwrap().keys().collect();
    assert!(
        paths.iter().all(|path| path.starts_with("/orgs/{org}")
            || path.starts_with("/user")
            || path.starts_with("/device-codes")
            || *path == "/instance"),
        "only the person of a session, their sign-in and the instance are outside an \
         organization: {paths:?}"
    );
    assert!(
        paths.contains(&&"/orgs/{org}/repos/{repo}/prs/{number}/cancel".to_owned()),
        "{paths:?}"
    );
    assert!(
        paths.contains(&&"/orgs/{org}/queues".to_owned()),
        "{paths:?}"
    );

    q.stop().await;
}

/// Serves the API of the module without its loops, with the clock fixed at
/// `clock()`: no reconciliation changes the rows that a test writes.
async fn serve_module_api(pool: &PgPool, cancel: &CancellationToken) -> (String, String) {
    let scm: Arc<dyn Scm> = Arc::new(Sim::new());
    let deliveries = Arc::new(Mutex::new(HashMap::new()));
    let fixed: Clock = Arc::new(clock);
    let module = cihub_mergequeue::module(
        pool.clone(),
        scm,
        translate(&deliveries),
        "scheduler".into(),
        fixed,
        None,
    );

    serve_api(pool, module.api, cancel.clone()).await
}

/// An item of the PR `pr_number` in the queue `queue_id`, which reached
/// `state` at `changed_at`.
struct ItemFixture<'a> {
    queue_id: &'a str,
    pr_number: i64,
    state: &'a str,
    changed_at: OffsetDateTime,
    is_current: bool,
}

async fn insert_item(conn: &mut PgConnection, item: &ItemFixture<'_>) {
    let id = format!(
        "item-{}-{}-{}",
        item.queue_id, item.pr_number, item.is_current
    );

    sqlx::query(
        "INSERT INTO queue_items (id, org_id, queue_id, pr_number, pr_head_sha, pr_author_id,
                                  state, state_reason, state_changed_at, priority_value, no_batch,
                                  force_enqueued, submitted_via, submitted_at, block_reasons,
                                  is_current)
         VALUES ($1, $2, $3, $4, 'abc', $5, $6::item_state, 'submitted', $7, 100, false, false,
                 'api', $7, '[]', $8)",
    )
    .bind(id)
    .bind(org().to_string())
    .bind(item.queue_id)
    .bind(item.pr_number)
    .bind(user().to_string())
    .bind(item.state)
    .bind(item.changed_at)
    .bind(item.is_current)
    .execute(conn)
    .await
    .unwrap();
}

/// `GET /orgs/{org}/queues` gives every queue of the organization in the
/// order of the repository names, then of the queue names, with the current
/// items in each live state and the current items that merged, failed or were
/// cancelled in the 24 hours before the request.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_api_lists_the_queues_of_the_organization_with_their_counters(pool: PgPool) {
    seed(&pool).await;

    let now = clock();
    let app_main = queue().to_string();
    let app_release = QueueId::from_ulid(Ulid(60)).to_string();
    let api_main = QueueId::from_ulid(Ulid(62)).to_string();
    let mut tx = begin_system(&pool).await.unwrap();

    // The repository `api` comes from a second installation, suspended; a
    // person paused the queue `main` of `app`.
    sqlx::raw_sql(AssertSqlSafe(format!(
        "INSERT INTO scm_connections (id, org_id, account_external_id, kind, external_id,
                                      synced_at, permissions, suspended_at)
             VALUES ('{suspended}', '{org}', '2', 'github_app', '2', '{now}', '{{}}', '{now}');
         INSERT INTO repos (id, org_id, connection_id, external_id, namespace, name, default_branch)
             VALUES ('{api_repo}', '{org}', '{suspended}', '201', 'acme', 'api', 'main');
         INSERT INTO queues (id, org_id, repo_id, target_branch)
             VALUES ('{app_release}', '{org}', '{app_repo}', 'release'),
                    ('{api_main}', '{org}', '{api_repo}', 'main');
         UPDATE queues SET state = 'paused', state_changed_by = '{user}' WHERE id = '{app_main}';",
        suspended = ConnectionId::from_ulid(Ulid(63)),
        user = user(),
        api_repo = RepoId::from_ulid(Ulid(61)),
        app_repo = RepoId::from_ulid(Ulid(12)),
        org = org(),
    )))
    .execute(&mut *tx)
    .await
    .unwrap();

    let current = |queue_id, pr_number, state, hours_ago| ItemFixture {
        queue_id,
        pr_number,
        state,
        changed_at: now - Duration::hours(hours_ago),
        is_current: true,
    };
    let items = [
        current(&app_main, 1, "not_ready", 1),
        current(&app_main, 2, "pending", 1),
        current(&app_main, 3, "testing", 1),
        current(&app_main, 4, "testing", 1),
        current(&app_main, 5, "pending_failure", 1),
        current(&app_main, 6, "tests_passed", 1),
        current(&app_main, 7, "merged", 1),
        current(&app_main, 8, "merged", 23),
        current(&app_main, 9, "merged", 25),
        current(&app_main, 10, "failed", 2),
        current(&app_main, 11, "failed", 30),
        current(&app_main, 12, "cancelled", 1),
        current(&app_main, 14, "cancelled", 25),
        // The PR 13 failed, then came back: only its current item counts, as the
        // list of the PRs gives it.
        ItemFixture {
            is_current: false,
            ..current(&app_main, 13, "failed", 3)
        },
        current(&app_main, 13, "pending", 1),
        current(&api_main, 1, "merged", 26),
        current(&api_main, 2, "testing", 1),
    ];

    for item in &items {
        insert_item(&mut tx, item).await;
    }

    tx.commit().await.unwrap();

    let cancel = CancellationToken::new();
    let (addr, token) = serve_module_api(&pool, &cancel).await;

    let (status, document) = call(&addr, &token, "GET", "/orgs/acme/queues", None).await;
    let listed = serde_json::from_value::<cihub_api_types::QueueList>(document.clone())
        .unwrap()
        .queues;
    let paged = every_page(&addr, &token, "/orgs/acme/queues", "queues").await;

    assert_eq!(&paged, document["queues"].as_array().unwrap());

    let counters: Vec<_> = listed
        .iter()
        .map(|entry| {
            (
                entry.repo.as_str(),
                entry.target_branch.as_str(),
                [
                    entry.counts.not_ready,
                    entry.counts.pending,
                    entry.counts.testing,
                    entry.counts.pending_failure,
                    entry.counts.tests_passed,
                ],
                entry.merged_last_24h,
                entry.failed_last_24h,
                entry.failed_or_cancelled_last_24h,
                entry.last_merged_at,
            )
        })
        .collect();

    assert_eq!(
        counters,
        [
            (
                "api",
                "main",
                [0, 0, 1, 0, 0],
                0,
                0,
                0,
                Some(now - Duration::hours(26))
            ),
            (
                "app",
                "main",
                [1, 2, 2, 1, 1],
                2,
                1,
                2,
                Some(now - Duration::hours(1))
            ),
            ("app", "release", [0, 0, 0, 0, 0], 0, 0, 0, None),
        ]
    );
    assert_eq!(listed[1].id, app_main);
    assert_eq!(status, 200, "{document}");
    assert_eq!(
        document["queues"][1]["last_merged_at"],
        "2026-10-01T09:00:00Z"
    );
    assert_eq!(document["queues"][2]["last_merged_at"], Value::Null);
    assert_eq!(document["queues"][1]["counts"]["pending_failure"], 1);

    let access: Vec<_> = listed.iter().map(|entry| entry.access).collect();

    assert_eq!(
        access,
        [QueueAccess::Suspended, QueueAccess::Ok, QueueAccess::Ok]
    );
    assert_eq!(listed[1].strategy, Strategy::Pipelined, "the default");
    assert!(!listed[1].has_config_file);
    assert_eq!(listed[1].state_changed_by.as_deref(), Some("octocat"));
    assert_eq!(listed[2].state_changed_by, None);

    cancel.cancel();
}

/// An organization without a queue gets an empty list, not an error.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_api_lists_no_queue_for_an_organization_without_one(pool: PgPool) {
    seed(&pool).await;

    let mut tx = begin_system(&pool).await.unwrap();

    sqlx::query("DELETE FROM queues")
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let cancel = CancellationToken::new();
    let (addr, token) = serve_module_api(&pool, &cancel).await;

    let listed = call(&addr, &token, "GET", "/orgs/acme/queues", None).await;

    assert_eq!(
        listed,
        (
            200,
            serde_json::json!({ "queues": [], "has_more": false, "count": 0, "cursor": null })
        )
    );
    cancel.cancel();
}

/// A new PR gets the welcome comment of the queue, whose box submits it: an
/// account with `write` checks it, the comment shows the item, and the box
/// comes back once the item is cancelled. A box in a comment of someone else,
/// or checked by an account without `write`, submits nothing.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_box_of_the_welcome_comment_submits_the_pr(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let config = format!(
        "version: 1\nqueues:\n  - branch: main\n    required_statuses: [{REQUIRED}]\n    direct_merge_mode: off\n"
    );
    let main = target_repo(&sim, &[("a.txt", "a"), (".cihub.yaml", &config)]);

    q.deliver().await;
    q.settle().await;

    let shown = |number: u64| {
        sim.comments(&repo(), number)
            .into_iter()
            .filter(|body| is_status_comment(body))
            .collect::<Vec<_>>()
    };
    let item = |pool: PgPool, number: i64| async move {
        let mut tx = begin_system(&pool).await.unwrap();
        sqlx::query_scalar::<_, String>(
            "SELECT state::text FROM queue_items WHERE pr_number = $1
             ORDER BY submitted_at DESC LIMIT 1",
        )
        .bind(number)
        .fetch_optional(&mut *tx)
        .await
        .unwrap()
    };

    let (number, head) = open_pr_with(&sim, &main, 1, &[("f1.txt", Some("x"))]);

    sim.report_check(&repo(), &head, check(REQUIRED, true))
        .unwrap();

    q.deliver().await;
    q.settle().await;

    let welcome = shown(number);
    assert_eq!(welcome.len(), 1, "{welcome:?}");
    assert!(
        welcome[0].starts_with("CIHub manages merges into `main`."),
        "{welcome:?}"
    );
    assert!(
        welcome[0].contains("- [ ] <!-- cihub:submit -->"),
        "{welcome:?}"
    );

    sim.check_submit_box(&repo(), number, &alice()).unwrap();
    q.deliver().await;
    q.settle().await;
    assert_eq!(item(pool.clone(), 1).await.as_deref(), Some("testing"));

    let testing = shown(number);
    assert_eq!(
        testing.len(),
        1,
        "the same comment shows the item: {testing:?}"
    );
    assert!(
        testing[0].starts_with("🧪 **Testing** · position 1 in the queue for `main`"),
        "{testing:?}"
    );
    assert!(
        !testing[0].contains("cihub:submit"),
        "no box while in the queue"
    );

    sim.comment(&repo(), number, &alice(), "/cihub cancel")
        .unwrap();

    q.deliver().await;
    q.settle().await;
    assert_eq!(item(pool.clone(), 1).await.as_deref(), Some("cancelled"));
    assert!(
        shown(number)[0].contains("- [ ] <!-- cihub:submit -->"),
        "the box comes back"
    );

    sim.check_submit_box(&repo(), number, &alice()).unwrap();
    q.deliver().await;
    q.settle().await;

    assert_eq!(
        item(pool.clone(), 1).await.as_deref(),
        Some("testing"),
        "submitted again"
    );

    let (second, head) = open_pr_with(&sim, &main, 2, &[("f2.txt", Some("x"))]);
    sim.report_check(&repo(), &head, check(REQUIRED, true))
        .unwrap();

    q.deliver().await;
    q.settle().await;
    sim.check_submit_box(&repo(), second, &bob()).unwrap();
    q.deliver().await;
    q.settle().await;
    assert_eq!(item(pool.clone(), 2).await, None);

    let refused = sim
        .comments(&repo(), second)
        .into_iter()
        .filter(|body| body.contains("needs the write permission"))
        .count();

    assert_eq!(refused, 1);

    let (third, _) = open_pr_with(&sim, &main, 3, &[("f3.txt", Some("x"))]);
    sim.comment(&repo(), third, &alice(), "- [ ] <!-- cihub:submit --> mine")
        .unwrap();

    sim.check_submit_box(&repo(), third, &alice()).unwrap();
    q.deliver().await;
    q.settle().await;
    assert_eq!(item(pool.clone(), 3).await, None);
    q.stop().await;
}

/// Exit criterion of lot 8: a PR cannot change its own conditions. A PR
/// that removes the `queue_conditions` and changes the required statuses in
/// its own `.cihub.yaml` still waits for the conditions of the file of its
/// target, and its run requires the statuses of that file.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_pr_cannot_change_its_own_conditions(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let file = |body: &str| format!("version: 1\nqueues:\n  - branch: main\n{body}");
    let strict = file(&format!(
        "    required_statuses: [{REQUIRED}]\n    direct_merge_mode: off\n    queue_conditions:\n      - label = ready\n"
    ));
    let main = target_repo(&sim, &[("a.txt", "a"), (".cihub.yaml", &strict)]);

    q.deliver().await;
    q.settle().await;

    let loose = file("    required_statuses: [lint]\n");
    let (number, head) = open_pr_with(
        &sim,
        &main,
        1,
        &[("f1.txt", Some("x")), (".cihub.yaml", Some(&loose))],
    );

    sim.report_check(&repo(), &head, check(REQUIRED, true))
        .unwrap();

    sim.report_check(&repo(), &head, check("lint", true))
        .unwrap();

    sim.comment(&repo(), number, &alice(), "/cihub merge")
        .unwrap();

    q.deliver().await;
    q.settle().await;

    let unmet = serde_json::json!([{
        "kind": "queue_conditions_unmet", "conditions": ["label = ready"]
    }]);
    let states = admissions(&pool).await;

    assert_eq!(
        (states[0].1.as_str(), &states[0].2),
        ("not_ready", &unmet),
        "the conditions of the target apply"
    );

    sim.add_label(&repo(), number, "ready", JobClass::Interactive)
        .await
        .unwrap();

    q.deliver().await;
    q.settle().await;

    let mut tx = begin_system(&pool).await.unwrap();
    let runs = test_runs(&mut tx).await;

    drop(tx);
    assert_eq!(runs.len(), 1);

    let (_, _, _, _, _, _, required, source, ..) = &runs[0];
    assert_eq!(
        required,
        &[REQUIRED.to_owned()],
        "the statuses of the target"
    );
    assert_eq!(source.as_deref(), Some("repo_config"));
    q.stop().await;
}

/// The last valid file of the queue, the head read, and the error.
async fn config_of(pool: &PgPool) -> (Option<String>, Option<String>, Option<String>) {
    let mut tx = begin_system(pool).await.unwrap();
    sqlx::query_as("SELECT config_text, config_sha, config_error FROM queues WHERE id = $1")
        .bind(queue().to_string())
        .fetch_one(&mut *tx)
        .await
        .unwrap()
}

/// The queue reads its file on each new head of its target, keeps the last
/// valid one, and follows its mode.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_queue_follows_the_file_of_its_target(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let file = |body: &str| format!("version: 1\nqueues:\n  - branch: main\n{body}");
    let valid = file("    branch_prefix: ship\n");
    let main = target_repo(&sim, &[("a.txt", "a"), (".cihub.yaml", &valid)]);

    q.deliver().await;
    q.settle().await;

    assert_eq!(
        config_of(&pool).await,
        (Some(valid.clone()), Some(main.clone()), None)
    );

    let broken_file = file("    window: { floor: 30 }\n");
    let broken = sim
        .commit(&repo(), &main, &[(".cihub.yaml", Some(&broken_file))])
        .unwrap();

    sim.push(&repo(), "main", &broken, &alice()).unwrap();
    q.deliver().await;
    q.settle().await;

    let (text, sha, error) = config_of(&pool).await;
    assert_eq!((text, sha), (Some(valid), Some(broken.clone())));

    let error = error.unwrap();
    assert!(
        error.contains("window.floor (30) is greater than window.ceiling (20)"),
        "{error}"
    );

    let parallel_file = file("    strategy: parallel\n");
    let parallel = sim
        .commit(&repo(), &broken, &[(".cihub.yaml", Some(&parallel_file))])
        .unwrap();

    sim.push(&repo(), "main", &parallel, &alice()).unwrap();
    q.deliver().await;
    q.settle().await;

    assert_eq!(
        config_of(&pool).await,
        (Some(parallel_file), Some(parallel), None)
    );

    let mut tx = begin_system(&pool).await.unwrap();
    let events: Vec<(String, serde_json::Value)> = sqlx::query_as(
        "SELECT type::text, payload FROM events WHERE type::text LIKE 'queue.%' ORDER BY seq",
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap();
    let events: Vec<(&str, &serde_json::Value)> = events
        .iter()
        .map(|(kind, payload)| (kind.as_str(), payload))
        .collect();

    assert_eq!(
        events,
        [
            (
                "queue.settings_changed",
                &serde_json::json!({ "before": "single", "after": "parallel" })
            ),
            (
                "queue.transitioned",
                &serde_json::json!({
                    "from": "running", "to": "switching_modes", "reason": "mode_changed"
                })
            ),
            (
                "queue.transitioned",
                &serde_json::json!({
                    "from": "switching_modes", "to": "running", "reason": "mode_switch_completed"
                })
            ),
        ]
    );

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

/// The scheduler asks for a check of the configuration of a queue that has
/// none, or whose check is one hour old, while CIHub reaches its repository.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_configuration_of_a_queue_is_checked_every_hour(pool: PgPool) {
    seed(&pool).await;

    let now = clock();
    // The check that the job would write, once its job is done.
    let run_the_check = |at: OffsetDateTime| {
        let pool = pool.clone();
        async move {
            let mut tx = begin_system(&pool).await.unwrap();
            sqlx::query(
                "INSERT INTO config_checks (org_id, queue_id, findings, checked_at)
                 VALUES ($1, $2, '[]', $3)
                 ON CONFLICT (queue_id) DO UPDATE SET checked_at = $3",
            )
            .bind(org().to_string())
            .bind(queue().to_string())
            .bind(at)
            .execute(&mut *tx)
            .await
            .unwrap();
            sqlx::query(
                "UPDATE jobs SET state = 'done', finished_at = now() WHERE type = 'check_config'",
            )
            .execute(&mut *tx)
            .await
            .unwrap();
            tx.commit().await.unwrap();
        }
    };

    assert_eq!(request_config_checks(&pool, now).await.unwrap(), 1);
    assert_eq!(
        request_config_checks(&pool, now + Duration::minutes(5))
            .await
            .unwrap(),
        0,
        "a check that waits is not asked again"
    );

    let mut tx = begin_system(&pool).await.unwrap();
    let run_after: OffsetDateTime =
        sqlx::query_scalar("SELECT run_after FROM jobs WHERE type = 'check_config'")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    drop(tx);

    assert!(
        run_after >= now && run_after < now + Duration::hours(1),
        "{run_after}: within the hour"
    );

    run_the_check(now).await;

    assert_eq!(
        request_config_checks(&pool, now + Duration::minutes(30))
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        request_config_checks(&pool, now + Duration::minutes(61))
            .await
            .unwrap(),
        1
    );

    run_the_check(now + Duration::minutes(61)).await;

    let mut tx = begin_system(&pool).await.unwrap();
    sqlx::query("UPDATE scm_connections SET suspended_at = $1")
        .bind(now)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    assert_eq!(
        request_config_checks(&pool, now + Duration::days(1))
            .await
            .unwrap(),
        0,
        "no check while the access is lost"
    );
}

/// An old partition goes only once a snapshot covers its events, and the log
/// of the queue goes on from the snapshot.
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

    run_maintenance(&pool, today).await.unwrap();
    assert!(partition_exists(&pool, "events_2026_05").await);

    assert_eq!(wake_uncovered(&pool, today).await.unwrap(), 1);

    let reconciler = Reconciler {
        translate: translate(&Arc::default()),
        dashboard_url: None,
    };

    reconciler
        .reconcile(&pool, queue(), &|| today)
        .await
        .unwrap();

    assert_eq!(wake_uncovered(&pool, today).await.unwrap(), 0);
    run_maintenance(&pool, today).await.unwrap();
    assert!(!partition_exists(&pool, "events_2026_05").await);

    let mut tx = begin_system(&pool).await.unwrap();
    let snapshot: i64 = sqlx::query_scalar("SELECT snapshot_seq FROM queues WHERE id = $1")
        .bind(queue().to_string())
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
            sender_id: Some(user()),
        },
    };

    log.write(&mut tx, vec![pushed]).await.unwrap();
    assert_eq!(log.seq, snapshot + 1);

    let err = rebuild(&mut tx, queue()).await.unwrap_err();
    assert!(matches!(err, MergeQueueError::LogGap { .. }), "{err:?}");
}

/// A live queue without a wake-up is woken when a scheduler starts; a queue whose items all concluded is not.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_live_queues_are_woken_at_start(pool: PgPool) {
    async fn asleep(pool: &PgPool) {
        let mut tx = begin_system(pool).await.unwrap();
        sqlx::query("UPDATE queues SET dirty = false, next_tick_at = NULL")
            .execute(&mut *tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
    }

    let mut w = World::new(pool.clone()).await;
    asleep(&pool).await;
    assert_eq!(cihub_mergequeue::wake_live_queues(&pool).await.unwrap(), 0);
    w.submit(1).await;
    asleep(&pool).await;
    assert_eq!(cihub_mergequeue::wake_live_queues(&pool).await.unwrap(), 1);

    let mut tx = begin_system(&pool).await.unwrap();
    let dirty: bool = sqlx::query_scalar("SELECT dirty FROM queues WHERE id = $1")
        .bind(queue().to_string())
        .fetch_one(&mut *tx)
        .await
        .unwrap();

    assert!(dirty);
}

/// The current items: PR number, state and reason.
async fn current_items(pool: &PgPool) -> Vec<(i64, String, String)> {
    let mut tx = begin_system(pool).await.unwrap();
    sqlx::query_as(
        "SELECT pr_number, state::text, state_reason::text FROM queue_items WHERE is_current
         ORDER BY pr_number",
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap()
}

/// Opens the PR of the branch `pr-<n>` with a file of its own, reports its
/// CI, and returns its number: the draft PRs of the queue take numbers too.
fn open_green_pr(sim: &Sim, main: &str, n: u64) -> u64 {
    let path = format!("f{n}.txt");
    let (number, head) = open_pr_with(sim, main, n, &[(&path, Some("x"))]);

    sim.report_check(&repo(), &head, check(REQUIRED, true))
        .unwrap();

    number
}

fn submit_new_pr(sim: &Sim, main: &str, n: u64) -> u64 {
    let number = open_green_pr(sim, main, n);
    sim.comment(&repo(), number, &alice(), "/cihub merge")
        .unwrap();
    number
}

/// The merge job: a server error is retried, a head that changed fails the
/// item with `pr_updated_at_merge_time`.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_merge_job_retries_and_reports_its_errors(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let main = target_repo(&sim, &[("a.txt", "a"), (".cihub.yaml", TEST_PATH)]);

    q.deliver().await;
    q.settle().await;

    for n in 1..=2 {
        assert_eq!(open_green_pr(&sim, &main, n), n);
    }

    for n in 1..=2 {
        sim.comment(&repo(), n, &alice(), "/cihub merge").unwrap();
        q.deliver().await;
        q.settle().await;
    }

    let runs = q.runs_in_progress().await;
    assert_eq!(runs.len(), 2, "{runs:?}");

    let test_sha = |pr: u64| {
        runs.iter()
            .find(|(_, _, prs)| prs.last() == Some(&pr))
            .map(|(_, sha, _)| sha.clone())
            .unwrap()
    };

    sim.fail(Fault {
        call: Some("merge_pr"),
        error: ScmError::Unavailable("502".into()),
        times: 2,
    });

    sim.report_check(&repo(), &test_sha(1), check(REQUIRED, true))
        .unwrap();

    q.deliver().await;
    q.settle().await;
    q.deliver().await;
    q.settle().await;
    assert_eq!(current_items(&pool).await[0].1, "merged");

    sim.fail(Fault {
        call: Some("merge_pr"),
        error: ScmError::HeadChanged,
        times: 1,
    });

    sim.report_check(&repo(), &test_sha(2), check(REQUIRED, true))
        .unwrap();

    q.deliver().await;
    q.settle().await;

    assert_eq!(
        current_items(&pool).await,
        [
            (1, "merged".into(), "merge_graph_merge".into()),
            (2, "failed".into(), "pr_updated_at_merge_time".into()),
        ]
    );

    let merges = sim.calls().iter().filter(|c| c.name == "merge_pr").count();
    assert_eq!(merges, 4, "two errors and a merge, then a changed head");
    assert!(
        sim.calls()
            .iter()
            .filter(|c| c.name == "merge_pr")
            .all(|c| c.class == JobClass::Merge)
    );

    q.stop().await;
}

/// A PR merged outside the queue while in test: the merge job does not claim
/// it, the item is `manual_merge`, and the PR behind it is tested again on what
/// landed, then merged.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_pr_merged_outside_the_queue_is_not_claimed(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let main = target_repo(&sim, &[("a.txt", "a"), (".cihub.yaml", TEST_PATH)]);

    q.deliver().await;
    q.settle().await;

    for n in 1..=2 {
        assert_eq!(open_green_pr(&sim, &main, n), n);
    }

    for n in 1..=2 {
        sim.comment(&repo(), n, &alice(), "/cihub merge").unwrap();
        q.deliver().await;
        q.settle().await;
    }

    let runs = q.runs_in_progress().await;
    let first = runs.iter().find(|r| r.2 == [1]).unwrap().1.clone();
    let by_hand = cihub_scm::PrMerge {
        number: 1,
        head_sha: sim
            .pull_request(&repo(), 1, JobClass::Repair)
            .await
            .unwrap()
            .head_sha,
        tested_sha: String::new(),
        method: cihub_domain::MergeMethod::Squash,
        title: "merged by hand".into(),
        message: String::new(),
    };

    sim.merge_pr(&repo(), &by_hand, JobClass::Interactive)
        .await
        .unwrap();

    sim.take_deliveries();

    sim.report_check(&repo(), &first, check(REQUIRED, true))
        .unwrap();

    q.deliver().await;
    q.settle().await;

    assert_eq!(
        current_items(&pool).await[0],
        (1, "merged".into(), "manual_merge".into())
    );

    for _ in 0..3 {
        for (_, test_sha, prs) in q.runs_in_progress().await {
            if prs == [2] {
                sim.report_check(&repo(), &test_sha, check(REQUIRED, true))
                    .unwrap();
            }
        }
        q.deliver().await;
        q.settle().await;
    }

    assert_eq!(
        current_items(&pool).await[1],
        (2, "merged".into(), "merge_graph_merge".into())
    );

    let mut tx = begin_system(&pool).await.unwrap();
    let restarted: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM events WHERE type = 'item.transitioned'
         AND payload->>'reason' = 'prerequisite_merge_item_manually_merged'",
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();

    assert_eq!(restarted, 1);
    drop(tx);

    let merges = sim.calls().iter().filter(|c| c.name == "merge_pr").count();
    assert_eq!(merges, 2, "the merge by hand and the merge of PR 2");
    q.stop().await;
}

/// The repair reads: a lost report, a deleted test branch and a lost push on
/// the target are found again; `resync` asks for every read at once.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_reads_repair_what_lost_webhooks_leave(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let main = target_repo(&sim, &[("a.txt", "a"), (".cihub.yaml", TEST_PATH)]);

    q.deliver().await;
    q.settle().await;

    submit_new_pr(&sim, &main, 1);
    q.deliver().await;
    q.settle().await;

    let (run_id, test_sha, _) = q.runs_in_progress().await.remove(0);
    let mut tx = begin_system(&pool).await.unwrap();
    let test_branch: String = sqlx::query_scalar("SELECT test_branch FROM test_runs WHERE id = $1")
        .bind(run_id.to_string())
        .fetch_one(&mut *tx)
        .await
        .unwrap();

    drop(tx);

    sim.delete_branch(&repo(), &test_branch, JobClass::Repair)
        .await
        .unwrap();

    sim.take_deliveries();
    q.clock.advance(10);
    q.wake().await;
    q.settle().await;
    assert_eq!(sim.branch(&repo(), &test_branch), Some(test_sha.clone()));

    // The report of the CI is lost: the next read of the checks finds it.
    sim.report_check(&repo(), &test_sha, check(REQUIRED, true))
        .unwrap();

    sim.take_deliveries();
    q.settle().await;
    assert_eq!(current_items(&pool).await[0].1, "testing");
    q.clock.advance(10);
    q.wake().await;
    q.settle().await;
    q.deliver().await;
    q.settle().await;
    assert_eq!(current_items(&pool).await[0].1, "merged");

    // A push from outside is lost: the read of the target head restarts the
    // group in test.
    let head = sim.branch(&repo(), "main").unwrap();
    submit_new_pr(&sim, &head, 2);
    q.deliver().await;
    q.settle().await;
    assert_eq!(current_items(&pool).await[1].1, "testing");

    let pushed = sim
        .commit(&repo(), &head, &[("other.txt", Some("o"))])
        .unwrap();

    sim.push(&repo(), "main", &pushed, &bob()).unwrap();
    sim.take_deliveries();
    q.clock.advance(5);
    q.wake().await;
    q.settle().await;

    let mut tx = begin_system(&pool).await.unwrap();
    let read: (String, Option<String>) = sqlx::query_as(
        "SELECT payload->>'after', payload->>'sender_id' FROM events
         WHERE type = 'target.pushed' ORDER BY seq DESC LIMIT 1",
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();

    assert_eq!(read, (pushed, None));

    let restarted: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM events
         WHERE type = 'item.transitioned' AND payload->>'reason' = 'manual_push_to_target_branch'",
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();

    assert_eq!(restarted, 1);
    drop(tx);

    // A resync asks for every read once per minute.
    let now = q.clock.now();
    let requested = cihub_mergequeue::resync(&pool, queue(), now).await.unwrap();

    assert!(requested >= 2, "{requested} jobs");
    assert_eq!(
        cihub_mergequeue::resync(&pool, queue(), now).await.unwrap(),
        0
    );

    q.settle().await;
    q.stop().await;
}

/// A generator of the fault injection test: splitmix64, so that a seed gives
/// the same faults.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);

        z ^ (z >> 31)
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }
}

/// The PRs of one seed.
const CHAOS_PRS: u64 = 5;

/// One seed of the fault injection test: its own organization, connection,
/// repository and queue in the shared database, the scheduler and the worker
/// of the server against a simulator that fails calls at random, and
/// webhooks lost, sent twice and reordered.
struct Chaos {
    seed: u64,
    /// The `.cihub.yaml` of the repository.
    config: &'static str,
    pool: PgPool,
    sim: Arc<Sim>,
    clock: TestClock,
    rng: Rng,
    deliveries: Arc<Mutex<HashMap<String, ScmDelivery>>>,
    sent: usize,
    generation: u32,
    /// The pool of the running process: a kill closes it, as the end of a
    /// process closes its connections and rolls back its transactions.
    process: Option<PgPool>,
    loops: Vec<tokio::task::JoinHandle<()>>,
}

impl Chaos {
    fn id(&self, kind: u128) -> Ulid {
        Ulid((u128::from(self.seed) + 1) << 16 | kind)
    }

    fn org(&self) -> OrgId {
        OrgId::from_ulid(self.id(1))
    }

    fn queue(&self) -> QueueId {
        QueueId::from_ulid(self.id(2))
    }

    fn repo(&self) -> RepoRef {
        RepoRef {
            connection: ConnectionRef {
                id: ConnectionId::from_ulid(self.id(3)),
                external_id: format!("conn-{}", self.seed),
            },
            external_id: format!("repo-{}", self.seed),
            namespace: "acme".into(),
            name: format!("app-{}", self.seed),
        }
    }

    async fn new(pool: PgPool, seed: u64, clock: TestClock, config: &'static str) -> Self {
        let mut chaos = Self {
            seed,
            config,
            pool,
            sim: Arc::new(Sim::new()),
            clock,
            rng: Rng(seed),
            deliveries: Arc::new(Mutex::new(HashMap::new())),
            sent: 0,
            generation: 0,
            process: None,
            loops: Vec::new(),
        };
        let repo = chaos.repo();
        let mut tx = begin_system(&chaos.pool).await.unwrap();

        sqlx::raw_sql(AssertSqlSafe(format!(
            "INSERT INTO orgs (id, slug) VALUES ('{org}', 'acme-{seed}');
             INSERT INTO scm_connections (id, org_id, account_external_id, kind, external_id,
                                          synced_at, permissions)
                 VALUES ('{connection}', '{org}', 'account-{seed}', 'github_app',
                         '{connection_external_id}', '{synced_at}', '{{}}');
             INSERT INTO repos (id, org_id, connection_id, external_id, namespace, name,
                                default_branch)
                 VALUES ('{repo_id}', '{org}', '{connection}', '{repo_external_id}', 'acme',
                         '{name}', 'main');
             INSERT INTO queues (id, org_id, repo_id, target_branch, config_text)
                 VALUES ('{queue}', '{org}', '{repo_id}', 'main', '{config_text}');",
            org = chaos.org(),
            connection = repo.connection.id,
            connection_external_id = repo.connection.external_id,
            repo_id = RepoId::from_ulid(chaos.id(5)),
            // The queue has read its file, as after its first push.
            config_text = config,
            repo_external_id = repo.external_id,
            name = repo.name,
            queue = chaos.queue(),
            synced_at = chaos.clock.now(),
        )))
        .execute(&mut *tx)
        .await
        .unwrap();

        tx.commit().await.unwrap();
        chaos.start().await;

        chaos
    }

    /// Starts a worker and a scheduler with new names and their own pool, as
    /// a new process.
    async fn start(&mut self) {
        self.generation += 1;

        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(8)
            .connect_with(self.pool.connect_options().as_ref().clone())
            .await
            .unwrap();

        self.process = Some(pool.clone());

        let scm: Arc<dyn Scm> = self.sim.clone();
        let module = cihub_mergequeue::module(
            pool.clone(),
            scm,
            translate(&self.deliveries),
            format!("scheduler-{}-{}", self.seed, self.generation),
            self.clock.shared(),
            None,
        );
        let mut executors = Executors::default();
        (module.executors)(&mut executors);

        let cancel = CancellationToken::new();
        let clock = self.clock.clone();
        let worker = run_worker(
            pool,
            format!("worker-{}-{}", self.seed, self.generation),
            Arc::new(executors),
            move || clock.now(),
            std::time::Duration::from_millis(50),
            cancel.clone(),
        );
        let scheduler = (module.scheduler)(cancel);

        self.loops = vec![
            tokio::spawn(async move { worker.await.unwrap() }),
            tokio::spawn(async move { scheduler.await.unwrap() }),
        ];
    }

    /// Kills the worker and the scheduler wherever they are, then starts new
    /// ones once the locks of the jobs and the leases of the queues expire.
    async fn kill(&mut self) {
        self.stop().await;
        self.clock.advance(2);
        self.start().await;
        self.wake().await;
    }

    async fn stop(&mut self) {
        for running in self.loops.drain(..) {
            running.abort();
            let _ = running.await;
        }
        if let Some(pool) = self.process.take() {
            pool.close().await;
        }
    }

    async fn wake(&self) {
        let mut tx = begin_system(&self.pool).await.unwrap();
        mark_dirty(&mut tx, self.queue()).await.unwrap();
        tx.commit().await.unwrap();
    }

    /// Stores the deliveries of the simulator; with `faults`, some are lost,
    /// some sent twice with the same identifier, and their order changes.
    async fn deliver(&mut self, faults: bool) {
        let mut taken: Vec<(String, ScmDelivery)> = Vec::new();
        for delivery in self.sim.take_deliveries() {
            self.sent += 1;
            let id = format!("delivery-{}-{}", self.seed, self.sent);
            if faults && self.rng.chance(10) {
                continue;
            }
            if faults && self.rng.chance(10) {
                taken.push((id.clone(), delivery.clone()));
            }
            taken.push((id, delivery));
        }

        if faults {
            for i in (1..taken.len()).rev() {
                let j = usize::try_from(self.rng.next() % (i as u64 + 1)).unwrap();
                taken.swap(i, j);
            }
        }

        let mut tx = begin_system(&self.pool).await.unwrap();
        for (id, delivery) in taken {
            let route = find_route(&mut tx, &delivery).await.unwrap();
            self.deliveries.lock().unwrap().insert(id.clone(), delivery);
            let stored = Delivery {
                delivery_id: id.clone(),
                org_id: route.org_id,
                connection_id: route.connection_id,
                queue_id: route.queue_id,
                event: "sim".into(),
                payload: serde_json::json!({ "id": id }),
                received_at: self.clock.now(),
            };
            store_delivery(&mut tx, &stored).await.unwrap();
        }

        tx.commit().await.unwrap();
    }

    /// Waits until no job of the seed runs or is due, no fact waits and the
    /// queue is clean; moves the clock to the next job that waits for its
    /// backoff.
    async fn settle(&self) {
        let mut idle = 0;
        for _ in 0..4000 {
            let now = self.clock.now();
            let mut tx = begin_system(&self.pool).await.unwrap();
            let (busy, next_job, deliveries): (bool, Option<OffsetDateTime>, bool) =
                sqlx::query_as(
                    "SELECT EXISTS (SELECT 1 FROM queue_inbox WHERE queue_id = $3)
                         OR EXISTS (SELECT 1 FROM jobs WHERE org_id = $2 AND state = 'running')
                         OR EXISTS (SELECT 1 FROM jobs
                                    WHERE org_id = $2 AND state = 'queued' AND run_after <= $1)
                         OR EXISTS (SELECT 1 FROM queues WHERE id = $3 AND dirty),
                     (SELECT min(run_after) FROM jobs WHERE org_id = $2 AND state = 'queued'),
                     EXISTS (SELECT 1 FROM webhook_inbox
                             WHERE org_id = $2)",
                )
                .bind(now)
                .bind(self.org().to_string())
                .bind(self.queue().to_string())
                .fetch_one(&mut *tx)
                .await
                .unwrap();
            drop(tx);
            if busy {
                idle = 0;
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                continue;
            }
            if let Some(at) = next_job {
                let wait = (at - now).whole_seconds() + 1;
                self.clock.advance(((wait + 59) / 60).max(1));
                self.wake().await;
                continue;
            }
            if deliveries && idle < 20 {
                idle += 1;
                self.wake().await;
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                continue;
            }
            return;
        }
        self.not_settled().await;
    }

    /// Fails with what the seed waits for, and what a reconciliation of its
    /// queue gives.
    async fn not_settled(&self) -> ! {
        let mut tx = begin_system(&self.pool).await.unwrap();
        let left: Vec<(String, String)> = sqlx::query_as(
            "SELECT 'fact', type::text FROM queue_inbox WHERE queue_id = $2
             UNION ALL SELECT 'job', idempotency_key || ' ' || state::text || ' ' || run_after::text
                          || ' ' || coalesce(last_error, '')
                 FROM jobs WHERE org_id = $1 AND state IN ('queued', 'running')
             UNION ALL SELECT 'dirty', id FROM queues WHERE id = $2 AND dirty
             UNION ALL SELECT 'delivery', delivery_id FROM webhook_inbox WHERE org_id = $1",
        )
        .bind(self.org().to_string())
        .bind(self.queue().to_string())
        .fetch_all(&mut *tx)
        .await
        .unwrap();

        drop(tx);

        let reconciler = Reconciler {
            translate: translate(&self.deliveries),
            dashboard_url: None,
        };
        let direct = reconciler
            .reconcile(&self.pool, self.queue(), &|| self.clock.now())
            .await
            .map_err(|err| cihub_platform::error_chain(&err));

        panic!(
            "seed {}: the queue did not settle at {}: {left:?}; a reconciliation gives {direct:?}",
            self.seed,
            self.clock.now()
        );
    }

    /// PR number, state, reason, position and the rank of its merge in the
    /// log, for the current items.
    async fn items(&self) -> Vec<(i64, String, String, Option<i64>, Option<i64>)> {
        let mut tx = begin_system(&self.pool).await.unwrap();
        sqlx::query_as(
            "SELECT i.pr_number, i.state::text, i.state_reason::text, i.position,
                    (SELECT min(e.seq) FROM events e
                     WHERE e.queue_id = i.queue_id AND e.type = 'item.transitioned'
                       AND e.item_id = i.id AND e.payload->>'to' = 'merged')
             FROM queue_items i WHERE i.queue_id = $1 AND i.is_current ORDER BY i.pr_number",
        )
        .bind(self.queue().to_string())
        .fetch_all(&mut *tx)
        .await
        .unwrap()
    }

    /// The test SHA of each run in progress, and the heads that it tests: its
    /// predecessors, then its members.
    async fn runs_in_progress(&self) -> Vec<(String, Vec<String>)> {
        let mut tx = begin_system(&self.pool).await.unwrap();
        sqlx::query_as(
            "SELECT test_sha, head_shas FROM test_runs
             WHERE queue_id = $1 AND status = 'in_progress'",
        )
        .bind(self.queue().to_string())
        .fetch_all(&mut *tx)
        .await
        .unwrap()
    }

    /// Plays the seed: `CHAOS_PRS` PRs, some with a red CI; the CI reports
    /// on each run once; faults and kills all along. Then checks the oracle.
    async fn play(&mut self) {
        let repo = self.repo();
        let sim = self.sim.clone();
        let root = sim.add_repo(
            &repo,
            "main",
            &[("a.txt", "a"), (".cihub.yaml", self.config)],
        );
        let rules = RequiredChecks {
            names: BTreeSet::from([REQUIRED.to_owned()]),
            up_to_date: false,
        };

        sim.set_rules(&repo, "main", rules).unwrap();

        sim.set_permission(&repo, &alice(), Permission::Write)
            .unwrap();

        let mut red = BTreeSet::new();
        let mut red_heads = BTreeSet::new();

        for n in 1..=CHAOS_PRS {
            let branch = format!("pr-{n}");
            let path = format!("f{n}.txt");
            let head = sim.commit(&repo, &root, &[(&path, Some("x"))]).unwrap();
            sim.push(&repo, &branch, &head, &bob()).unwrap();
            let number = sim.open_pr(&repo, &branch, "main", &bob()).unwrap();
            sim.report_check(&repo, &head, check(REQUIRED, true))
                .unwrap();
            if self.rng.chance(25) {
                red.insert(i64::try_from(number).unwrap());
                red_heads.insert(head.clone());
            }
            sim.comment(&repo, number, &alice(), "/cihub merge")
                .unwrap();
        }

        // The submissions arrive whole: a lost comment is the sweep of lot 4.
        self.deliver(false).await;
        self.settle().await;

        sim.fail_at_random(Some(cihub_scm::sim::RandomFaults {
            seed: self.seed,
            unavailable_percent: 1,
            rate_limited_percent: 1,
            rate_limited_until: self.clock.now() + Duration::minutes(1),
        }));

        let mut reported: BTreeSet<String> = BTreeSet::new();
        for round in 0.. {
            assert!(
                round < 200,
                "seed {}: the queue does not conclude",
                self.seed
            );
            self.deliver(true).await;
            self.settle().await;
            if self.rng.chance(15) {
                self.kill().await;
                self.settle().await;
            }
            let items = self.items().await;
            let done = items.len() == usize::try_from(CHAOS_PRS).unwrap()
                && items
                    .iter()
                    .all(|i| matches!(i.1.as_str(), "merged" | "failed" | "cancelled"));
            if done {
                break;
            }
            let mut new = 0;
            for (test_sha, heads) in self.runs_in_progress().await {
                if !reported.insert(test_sha.clone()) {
                    continue;
                }
                // A red PR fails every run that tests its code, as a member
                // or as a predecessor.
                let passed = !heads.iter().any(|head| red_heads.contains(head));
                sim.report_check(&repo, &test_sha, check(REQUIRED, passed))
                    .unwrap();
                new += 1;
            }
            if new == 0 {
                // Lost reports and pushes come back with the repair reads.
                self.clock.advance(11);
                self.wake().await;
            }
        }

        sim.fail_at_random(None);
        self.deliver(false).await;
        self.settle().await;
        self.check_oracle(&red).await;
        self.stop().await;
    }

    /// Each green PR merged exactly once, in the order of the queue; each red
    /// PR failed with a reason; the replay of the log gives the same
    /// projections.
    async fn check_oracle(&self, red: &BTreeSet<i64>) {
        let seed = self.seed;
        let items = self.items().await;
        let mut merged: Vec<(i64, i64, i64)> = Vec::new();

        for (pr, state, reason, position, merged_at) in &items {
            if red.contains(pr) {
                assert_eq!(
                    state, "failed",
                    "seed {seed}: red PR {pr} is {state} ({reason})"
                );
                assert!(!reason.is_empty(), "seed {seed}: red PR {pr} has no reason");
            } else {
                assert_eq!(
                    (state.as_str(), reason.as_str()),
                    ("merged", "merge_graph_merge"),
                    "seed {seed}: green PR {pr}"
                );
                merged.push((merged_at.unwrap(), position.unwrap(), *pr));
            }
        }

        let mut tx = begin_system(&self.pool).await.unwrap();
        let decisions: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT i.pr_number, count(*) FROM events e JOIN queue_items i ON i.id = e.item_id
             WHERE e.queue_id = $1 AND e.type = 'item.transitioned' AND e.payload->>'to' = 'merged'
             GROUP BY i.pr_number ORDER BY 1",
        )
        .bind(self.queue().to_string())
        .fetch_all(&mut *tx)
        .await
        .unwrap();

        assert!(
            decisions.iter().all(|(_, count)| *count == 1),
            "seed {seed}: {decisions:?}"
        );

        for (_, _, pr) in &merged {
            let pr = self
                .sim
                .pull_request(&self.repo(), u64::try_from(*pr).unwrap(), JobClass::Repair)
                .await
                .unwrap();
            assert_eq!(pr.state, PrState::Merged, "seed {seed}: PR {}", pr.number);
        }

        merged.sort_unstable();

        let positions: Vec<i64> = merged.iter().map(|m| m.1).collect();
        assert!(
            positions.is_sorted(),
            "seed {seed}: merged out of order {merged:?}"
        );

        let before = projections(&mut tx).await;
        rebuild(&mut tx, self.queue()).await.unwrap();
        assert_eq!(projections(&mut tx).await, before, "seed {seed}");
    }
}

/// The exit criterion of lot 7 (`specs/RELIABILITY.md`): on each seed, server
/// errors and rate limits at random, webhooks lost, sent twice and reordered,
/// and processes killed at random, each green PR is merged exactly once, in the
/// order of the queue, and each red PR fails with a reason. `CIHUB_SIM_SEEDS`
/// gives the number of seeds (10 by default; `mise run sim` plays 1 000, for
/// this test and the `batched` one). Direct merge is `off`: a red PR is green
/// on its own head and red only on its test commits, which a direct merge
/// does not run.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn fault_injection(pool: PgPool) {
    play_chaos(
        pool,
        "version: 1\nqueues:\n  - branch: main\n    direct_merge_mode: off\n",
    )
    .await;
}

/// The same test with the `batched` strategy (lot 12): batches, bisections,
/// failure depth and optimistic merge under the same faults.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn fault_injection_batched(pool: PgPool) {
    play_chaos(
        pool.clone(),
        "version: 1\nqueues:\n  - branch: main\n    strategy: batched\n    direct_merge_mode: off\n",
    )
    .await;

    // The file applied: the seeds formed batches and bisected some.
    let mut tx = begin_system(&pool).await.unwrap();
    let kinds: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT payload->>'kind' FROM events WHERE type = 'group.formed' ORDER BY 1",
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap();

    assert!(
        kinds.contains(&"batch".to_owned()) && kinds.contains(&"bisection".to_owned()),
        "{kinds:?}"
    );
}

async fn play_chaos(pool: PgPool, config: &'static str) {
    let seeds: u64 = std::env::var("CIHUB_SIM_SEEDS")
        .ok()
        .and_then(|seeds| seeds.parse().ok())
        .unwrap_or(10);

    create_event_partitions(
        &pool,
        Date::from_calendar_date(2026, Month::October, 1).unwrap(),
    )
    .await
    .unwrap();

    let clock = TestClock::new();
    for seed in 0..seeds {
        let mut chaos = Chaos::new(pool.clone(), seed, clock.clone(), config).await;
        chaos.play().await;
    }
}

/// A session of the dashboard of `login`, with `role` in the organization,
/// and its secret.
async fn session_of(pool: &PgPool, n: u128, login: &str, role: MembershipRole) -> SecretString {
    let user_id = UserId::from_ulid(Ulid(n));
    let secret = cihub_store::new_session_secret().unwrap();
    let session = cihub_store::NewSession {
        id: SessionId::from_ulid(Ulid(n)),
        user_id,
        created_at: OffsetDateTime::from_unix_timestamp(1_790_000_000).unwrap(),
        expires_at: OffsetDateTime::from_unix_timestamp(1_790_100_000).unwrap(),
    };
    let mut tx = begin_system(pool).await.unwrap();

    cihub_store::ensure_user(&mut tx, user_id, &n.to_string(), login)
        .await
        .unwrap();
    cihub_store::create_session(&mut tx, &session, &secret)
        .await
        .unwrap();
    cihub_store::record_membership(&mut tx, org(), user_id, role, session.created_at)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    secret
}

/// Sends `method path` with the session of `secret`, with its CSRF token or
/// without it, and returns the status of the answer.
async fn call_as(
    addr: &str,
    secret: &SecretString,
    with_csrf: bool,
    method: &str,
    path: &str,
    body: &str,
) -> u16 {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let csrf = if with_csrf {
        format!("X-CSRF-Token: {}\r\n", cihub_store::csrf_token(secret))
    } else {
        String::new()
    };
    let request = format!(
        "{method} /api/v1{path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\
         Cookie: __Host-cihub_session={}\r\n{csrf}Content-Type: application/json\r\n\
         Content-Length: {}\r\n\r\n{body}",
        secret.expose_secret(),
        body.len()
    );
    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();

    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    response.split(' ').nth(1).unwrap().parse().unwrap()
}

/// A session acts on a queue with the role of its user: a member cannot
/// pause it, an admin needs the CSRF token, and the log and the audit log
/// name the user. A submission from a session carries the account of its
/// user, so that the job checks their permission on the repository, as for
/// a command in a comment.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_session_acts_on_a_queue_in_the_name_of_its_user(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let config = "version: 1\nqueues:\n  - branch: main\n";
    let queue = "/orgs/acme/repos/app/queues/main";
    let pause = r#"{"state":"paused"}"#;
    let direct = r#"{"direct":true}"#;

    target_repo(&q.sim, &[("a.txt", "a"), (".cihub.yaml", config)]);
    q.deliver().await;
    q.settle().await;
    let (addr, _) = q.serve_api().await;
    let admin = session_of(&pool, 70, "ada", MembershipRole::Admin).await;
    let member = session_of(&pool, 71, "grace", MembershipRole::Member).await;

    let by_member = call_as(&addr, &member, true, "PATCH", queue, pause).await;
    let without_csrf = call_as(&addr, &admin, false, "PATCH", queue, pause).await;
    let by_admin = call_as(&addr, &admin, true, "PATCH", queue, pause).await;
    let submitted = call_as(
        &addr,
        &member,
        true,
        "POST",
        "/orgs/acme/repos/app/prs/1/submit",
        direct,
    )
    .await;

    assert_eq!(
        (by_member, without_csrf, by_admin, submitted),
        (403, 403, 202, 202)
    );

    let mut tx = begin_system(&pool).await.unwrap();
    let actors: Vec<(Option<String>, Option<String>)> =
        sqlx::query_as("SELECT actor_user_id, actor_token_id FROM audit_log")
            .fetch_all(&mut *tx)
            .await
            .unwrap();
    let controls: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT actor_kind::text, actor_user_id FROM queue_inbox
         WHERE type = 'queue.control_requested'
         UNION ALL
         SELECT actor_kind::text, actor_user_id FROM events WHERE type = 'queue.control_requested'",
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap();
    let requested_by: Vec<serde_json::Value> =
        sqlx::query_scalar("SELECT payload -> 'requested_by' FROM jobs WHERE type = 'submit_pr'")
            .fetch_all(&mut *tx)
            .await
            .unwrap();

    let ada = UserId::from_ulid(Ulid(70)).to_string();
    assert_eq!(actors, [(Some(ada.clone()), None)]);
    assert_eq!(controls, [("user".to_owned(), Some(ada))]);
    assert_eq!(
        requested_by,
        [serde_json::json!({ "external_id": "71", "login": "grace" })]
    );
    q.stop().await;
}

const FACT_TABLES: [&str; 7] = [
    "an_item_conclusions",
    "an_state_durations",
    "an_test_run_results",
    "an_check_results",
    "an_hourly_conclusions",
    "an_hourly_durations",
    "an_hourly_checks",
];

async fn facts(pool: &PgPool) -> Vec<Value> {
    let mut tx = begin_system(pool).await.unwrap();
    let mut rows = Vec::new();
    for table in FACT_TABLES {
        let found: Vec<Value> = sqlx::query_scalar(AssertSqlSafe(format!(
            "SELECT to_jsonb(t) FROM {table} t ORDER BY to_jsonb(t)::text"
        )))
        .fetch_all(&mut *tx)
        .await
        .unwrap();
        rows.extend(found);
    }
    rows
}

/// The Health of a queue reads the facts that its log concludes: a PR that
/// merged after its check passed on a retry, and a PR that failed. A log
/// read again counts once, the facts and the hourly aggregate rebuilt from the
/// log are the same, and another organization reads none.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_health_of_a_queue_reads_the_facts_of_its_log(pool: PgPool) {
    let mut q = SimQueue::new(pool.clone()).await;
    let sim = q.sim.clone();
    let main = target_repo(&sim, &[("a.txt", "a"), (".cihub.yaml", TEST_PATH)]);

    q.deliver().await;
    q.settle().await;

    let (addr, token) = q.serve_api().await;

    // The second PR fails twice: past the failure budget of one retry.
    for (n, results) in [(1, [false, true]), (2, [false, false])] {
        submit_new_pr(&sim, &main, n);
        q.deliver().await;
        q.settle().await;
        for passed in results {
            let (_, test_sha, _) = q.runs_in_progress().await.remove(0);
            sim.report_check(&repo(), &test_sha, check(REQUIRED, passed))
                .unwrap();
            q.deliver().await;
            q.settle().await;
            q.deliver().await;
            q.settle().await;
        }
    }
    q.notify().await;

    let path = "/orgs/acme/repos/app/queues/main/health?range=7d";
    let (status, health) = call(&addr, &token, "GET", path, None).await;

    assert_eq!(status, 200, "{health}");
    assert_eq!(health["current"]["merged"], 1);
    assert_eq!(health["current"]["failed"], 1);
    assert_eq!(
        health["verdict"],
        serde_json::json!({ "kind": "many_failures" })
    );
    let buckets = health["buckets"].as_array().unwrap();
    assert_eq!(buckets.len(), 7);
    let in_buckets = |field: &str| {
        buckets
            .iter()
            .map(|bucket| bucket[field].as_u64().unwrap())
            .sum::<u64>()
    };
    assert_eq!((in_buckets("merged"), in_buckets("failed")), (1, 1));
    assert_eq!(health["time_in_states"].as_array().unwrap().len(), 5);
    assert_eq!(
        health["left_without_merge"],
        serde_json::json!([{
            "conclusion": "failed",
            "reason": "test_run_required_status_failed",
            "description": "a required check failed",
            "count": 1
        }])
    );
    assert_eq!(
        health["checks"],
        serde_json::json!([{
            "name": REQUIRED,
            "kind": "status",
            "required": true,
            "runs": 4,
            "failed": 3,
            "passed_on_retry": 1
        }])
    );

    // The same 7 days between two times, then the periods that the API refuses.
    let to = OffsetDateTime::parse(health["to"].as_str().unwrap(), &Rfc3339).unwrap();
    let from = OffsetDateTime::parse(health["from"].as_str().unwrap(), &Rfc3339).unwrap();
    let at = |time: OffsetDateTime| time.format(&Rfc3339).unwrap();
    let between = format!(
        "/orgs/acme/repos/app/queues/main/health?from={}&to={}",
        at(from),
        at(to)
    );
    let (status, same) = call(&addr, &token, "GET", &between, None).await;

    assert_eq!(status, 200, "{same}");
    assert_eq!(
        (&same["current"], &same["buckets"], &same["checks"]),
        (&health["current"], &health["buckets"], &health["checks"])
    );
    for query in [
        format!("range=7d&from={}&to={}", at(from), at(to)),
        format!("from={}", at(from)),
        format!("from={}&to={}", at(to), at(from)),
        format!("from={}&to={}", at(to - Duration::days(400)), at(to)),
    ] {
        let path = format!("/orgs/acme/repos/app/queues/main/health?{query}");
        let (status, error) = call(&addr, &token, "GET", &path, None).await;

        assert_eq!(
            (status, &error["code"]),
            (400, &serde_json::json!("invalid_request")),
            "{query}"
        );
    }

    let before = facts(&pool).await;
    let mut tx = begin_system(&pool).await.unwrap();
    sqlx::query("DELETE FROM log_cursors")
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    q.notify().await;

    assert_eq!(facts(&pool).await, before, "the log read again counts once");

    let mut tx = begin_system(&pool).await.unwrap();
    for table in FACT_TABLES.iter().rev().chain(&["log_cursors"]) {
        sqlx::query(AssertSqlSafe(format!("DELETE FROM {table}")))
            .execute(&mut *tx)
            .await
            .unwrap();
    }
    tx.commit().await.unwrap();
    q.notify().await;

    assert_eq!(facts(&pool).await, before, "the facts rebuilt from the log");
    assert_eq!(call(&addr, &token, "GET", path, None).await.1, health);

    let mut tx = begin_org(&pool, OrgId::from_ulid(Ulid(99))).await.unwrap();
    for table in FACT_TABLES {
        let seen: i64 = sqlx::query_scalar(AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
            .fetch_one(&mut *tx)
            .await
            .unwrap();

        assert_eq!(seen, 0, "another organization reads {table}");
    }
    q.stop().await;
}

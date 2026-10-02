//! Tests against Postgres: each `#[sqlx::test]` gets a new database.

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

use std::sync::Arc;

use cihub_domain::{
    ApiScope, ConnectionId, ConnectionKind, ConnectionMode, JobClass, OrgId, QueueId, ScmInstanceId,
};
use cihub_platform::{
    ConnectionBudget, Delivery, Executors, JOB_LOCK, JOBS_PER_CONNECTION, Job, LEASE_TTL,
    MAX_ATTEMPTS, Outcome, PlatformError, QueueWakeup, Retry, Wake, acquire_leases, backoff,
    begin_reconciliation, bootstrap, claim_jobs, complete_job, enqueue_job, fail_job, lock_queue,
    maintenance_job, mark_processed, pending_deliveries, record_budgets, release_leases,
    request_maintenance, request_repo_syncs, run_budget_writer, run_maintenance, run_router,
    run_sweep, run_sync_repos, run_worker, schedule_tick, store_delivery, sync_repos_job,
};
use cihub_scm::sim::Sim;
use cihub_scm::{ConnectionRef, RepoRef, ScmAccount, ScmDelivery, ScmError, ScmEvent, ScmUser};
use cihub_store::{authenticate, begin_org, begin_system};
use serde_json::json;
use sqlx::{AssertSqlSafe, PgPool};
use time::{Duration, OffsetDateTime};
use tokio_util::sync::CancellationToken;
use ulid::Ulid;

fn queue(n: u128) -> QueueId {
    QueueId::from_ulid(Ulid(n))
}

fn org() -> OrgId {
    OrgId::from_ulid(Ulid(1))
}

fn instance(n: u128) -> ScmInstanceId {
    ScmInstanceId::from_ulid(Ulid(n))
}

fn connection() -> ConnectionId {
    ConnectionId::from_ulid(Ulid(1))
}

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(1_789_000_000 + seconds).unwrap()
}

/// Two SCM instances, one organization and one repository with three queues:
/// 1 is dirty, 2 has a tick due at `at(0)`, 3 has no work.
async fn seed(pool: &PgPool) {
    let (org, i1, i2, c) = (org(), instance(1), instance(2), connection());
    let (q1, q2, q3) = (queue(1), queue(2), queue(3));
    let mut tx = begin_system(pool).await.unwrap();
    sqlx::raw_sql(AssertSqlSafe(format!(
        "INSERT INTO scm_instances (id, kind, base_url) VALUES
             ('{i1}', 'github', 'https://github.com'),
             ('{i2}', 'github', 'https://github.example.com');
         INSERT INTO orgs (id, slug) VALUES ('{org}', 'org');
         INSERT INTO scm_accounts (id, org_id, scm_instance_id, external_id, path)
             VALUES ('a', '{org}', '{i1}', '1', 'acme');
         INSERT INTO scm_connections (id, org_id, scm_instance_id, scm_account_id, kind, external_id)
             VALUES ('{c}', '{org}', '{i1}', 'a', 'github_app', '1');
         INSERT INTO repos (id, org_id, scm_instance_id, connection_id, external_id, namespace, name,
                            default_branch)
             VALUES ('r', '{org}', '{i1}', '{c}', '1', 'acme', 'app', 'main');
         INSERT INTO queues (id, org_id, repo_id, target_branch, name, dirty, next_tick_at) VALUES
             ('{q1}', '{org}', 'r', 'main', 'main', true, NULL),
             ('{q2}', '{org}', 'r', 'release', 'release', false, '{}'),
             ('{q3}', '{org}', 'r', 'next', 'next', false, NULL);",
        at(0)
    )))
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
}

async fn acquire(pool: &PgPool, owner: &str, now: OffsetDateTime) -> Vec<QueueId> {
    let mut tx = begin_system(pool).await.unwrap();
    let queues = acquire_leases(&mut tx, owner, now, 10).await.unwrap();
    tx.commit().await.unwrap();
    queues
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_lease_goes_to_one_owner_until_it_expires(pool: PgPool) {
    seed(&pool).await;
    // Only the queues with work, the dirty one first.
    assert_eq!(acquire(&pool, "a", at(0)).await, [queue(1), queue(2)]);
    assert_eq!(acquire(&pool, "b", at(1)).await, []);
    // Its owner renews a lease.
    assert_eq!(acquire(&pool, "a", at(10)).await, [queue(1), queue(2)]);
    assert_eq!(acquire(&pool, "b", at(20)).await, []);
    // Not renewed, it expires, and another owner takes it.
    let expired = at(10) + LEASE_TTL;
    assert_eq!(acquire(&pool, "b", expired).await, [queue(1), queue(2)]);
    assert_eq!(acquire(&pool, "a", expired + Duration::SECOND).await, []);
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn two_owners_at_the_same_time_never_share_a_queue(pool: PgPool) {
    seed(&pool).await;
    for round in 0..20 {
        let now = at(round * 100);
        let (a, b) = tokio::join!(acquire(&pool, "a", now), acquire(&pool, "b", now));
        let mut all = [a.clone(), b.clone()].concat();
        all.sort_unstable();
        assert_eq!(all, [queue(1), queue(2)], "round {round}: {a:?} {b:?}");
    }
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_released_lease_is_free_at_once(pool: PgPool) {
    seed(&pool).await;
    acquire(&pool, "a", at(0)).await;
    let mut tx = begin_system(&pool).await.unwrap();
    assert_eq!(release_leases(&mut tx, "a").await.unwrap(), 2);
    tx.commit().await.unwrap();
    assert_eq!(acquire(&pool, "b", at(1)).await, [queue(1), queue(2)]);
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn one_transaction_at_a_time_holds_the_lock_of_a_queue(pool: PgPool) {
    let mut first = pool.begin().await.unwrap();
    let mut second = pool.begin().await.unwrap();
    assert!(lock_queue(&mut first, queue(1)).await.unwrap());
    assert!(!lock_queue(&mut second, queue(1)).await.unwrap());
    assert!(lock_queue(&mut second, queue(2)).await.unwrap());
    // The lock ends with the transaction.
    first.commit().await.unwrap();
    assert!(lock_queue(&mut second, queue(1)).await.unwrap());
}

fn delivery(id: &str, received: i64) -> Delivery {
    Delivery {
        scm_instance_id: instance(1),
        delivery_id: id.to_owned(),
        org_id: Some(org()),
        connection_id: Some(connection()),
        queue_id: Some(queue(1)),
        event: "pull_request".to_owned(),
        action: Some("opened".to_owned()),
        payload: json!({ "number": 12 }),
        received_at: at(received),
    }
}

async fn store(pool: &PgPool, delivery: &Delivery) -> bool {
    let mut tx = begin_system(pool).await.unwrap();
    let stored = store_delivery(&mut tx, delivery).await.unwrap();
    tx.commit().await.unwrap();
    stored
}

async fn pending(pool: &PgPool) -> Vec<Delivery> {
    let mut tx = begin_system(pool).await.unwrap();
    pending_deliveries(&mut tx, queue(1), 10).await.unwrap()
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_copy_of_a_delivery_changes_nothing(pool: PgPool) {
    seed(&pool).await;
    let first = delivery("d1", 0);
    assert!(store(&pool, &first).await);
    let copy = Delivery {
        payload: json!({ "number": 99 }),
        received_at: at(5),
        ..first.clone()
    };
    assert!(!store(&pool, &copy).await);
    assert_eq!(pending(&pool).await, [first]);
    // A delivery identifier is unique within its SCM instance only.
    let unrouted = Delivery {
        scm_instance_id: instance(2),
        org_id: None,
        connection_id: None,
        queue_id: None,
        ..delivery("d1", 0)
    };
    assert!(store(&pool, &unrouted).await);
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn deliveries_come_in_order_until_processed(pool: PgPool) {
    seed(&pool).await;
    for (id, received) in [("d2", 2), ("d1", 1), ("d3", 3)] {
        store(&pool, &delivery(id, received)).await;
    }
    let ids = |deliveries: Vec<Delivery>| -> Vec<String> {
        deliveries.into_iter().map(|d| d.delivery_id).collect()
    };
    assert_eq!(ids(pending(&pool).await), ["d1", "d2", "d3"]);
    let mut tx = begin_system(&pool).await.unwrap();
    mark_processed(&mut tx, instance(1), "d1", at(10), None)
        .await
        .unwrap();
    mark_processed(&mut tx, instance(1), "d2", at(10), Some("unknown action"))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(ids(pending(&pool).await), ["d3"]);
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn an_unrouted_delivery_is_seen_by_the_system_only(pool: PgPool) {
    seed(&pool).await;
    let unrouted = Delivery {
        org_id: None,
        connection_id: None,
        queue_id: None,
        ..delivery("d1", 0)
    };
    assert!(store(&pool, &unrouted).await);
    let mut tx = begin_org(&pool, org()).await.unwrap();
    let seen: i64 = sqlx::query_scalar("SELECT count(*) FROM webhook_inbox")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(seen, 0);
    // A routed delivery needs its organization.
    let orphan = Delivery {
        org_id: None,
        ..delivery("d2", 0)
    };
    let mut tx = begin_system(&pool).await.unwrap();
    assert!(store_delivery(&mut tx, &orphan).await.is_err());
}

fn job(key: &str, class: JobClass) -> Job {
    Job {
        idempotency_key: key.to_owned(),
        job_type: "test".to_owned(),
        class,
        org_id: Some(org()),
        connection: Some((instance(1), connection())),
        queue_id: Some(queue(1)),
        payload: json!({}),
        attempts: 0,
    }
}

/// A job of the system: no organization, no connection, no quota.
fn system_job(key: &str) -> Job {
    Job {
        org_id: None,
        connection: None,
        queue_id: None,
        ..job(key, JobClass::Repair)
    }
}

async fn enqueue(pool: &PgPool, job: &Job, run_after: OffsetDateTime) -> bool {
    let mut tx = begin_system(pool).await.unwrap();
    let enqueued = enqueue_job(&mut tx, job, run_after, at(0)).await.unwrap();
    tx.commit().await.unwrap();
    enqueued
}

async fn claim(pool: &PgPool, worker: &str, now: OffsetDateTime, limit: i64) -> Vec<String> {
    let mut tx = begin_system(pool).await.unwrap();
    let jobs = claim_jobs(&mut tx, worker, now, limit).await.unwrap();
    tx.commit().await.unwrap();
    jobs.into_iter().map(|job| job.idempotency_key).collect()
}

async fn complete(pool: &PgPool, key: &str, worker: &str) -> bool {
    let mut tx = begin_system(pool).await.unwrap();
    let done = complete_job(&mut tx, key, worker, at(0)).await.unwrap();
    tx.commit().await.unwrap();
    done
}

async fn fail(pool: &PgPool, key: &str, now: OffsetDateTime, retry: Retry) -> bool {
    let mut tx = begin_system(pool).await.unwrap();
    let state = fail_job(&mut tx, key, "a", now, "boom", retry)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    state.is_some()
}

async fn state(pool: &PgPool, key: &str) -> (String, i32) {
    let mut tx = begin_system(pool).await.unwrap();
    sqlx::query_as("SELECT state::text, attempts FROM jobs WHERE idempotency_key = $1")
        .bind(key)
        .fetch_one(&mut *tx)
        .await
        .unwrap()
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_second_request_of_a_job_changes_nothing(pool: PgPool) {
    seed(&pool).await;
    assert!(enqueue(&pool, &job("merge_pr:item:1", JobClass::Merge), at(0)).await);
    assert!(!enqueue(&pool, &job("merge_pr:item:1", JobClass::Repair), at(9)).await);
    assert_eq!(claim(&pool, "a", at(0), 10).await, ["merge_pr:item:1"]);
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn due_jobs_come_by_class_then_by_time(pool: PgPool) {
    seed(&pool).await;
    enqueue(&pool, &system_job("repair"), at(0)).await;
    enqueue(&pool, &job("merge-late", JobClass::Merge), at(2)).await;
    enqueue(&pool, &job("merge-early", JobClass::Merge), at(1)).await;
    enqueue(&pool, &job("interactive", JobClass::Interactive), at(3)).await;
    enqueue(&pool, &job("later", JobClass::Interactive), at(100)).await;
    assert_eq!(
        claim(&pool, "a", at(3), 10).await,
        ["interactive", "merge-early", "merge-late", "repair"]
    );
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_job_of_a_stopped_worker_is_taken_back_and_finished_once(pool: PgPool) {
    seed(&pool).await;
    enqueue(&pool, &job("j", JobClass::Merge), at(0)).await;
    assert_eq!(claim(&pool, "a", at(0), 10).await, ["j"]);
    // Worker a stops: the job stays with it until its lock expires.
    assert_eq!(claim(&pool, "b", at(30), 10).await, Vec::<String>::new());
    assert_eq!(claim(&pool, "b", at(0) + JOB_LOCK, 10).await, ["j"]);
    assert_eq!(state(&pool, "j").await, ("running".to_owned(), 2));
    // A late answer of worker a changes nothing: worker b finishes the job.
    assert!(!complete(&pool, "j", "a").await);
    assert!(!fail(&pool, "j", at(70), Retry::Later).await);
    assert!(complete(&pool, "j", "b").await);
    assert_eq!(state(&pool, "j").await, ("done".to_owned(), 2));
    assert_eq!(
        claim(&pool, "c", at(10_000), 10).await,
        Vec::<String>::new()
    );
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn two_workers_at_the_same_time_never_share_a_job(pool: PgPool) {
    seed(&pool).await;
    let keys: Vec<String> = (0..30).map(|n| format!("j{n:02}")).collect();
    for key in &keys {
        enqueue(&pool, &job(key, JobClass::Speculate), at(0)).await;
    }
    let (a, b) = tokio::join!(claim(&pool, "a", at(1), 20), claim(&pool, "b", at(1), 20));
    assert_eq!(a.len() + b.len(), 30, "{a:?} {b:?}");
    let mut all = [a, b].concat();
    all.sort_unstable();
    assert_eq!(all, keys);
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_failed_job_waits_then_dies_after_its_last_attempt(pool: PgPool) {
    seed(&pool).await;
    enqueue(&pool, &job("j", JobClass::Merge), at(0)).await;
    let mut now = at(0);
    for attempt in 1..=MAX_ATTEMPTS {
        assert_eq!(claim(&pool, "a", now, 10).await, ["j"], "attempt {attempt}");
        assert!(fail(&pool, "j", now, Retry::Later).await);
        now += backoff(attempt);
        if attempt < MAX_ATTEMPTS {
            // Not before its backoff.
            assert_eq!(
                claim(&pool, "a", now - Duration::SECOND, 10).await,
                Vec::<String>::new()
            );
        }
    }
    assert_eq!(state(&pool, "j").await, ("dead".to_owned(), MAX_ATTEMPTS));
    assert_eq!(claim(&pool, "a", now, 10).await, Vec::<String>::new());

    enqueue(&pool, &job("k", JobClass::Merge), at(0)).await;
    claim(&pool, "a", now, 10).await;
    assert!(fail(&pool, "k", now, Retry::Never).await);
    assert_eq!(state(&pool, "k").await, ("failed".to_owned(), 1));
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_connection_gets_no_more_jobs_past_its_quota(pool: PgPool) {
    seed(&pool).await;
    let quota = usize::try_from(JOBS_PER_CONNECTION).unwrap();
    for n in 0..=quota {
        enqueue(&pool, &job(&format!("j{n:03}"), JobClass::Speculate), at(0)).await;
    }
    enqueue(&pool, &system_job("system"), at(0)).await;
    let first = claim(&pool, "a", at(1), 1_000).await;
    assert_eq!(first.len(), quota + 1);
    assert!(first.contains(&"system".to_owned()));
    // The last job waits until a job of the connection ends.
    assert_eq!(claim(&pool, "b", at(2), 1_000).await, Vec::<String>::new());
    complete(&pool, "j000", "a").await;
    assert_eq!(
        claim(&pool, "b", at(3), 1_000).await,
        [format!("j{quota:03}")]
    );
}

fn budget(paused_until: Option<OffsetDateTime>) -> ConnectionBudget {
    ConnectionBudget {
        mode: ConnectionMode::Normal,
        mode_reason: None,
        rate_remaining: None,
        rate_limit: None,
        rate_reset_at: None,
        paused_until,
        consecutive_failures: 0,
    }
}

async fn record(pool: &PgPool, budget: ConnectionBudget) {
    let mut tx = begin_system(pool).await.unwrap();
    record_budgets(&mut tx, &[(connection(), budget)])
        .await
        .unwrap();
    tx.commit().await.unwrap();
}

async fn pause(pool: &PgPool, until: OffsetDateTime) {
    record(pool, budget(Some(until))).await;
}

type BudgetRow = (
    String,
    Option<String>,
    Option<i32>,
    Option<i32>,
    Option<OffsetDateTime>,
    Option<OffsetDateTime>,
    i32,
);

async fn recorded_budget(pool: &PgPool) -> BudgetRow {
    let mut tx = begin_system(pool).await.unwrap();
    sqlx::query_as(
        "SELECT mode::text, mode_reason, rate_remaining, rate_limit, rate_reset_at, paused_until,
                consecutive_failures
         FROM scm_connections WHERE id = $1",
    )
    .bind(connection().to_string())
    .fetch_one(&mut *tx)
    .await
    .unwrap()
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_budget_is_recorded_on_its_connection(pool: PgPool) {
    seed(&pool).await;
    record(
        &pool,
        ConnectionBudget {
            mode: ConnectionMode::ScmUnavailable,
            mode_reason: Some("3 failures in a row".into()),
            rate_remaining: Some(900),
            rate_limit: Some(5_000),
            rate_reset_at: Some(at(3_600)),
            paused_until: None,
            consecutive_failures: 3,
        },
    )
    .await;
    assert_eq!(
        recorded_budget(&pool).await,
        (
            "scm_unavailable".into(),
            Some("3 failures in a row".into()),
            Some(900),
            Some(5_000),
            Some(at(3_600)),
            None,
            3
        )
    );
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn an_urgent_budget_is_written_before_the_next_period(pool: PgPool) {
    seed(&pool).await;
    let changed = Arc::new(std::sync::Mutex::new(Vec::new()));
    let urgent = Arc::new(tokio::sync::Notify::new());
    let cancel = CancellationToken::new();
    let writer = tokio::spawn(run_budget_writer(
        pool.clone(),
        {
            let changed = Arc::clone(&changed);
            move || std::mem::take(&mut *changed.lock().unwrap())
        },
        Arc::clone(&urgent),
        cancel.clone(),
    ));
    // The first period passes at once; the next one is 5 s later.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    changed
        .lock()
        .unwrap()
        .push((connection(), budget(Some(at(60)))));
    urgent.notify_one();
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while recorded_budget(&pool).await.5.is_none() {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(recorded_budget(&pool).await.5, Some(at(60)));
    cancel.cancel();
    writer.await.unwrap();
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_paused_connection_gets_no_job_until_the_end_of_its_pause(pool: PgPool) {
    seed(&pool).await;
    enqueue(&pool, &job("connection", JobClass::Interactive), at(0)).await;
    enqueue(&pool, &system_job("system"), at(0)).await;
    pause(&pool, at(60)).await;
    assert_eq!(claim(&pool, "a", at(10), 10).await, ["system"]);
    pause(&pool, at(30)).await;
    assert_eq!(claim(&pool, "a", at(40), 10).await, Vec::<String>::new());
    assert_eq!(claim(&pool, "a", at(60), 10).await, ["connection"]);
}

async fn queue_state(pool: &PgPool, queue: QueueId) -> (bool, Option<OffsetDateTime>) {
    let mut tx = begin_system(pool).await.unwrap();
    sqlx::query_as("SELECT dirty, next_tick_at FROM queues WHERE id = $1")
        .bind(queue.to_string())
        .fetch_one(&mut *tx)
        .await
        .unwrap()
}

const SHORT_TICK: std::time::Duration = std::time::Duration::from_millis(200);

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_new_delivery_wakes_a_scheduler_within_a_second(pool: PgPool) {
    seed(&pool).await;
    let cancel = CancellationToken::new();
    let mut wakeup = QueueWakeup::listen(&pool).await.unwrap();
    assert_eq!(wakeup.wait(SHORT_TICK, &cancel).await.unwrap(), Wake::Tick);

    let queue_3 = Delivery {
        queue_id: Some(queue(3)),
        ..delivery("d1", 0)
    };
    store(&pool, &queue_3).await;
    let woken = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        wakeup.wait(std::time::Duration::from_secs(60), &cancel),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(woken, Wake::Notified);
    assert!(queue_state(&pool, queue(3)).await.0);

    // A copy is not new work: no notification.
    store(&pool, &queue_3).await;
    assert_eq!(wakeup.wait(SHORT_TICK, &cancel).await.unwrap(), Wake::Tick);
    cancel.cancel();
    assert_eq!(
        wakeup.wait(SHORT_TICK, &cancel).await.unwrap(),
        Wake::Cancelled
    );
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_tick_finds_a_queue_whose_notification_was_lost(pool: PgPool) {
    seed(&pool).await;
    let cancel = CancellationToken::new();
    let mut wakeup = QueueWakeup::listen(&pool).await.unwrap();
    // Dirty without a notification, as when the listener was down.
    let mut tx = begin_system(&pool).await.unwrap();
    sqlx::query("UPDATE queues SET dirty = true WHERE id = $1")
        .bind(queue(3).to_string())
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(wakeup.wait(SHORT_TICK, &cancel).await.unwrap(), Wake::Tick);
    assert!(acquire(&pool, "a", at(0)).await.contains(&queue(3)));
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_write_during_a_reconciliation_marks_the_queue_again(pool: PgPool) {
    seed(&pool).await;
    let mut tx = begin_reconciliation(&pool, queue(1))
        .await
        .unwrap()
        .unwrap();
    assert!(
        begin_reconciliation(&pool, queue(1))
            .await
            .unwrap()
            .is_none()
    );

    // A delivery arrives during the reconciliation: it waits for the commit.
    let writer = tokio::spawn({
        let pool = pool.clone();
        async move { store(&pool, &delivery("d1", 0)).await }
    });
    tokio::time::sleep(SHORT_TICK).await;
    assert!(!writer.is_finished());

    schedule_tick(&mut tx, queue(1), Some(at(50)))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert!(writer.await.unwrap());
    assert_eq!(queue_state(&pool, queue(1)).await, (true, Some(at(50))));

    // Without a write, the queue stays clean after its reconciliation.
    let tx = begin_reconciliation(&pool, queue(1))
        .await
        .unwrap()
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(queue_state(&pool, queue(1)).await, (false, Some(at(50))));
}

/// Starts a worker whose clock stays at `now`, and returns its handle.
fn start_worker(
    pool: &PgPool,
    now: OffsetDateTime,
    idle_wait: std::time::Duration,
    cancel: &CancellationToken,
) -> tokio::task::JoinHandle<()> {
    let mut executors = Executors::default();
    executors.register("ok", |_, _, _| async { Outcome::Done });
    executors.register("later", |_, _, _| async {
        Outcome::Retry("not yet".to_owned())
    });
    let (pool, cancel) = (pool.clone(), cancel.clone());
    tokio::spawn(async move {
        run_worker(
            pool,
            "w".to_owned(),
            Arc::new(executors),
            move || now,
            idle_wait,
            cancel,
        )
        .await
        .unwrap();
    })
}

fn typed_job(key: &str, job_type: &str) -> Job {
    Job {
        job_type: job_type.to_owned(),
        ..system_job(key)
    }
}

/// Waits until the job `key` has `wanted` as its state, for at most 2 s.
async fn wait_for_state(pool: &PgPool, key: &str, wanted: &str) -> (String, i32) {
    for _ in 0..40 {
        let current = state(pool, key).await;
        if current.0 == wanted {
            return current;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    state(pool, key).await
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_worker_runs_each_job_with_its_executor(pool: PgPool) {
    seed(&pool).await;
    for (key, job_type) in [("a", "ok"), ("b", "later"), ("c", "unknown")] {
        enqueue(&pool, &typed_job(key, job_type), at(0)).await;
    }
    let cancel = CancellationToken::new();
    let worker = start_worker(&pool, at(10), SHORT_TICK, &cancel);
    assert_eq!(
        wait_for_state(&pool, "a", "done").await,
        ("done".to_owned(), 1)
    );
    assert_eq!(
        wait_for_state(&pool, "b", "queued").await,
        ("queued".to_owned(), 1)
    );
    assert_eq!(
        wait_for_state(&pool, "c", "failed").await,
        ("failed".to_owned(), 1)
    );
    let mut tx = begin_system(&pool).await.unwrap();
    let errors: Vec<Option<String>> =
        sqlx::query_scalar("SELECT last_error FROM jobs ORDER BY idempotency_key")
            .fetch_all(&mut *tx)
            .await
            .unwrap();
    assert_eq!(
        errors,
        [
            None,
            Some("not yet".to_owned()),
            Some("no executor for job type `unknown`".to_owned())
        ]
    );
    cancel.cancel();
    worker.await.unwrap();
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_new_job_wakes_a_worker_within_a_second(pool: PgPool) {
    seed(&pool).await;
    let cancel = CancellationToken::new();
    let worker = start_worker(&pool, at(10), std::time::Duration::from_secs(60), &cancel);
    tokio::time::sleep(SHORT_TICK).await;
    enqueue(&pool, &typed_job("a", "ok"), at(0)).await;
    assert_eq!(wait_for_state(&pool, "a", "done").await.0, "done");
    cancel.cancel();
    worker.await.unwrap();
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn maintenance_runs_once_a_day_and_purges_old_rows(pool: PgPool) {
    seed(&pool).await;
    let now = at(30 * 86_400);
    let mut tx = begin_system(&pool).await.unwrap();
    assert!(request_maintenance(&mut tx, now).await.unwrap());
    assert!(
        !request_maintenance(&mut tx, now + Duration::hours(1))
            .await
            .unwrap()
    );
    tx.commit().await.unwrap();
    assert_eq!(
        maintenance_job(now.date()).idempotency_key,
        "maintenance:2026-10-10:1"
    );

    // Old and recent rows: only the old ones go.
    for (id, processed) in [
        ("old", now - Duration::days(8)),
        ("recent", now - Duration::days(1)),
    ] {
        store(&pool, &delivery(id, 0)).await;
        let mut tx = begin_system(&pool).await.unwrap();
        mark_processed(&mut tx, instance(1), id, processed, None)
            .await
            .unwrap();
        tx.commit().await.unwrap();
    }
    // A delivery that no connection claimed goes after the same time.
    for (id, received) in [("unrouted-old", 22), ("unrouted-recent", 29)] {
        let unrouted = Delivery {
            org_id: None,
            connection_id: None,
            queue_id: None,
            ..delivery(id, received * 86_400)
        };
        store(&pool, &unrouted).await;
    }
    for (key, finished) in [
        ("old", now - Duration::days(4)),
        ("recent", now - Duration::days(1)),
    ] {
        enqueue(&pool, &system_job(key), at(0)).await;
        claim(&pool, "a", at(0), 1).await;
        let mut tx = begin_system(&pool).await.unwrap();
        complete_job(&mut tx, key, "a", finished).await.unwrap();
        tx.commit().await.unwrap();
    }
    run_maintenance(&pool, now).await.unwrap();

    let mut tx = begin_system(&pool).await.unwrap();
    let deliveries: Vec<String> =
        sqlx::query_scalar("SELECT delivery_id FROM webhook_inbox ORDER BY 1")
            .fetch_all(&mut *tx)
            .await
            .unwrap();
    assert_eq!(deliveries, ["recent", "unrouted-recent"]);
    let jobs: Vec<String> = sqlx::query_scalar("SELECT idempotency_key FROM jobs ORDER BY 1")
        .fetch_all(&mut *tx)
        .await
        .unwrap();
    assert_eq!(jobs, ["maintenance:2026-10-10:1", "recent"]);
    let partitions: Vec<String> = sqlx::query_scalar(
        "SELECT c.relname::text FROM pg_inherits i JOIN pg_class c ON c.oid = i.inhrelid
         WHERE i.inhparent = 'events'::regclass ORDER BY 1",
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap();
    assert_eq!(partitions, ["events_2026_10", "events_2026_11"]);
}

/// A delivery of a fake SCM whose payload says what it is: the routing does
/// not depend on one SCM.
fn fake_translate(
    event: &str,
    payload: &serde_json::Value,
) -> Result<ScmDelivery, std::fmt::Error> {
    let text = |key: &str| payload[key].as_str().unwrap_or_default().to_owned();
    let scm_event = match event {
        "pr" => ScmEvent::PrChanged {
            repo_external_id: text("repo"),
            number: 1,
            base_branch: text("branch"),
            label_added: None,
            opened: false,
        },
        "push" => ScmEvent::Pushed {
            repo_external_id: text("repo"),
            branch: text("branch"),
            before: None,
            after: Some("a".repeat(40)),
            sender: ScmUser {
                external_id: "3".into(),
                login: "alice".into(),
            },
        },
        "rules" => ScmEvent::RulesChanged {
            repo_external_id: text("repo"),
        },
        "repos" => ScmEvent::ReposChanged {
            added: vec![text("repo")],
            removed: Vec::new(),
        },
        "broken" => return Err(std::fmt::Error),
        _ => ScmEvent::Ignored,
    };
    Ok(ScmDelivery {
        connection_external_id: payload["installation"].as_str().map(str::to_owned),
        event: scm_event,
    })
}

fn unrouted(id: &str, event: &str, payload: serde_json::Value) -> Delivery {
    Delivery {
        org_id: None,
        connection_id: None,
        queue_id: None,
        event: event.to_owned(),
        action: None,
        payload,
        ..delivery(id, 0)
    }
}

type RouteRow = (
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    bool,
    bool,
);

async fn routes(pool: &PgPool) -> Vec<RouteRow> {
    let mut tx = begin_system(pool).await.unwrap();
    sqlx::query_as(
        "SELECT delivery_id, org_id, connection_id, queue_id, routed_at IS NOT NULL,
                processed_at IS NOT NULL
         FROM webhook_inbox ORDER BY delivery_id",
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap()
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_router_routes_each_delivery_to_its_connection_and_sure_queue(pool: PgPool) {
    seed(&pool).await;
    // Two queues on one branch: no queue is sure (`DECISIONS.md` D135).
    let mut tx = begin_system(&pool).await.unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "INSERT INTO queues (id, org_id, repo_id, target_branch, name) VALUES
             ('{}', '{org}', 'r', 'shared', 'shared'),
             ('{}', '{org}', 'r', 'shared', 'hotfix');",
        queue(4),
        queue(5),
        org = org(),
    )))
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let cancel = CancellationToken::new();
    let router = tokio::spawn(run_router(
        pool.clone(),
        fake_translate,
        || at(5),
        std::time::Duration::from_secs(60),
        cancel.clone(),
    ));
    // The first pass finds nothing; a notification wakes the next one.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let known = json!({ "installation": "1", "repo": "1", "branch": "release" });
    for delivery in [
        unrouted("a-pr", "pr", known.clone()),
        unrouted(
            "b-push",
            "push",
            json!({ "installation": "1", "repo": "1", "branch": "next" }),
        ),
        unrouted(
            "c-other-branch",
            "pr",
            json!({ "installation": "1", "repo": "1", "branch": "dev" }),
        ),
        unrouted("d-rules", "rules", known.clone()),
        unrouted(
            "e-unknown",
            "pr",
            json!({ "installation": "99", "repo": "1", "branch": "main" }),
        ),
        unrouted("f-ignored", "ping", known.clone()),
        unrouted("g-broken", "broken", known),
        unrouted(
            "h-shared-branch",
            "pr",
            json!({ "installation": "1", "repo": "1", "branch": "shared" }),
        ),
        unrouted(
            "i-repos",
            "repos",
            json!({ "installation": "1", "repo": "2" }),
        ),
        unrouted("j-repos-unknown", "repos", json!({ "installation": "99" })),
    ] {
        store(&pool, &delivery).await;
    }
    let (o, c) = (Some(org().to_string()), Some(connection().to_string()));
    let expected: Vec<RouteRow> = vec![
        (
            "a-pr".into(),
            o.clone(),
            c.clone(),
            Some(queue(2).to_string()),
            true,
            false,
        ),
        (
            "b-push".into(),
            o.clone(),
            c.clone(),
            Some(queue(3).to_string()),
            true,
            false,
        ),
        (
            "c-other-branch".into(),
            o.clone(),
            c.clone(),
            None,
            true,
            false,
        ),
        ("d-rules".into(), o.clone(), c.clone(), None, true, false),
        ("e-unknown".into(), None, None, None, true, false),
        ("f-ignored".into(), None, None, None, true, true),
        ("g-broken".into(), None, None, None, true, true),
        (
            "h-shared-branch".into(),
            o.clone(),
            c.clone(),
            None,
            true,
            false,
        ),
        ("i-repos".into(), o, c, None, true, false),
        ("j-repos-unknown".into(), None, None, None, true, false),
    ];
    let _ = tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while routes(&pool).await != expected {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await;
    assert_eq!(routes(&pool).await, expected, "routed within a second");
    // The queues of a routed delivery are dirty.
    assert!(queue_state(&pool, queue(2)).await.0);
    assert!(queue_state(&pool, queue(3)).await.0);
    // A change of the repositories of a known connection asks to read them
    // again (`DECISIONS.md` D155).
    assert_eq!(
        sync_jobs(&pool).await,
        [format!("sync_repos:{}-i-repos:1", connection())]
    );
    cancel.cancel();
    router.await.unwrap().unwrap();
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_sweep_asks_again_only_for_the_deliveries_missing_from_the_inbox(pool: PgPool) {
    seed(&pool).await;
    let repo = RepoRef {
        connection: ConnectionRef {
            id: connection(),
            external_id: "1".into(),
        },
        external_id: "1".into(),
        namespace: "acme".into(),
        name: "app".into(),
    };
    let sim = Sim::new();
    let root = sim.add_repo(&repo, "main", &[]);
    let alice = ScmUser {
        external_id: "3".into(),
        login: "alice".into(),
    };
    for branch in ["a", "b", "c"] {
        sim.push(&repo, branch, &root, &alice).unwrap();
    }
    let sent = sim.take_deliveries();
    // The inbox got the first and the third delivery; the second was lost.
    for id in ["sim-1", "sim-3"] {
        store(&pool, &unrouted(id, "push", json!({}))).await;
    }
    assert_eq!(run_sweep(&pool, &sim, instance(1), at(0)).await.unwrap(), 1);
    assert_eq!(sim.take_deliveries(), [sent[1].clone()]);
}

async fn sync_jobs(pool: &PgPool) -> Vec<String> {
    let mut tx = begin_system(pool).await.unwrap();
    sqlx::query_scalar(
        "SELECT idempotency_key FROM jobs WHERE type = 'sync_repos' ORDER BY idempotency_key",
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap()
}

type OrgRow = (String, String, String, String, bool, bool);

/// The organizations with their account, connection and module.
async fn orgs(pool: &PgPool) -> Vec<OrgRow> {
    let mut tx = begin_system(pool).await.unwrap();
    sqlx::query_as(
        "SELECT o.slug, a.external_id, a.path, c.external_id, c.claimed_at IS NOT NULL,
                EXISTS (SELECT 1 FROM org_modules m WHERE m.org_id = o.id AND m.module = 'mergequeue')
         FROM orgs o
         JOIN scm_accounts a ON a.org_id = o.id
         JOIN scm_connections c ON c.scm_account_id = a.id
         ORDER BY o.slug",
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap()
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_bootstrap_starts_an_organization_and_a_second_one_gives_a_token(pool: PgPool) {
    seed(&pool).await;
    let sim = Sim::new();
    let account = ScmAccount {
        external_id: "70".into(),
        path: "globex".into(),
    };
    sim.add_account("7", account);
    let kind = ConnectionKind::GithubApp;

    let first = bootstrap(&pool, &sim, instance(1), kind, "7", at(0))
        .await
        .unwrap();
    assert!(first.created);
    let globex = (
        "globex".to_owned(),
        "70".to_owned(),
        "globex".to_owned(),
        "7".to_owned(),
        true,
        true,
    );
    let acme = (
        "org".to_owned(),
        "1".to_owned(),
        "acme".to_owned(),
        "1".to_owned(),
        false,
        false,
    );
    assert_eq!(orgs(&pool).await, [globex.clone(), acme.clone()]);
    let grant = authenticate(&pool, &first.token, at(0))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(grant.org_id, first.org_id);
    assert_eq!(grant.scopes.len(), 5, "{:?}", grant.scopes);
    assert!(grant.scopes.contains(&ApiScope::QueuesWrite));
    let read = format!("sync_repos:{}-20260910T00:1", first.connection_id);
    assert_eq!(sync_jobs(&pool).await, std::slice::from_ref(&read));

    // Again, in the same hour: the same organization and a new token; the
    // read of the hour is already requested.
    let second = bootstrap(&pool, &sim, instance(1), kind, "7", at(60))
        .await
        .unwrap();
    assert!(!second.created);
    assert_eq!(
        (second.org_id, second.connection_id),
        (first.org_id, first.connection_id)
    );
    assert_eq!(orgs(&pool).await, [globex, acme]);
    for token in [&first.token, &second.token] {
        let grant = authenticate(&pool, token, at(60)).await.unwrap().unwrap();
        assert_eq!(grant.org_id, first.org_id);
    }
    assert_eq!(sync_jobs(&pool).await, [read]);
    let mut tx = begin_system(&pool).await.unwrap();
    let audited: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE action = 'token.created'")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    drop(tx);
    assert_eq!(audited, 2, "each token is in the audit log");

    // An installation that the SCM does not know starts nothing.
    let unknown = bootstrap(&pool, &sim, instance(1), kind, "8", at(0)).await;
    assert!(
        matches!(unknown, Err(PlatformError::Scm(ScmError::NotFound(_)))),
        "{:?}",
        unknown.err()
    );
    assert_eq!(orgs(&pool).await.len(), 2);
}

type RepoRow = (String, String, String, String, String, String);

async fn repos(pool: &PgPool) -> Vec<RepoRow> {
    let mut tx = begin_system(pool).await.unwrap();
    sqlx::query_as(
        "SELECT external_id, org_id, connection_id, namespace, name, default_branch
         FROM repos ORDER BY external_id",
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap()
}

fn sim_repo(connection: &str, external_id: &str, name: &str) -> RepoRef {
    RepoRef {
        connection: ConnectionRef {
            id: ConnectionId::from_ulid(Ulid(9)),
            external_id: connection.into(),
        },
        external_id: external_id.into(),
        namespace: "acme".into(),
        name: name.into(),
    }
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_read_of_the_repositories_creates_and_updates_them(pool: PgPool) {
    seed(&pool).await;
    // Another organization has the repository 4.
    let mut tx = begin_system(&pool).await.unwrap();
    sqlx::raw_sql(AssertSqlSafe(format!(
        "INSERT INTO orgs (id, slug) VALUES ('other', 'other');
         INSERT INTO scm_accounts (id, org_id, scm_instance_id, external_id, path)
             VALUES ('b', 'other', '{i1}', '2', 'other');
         INSERT INTO scm_connections (id, org_id, scm_instance_id, scm_account_id, kind, external_id)
             VALUES ('c2', 'other', '{i1}', 'b', 'github_app', '2');
         INSERT INTO repos (id, org_id, scm_instance_id, connection_id, external_id, namespace, name,
                            default_branch)
             VALUES ('r4', 'other', '{i1}', 'c2', '4', 'other', 'kept', 'main');",
        i1 = instance(1),
    )))
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let sim = Sim::new();
    sim.add_repo(&sim_repo("1", "1", "renamed"), "trunk", &[]);
    sim.add_repo(&sim_repo("1", "2", "lib"), "main", &[]);
    sim.add_repo(&sim_repo("1", "4", "taken"), "main", &[]);
    sim.add_repo(&sim_repo("9", "3", "elsewhere"), "main", &[]);

    let job = sync_repos_job(org(), instance(1), connection(), "w");
    assert_eq!(run_sync_repos(&pool, &sim, &job).await.unwrap(), 2);
    let row = |id: &str, org: &str, connection: &str, namespace: &str, name: &str, branch: &str| {
        (
            id.to_owned(),
            org.to_owned(),
            connection.to_owned(),
            namespace.to_owned(),
            name.to_owned(),
            branch.to_owned(),
        )
    };
    let (o, c) = (org().to_string(), connection().to_string());
    let expected = [
        row("1", &o, &c, "acme", "renamed", "trunk"),
        row("2", &o, &c, "acme", "lib", "main"),
        row("4", "other", "c2", "other", "kept", "main"),
    ];
    assert_eq!(repos(&pool).await, expected);
    // The repository 1 keeps its identifier, and its queues with it.
    let mut tx = begin_system(&pool).await.unwrap();
    let id: String = sqlx::query_scalar("SELECT id FROM repos WHERE external_id = '1'")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(id, "r");

    // A suspended connection is not read.
    let mut tx = begin_system(&pool).await.unwrap();
    sqlx::query("UPDATE scm_connections SET suspended_at = now() WHERE id = $1")
        .bind(connection().to_string())
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let calls = sim.calls().len();
    assert_eq!(run_sync_repos(&pool, &sim, &job).await.unwrap(), 0);
    assert_eq!(sim.calls().len(), calls);
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn each_active_connection_is_read_once_an_hour(pool: PgPool) {
    seed(&pool).await;
    let request = |now| {
        let pool = pool.clone();
        async move {
            let mut tx = begin_system(&pool).await.unwrap();
            let requested = request_repo_syncs(&mut tx, now).await.unwrap();
            tx.commit().await.unwrap();
            requested
        }
    };
    assert_eq!(request(at(0)).await, 1);
    // Another scheduler in the same hour asks for nothing more.
    assert_eq!(request(at(60)).await, 0);
    assert_eq!(request(at(3600)).await, 1);
    assert_eq!(
        sync_jobs(&pool).await,
        [
            format!("sync_repos:{}-20260910T00:1", connection()),
            format!("sync_repos:{}-20260910T01:1", connection()),
        ]
    );
}

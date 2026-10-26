//! Tests against Postgres: each `#[sqlx::test]` gets a new database.

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use cihub_domain::{
    Actor, ApiScope, ConnectionId, ConnectionKind, ConnectionMode, Event, EventId, EventKind,
    JobClass, MembershipRole, OrgId, Permission, QueueId, RepoId, UserId,
};
use cihub_platform::{
    Access, ConnectionBudget, Delivery, Executors, JOB_LOCK, JOBS_PER_CONNECTION, Job, LEASE_TTL,
    MAX_ATTEMPTS, MIN_WAIT, Outcome, PlatformError, QueueWakeup, Retry, Subscription, TICK, Wake,
    acquire_leases, backoff, begin_reconciliation, claim_jobs, complete_job, create_token,
    delete_delivery, deliver_all, enqueue_job, fail_job, find_route, lock_queue, maintenance_job,
    pending_deliveries, record_budgets, record_sign_in, release_leases, repo_permission_now,
    request_installations_sync, request_installations_sync_at_start, request_maintenance,
    request_member_syncs, request_repo_syncs, role_now, run_budget_writer, run_maintenance,
    run_router, run_sync_installation, run_sync_installations, run_sync_members, run_sync_repos,
    run_sync_user, run_worker, schedule_tick, store_delivery, sync_installation_executor,
    sync_installation_job, sync_members_job, sync_repos_job, sync_user_job, token_of_person,
    until_next_tick,
};
use cihub_scm::Scm;
use cihub_scm::sim::{Fault, Sim};
use cihub_scm::{
    ConnectionRef, RepoRef, ScmAccount, ScmDelivery, ScmError, ScmEvent, ScmInstallation,
    ScmMembership, ScmPersonToken, ScmProfile, ScmUser, SignInCode, SignInRequest, SignedIn,
    code_challenge,
};
use cihub_store::{
    DatabaseCipher, Membership, UserProfile, append_events, authenticate, begin_org, begin_system,
    create_event_partitions, memberships_of, profile_of,
};
use secrecy::{ExposeSecret, SecretString};
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

fn connection() -> ConnectionId {
    ConnectionId::from_ulid(Ulid(1))
}

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(1_789_000_000 + seconds).unwrap()
}

async fn seed(pool: &PgPool) {
    let (org, c) = (org(), connection());
    let (q1, q2, q3) = (queue(1), queue(2), queue(3));
    let mut tx = begin_system(pool).await.unwrap();

    sqlx::raw_sql(AssertSqlSafe(format!(
        "INSERT INTO orgs (id, slug) VALUES ('{org}', 'org');
         INSERT INTO scm_connections (id, org_id, account_external_id, kind, external_id,
                                      synced_at, permissions)
             VALUES ('{c}', '{org}', '1', 'github_app', '1', '{synced_at}', '{{}}');
         INSERT INTO repos (id, org_id, connection_id, external_id, owner, name, default_branch)
             VALUES ('r', '{org}', '{c}', '1', 'acme', 'app', 'main');
         INSERT INTO queues (id, org_id, repo_id, target_branch, dirty, next_tick_at) VALUES
             ('{q1}', '{org}', 'r', 'main', true, NULL),
             ('{q2}', '{org}', 'r', 'release', false, '{}'),
             ('{q3}', '{org}', 'r', 'next', false, NULL);",
        at(0),
        synced_at = at(0),
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

    assert_eq!(acquire(&pool, "a", at(0)).await, [queue(1), queue(2)]);
    assert_eq!(acquire(&pool, "b", at(1)).await, []);
    assert_eq!(acquire(&pool, "a", at(10)).await, [queue(1), queue(2)]);
    assert_eq!(acquire(&pool, "b", at(20)).await, []);
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
    first.commit().await.unwrap();

    assert!(lock_queue(&mut second, queue(1)).await.unwrap());
}

fn delivery(id: &str, received: i64) -> Delivery {
    Delivery {
        delivery_id: id.to_owned(),
        org_id: Some(org()),
        connection_id: Some(connection()),
        queue_id: Some(queue(1)),
        event: "pull_request".to_owned(),
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
    delete_delivery(&mut tx, "d1").await.unwrap();
    delete_delivery(&mut tx, "d2").await.unwrap();
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
        connection: Some(connection()),
        queue_id: Some(queue(1)),
        payload: json!({}),
        attempts: 0,
    }
}

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
    assert_eq!(claim(&pool, "b", at(30), 10).await, Vec::<String>::new());
    assert_eq!(claim(&pool, "b", at(0) + JOB_LOCK, 10).await, ["j"]);
    assert_eq!(state(&pool, "j").await, ("running".to_owned(), 2));
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

    let mut tx = begin_system(&pool).await.unwrap();
    let a: Vec<String> = claim_jobs(&mut tx, "a", at(1), 20)
        .await
        .unwrap()
        .into_iter()
        .map(|job| job.idempotency_key)
        .collect();
    let b = claim(&pool, "b", at(1), 20).await;

    tx.commit().await.unwrap();

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
    assert_eq!(claim(&pool, "b", at(2), 1_000).await, Vec::<String>::new());
    complete(&pool, "j000", "a").await;

    assert_eq!(
        claim(&pool, "b", at(3), 1_000).await,
        [format!("j{quota:03}")]
    );
}

/// The backlog of a connection at its quota sorts first: it must not hide
/// the job of another connection behind the window of the claim.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_connection_at_its_quota_does_not_hide_the_jobs_of_the_others(pool: PgPool) {
    seed(&pool).await;
    let other = ConnectionId::from_ulid(Ulid(2));
    let mut tx = begin_system(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO scm_connections (id, org_id, account_external_id, kind, external_id,
                                      synced_at, permissions)
         VALUES ($1, $2, '2', 'github_app', '2', $3, '{}')",
    )
    .bind(other.to_string())
    .bind(org().to_string())
    .bind(at(0))
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    for n in 0..JOBS_PER_CONNECTION + 20 {
        enqueue(&pool, &job(&format!("j{n:03}"), JobClass::Speculate), at(0)).await;
    }
    let of_other = Job {
        connection: Some(other),
        queue_id: None,
        ..job("other", JobClass::Speculate)
    };
    enqueue(&pool, &of_other, at(0)).await;
    let first = claim(&pool, "a", at(1), JOBS_PER_CONNECTION).await;

    assert_eq!(first.len(), usize::try_from(JOBS_PER_CONNECTION).unwrap());
    assert!(!first.contains(&"other".to_owned()));
    assert_eq!(claim(&pool, "b", at(2), 1).await, ["other"]);
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
        wakeup.wait(std::time::Duration::from_mins(1), &cancel),
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(woken, Wake::Notified);
    assert!(queue_state(&pool, queue(3)).await.0);

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
async fn the_scheduler_waits_until_the_next_tick(pool: PgPool) {
    seed(&pool).await;
    let mut tx = begin_system(&pool).await.unwrap();
    for (now, wait) in [
        (at(-10), std::time::Duration::from_secs(10)),
        (at(-100), TICK),
        (at(0), TICK),
        (at(-1) + Duration::milliseconds(999), MIN_WAIT),
    ] {
        assert_eq!(until_next_tick(&mut tx, now).await.unwrap(), wait, "{now}");
    }
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

    let tx = begin_reconciliation(&pool, queue(1))
        .await
        .unwrap()
        .unwrap();

    tx.commit().await.unwrap();

    assert_eq!(queue_state(&pool, queue(1)).await, (false, Some(at(50))));
}

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
    let worker = start_worker(&pool, at(10), std::time::Duration::from_mins(1), &cancel);

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

    store(&pool, &delivery("waiting", 0)).await;
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
    enqueue(&pool, &system_job("refused"), at(0)).await;
    claim(&pool, "a", at(0), 1).await;
    fail(&pool, "refused", now - Duration::days(4), Retry::Never).await;

    sqlx::query("INSERT INTO users (id, external_id, login) VALUES ('user-9', '9', 'hubot')")
        .execute(&pool)
        .await
        .unwrap();
    let mut tx = begin_system(&pool).await.unwrap();
    for (user, checked_at) in [
        ("user-9", now - Duration::days(15)),
        ("user-10", now - Duration::days(13)),
    ] {
        sqlx::query(
            "INSERT INTO users (id, external_id, login, synced_at)
             VALUES ($1, $1, $1, $2) ON CONFLICT (id) DO UPDATE SET synced_at = $2",
        )
        .bind(user)
        .bind(checked_at)
        .execute(&mut *tx)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO repo_permissions (org_id, repo_id, user_id, permission, checked_at)
             VALUES ($1, 'r', $2, 'read', $3)",
        )
        .bind(org().to_string())
        .bind(user)
        .bind(checked_at)
        .execute(&mut *tx)
        .await
        .unwrap();
    }
    tx.commit().await.unwrap();
    for (id, expires_at) in [("expired", now), ("live", now + Duration::days(1))] {
        sqlx::query(
            "INSERT INTO sessions (id, user_id, token_hash, created_at, expires_at)
             VALUES ($1, 'user-9', sha256($1::bytea), $2, $3)",
        )
        .bind(id)
        .bind(now - Duration::days(14))
        .bind(expires_at)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO device_codes (id, device_code_hash, user_code, created_at, expires_at)
             VALUES ($1, sha256($1::bytea), $1, $2, $3)",
        )
        .bind(id)
        .bind(now - Duration::minutes(15))
        .bind(expires_at)
        .execute(&pool)
        .await
        .unwrap();
    }

    run_maintenance(&pool, now).await.unwrap();

    let mut tx = begin_system(&pool).await.unwrap();
    let deliveries: Vec<String> =
        sqlx::query_scalar("SELECT delivery_id FROM webhook_inbox ORDER BY 1")
            .fetch_all(&mut *tx)
            .await
            .unwrap();

    assert_eq!(deliveries, ["waiting"]);
    let jobs: Vec<String> = sqlx::query_scalar("SELECT idempotency_key FROM jobs ORDER BY 1")
        .fetch_all(&mut *tx)
        .await
        .unwrap();

    assert_eq!(jobs, ["maintenance:2026-10-10:1", "recent"]);
    let sessions: Vec<String> = sqlx::query_scalar("SELECT id FROM sessions")
        .fetch_all(&mut *tx)
        .await
        .unwrap();

    assert_eq!(sessions, ["live"]);
    let device_codes: Vec<String> = sqlx::query_scalar("SELECT id FROM device_codes")
        .fetch_all(&mut *tx)
        .await
        .unwrap();

    assert_eq!(device_codes, ["live"]);
    let permissions: Vec<String> = sqlx::query_scalar("SELECT user_id FROM repo_permissions")
        .fetch_all(&mut *tx)
        .await
        .unwrap();
    let synced: Vec<String> =
        sqlx::query_scalar("SELECT id FROM users WHERE synced_at IS NOT NULL ORDER BY 1")
            .fetch_all(&mut *tx)
            .await
            .unwrap();

    assert_eq!(permissions, ["user-10"], "a permission lasts 14 days");
    assert_eq!(synced, ["user-10"]);
    let partitions: Vec<String> = sqlx::query_scalar(
        "SELECT c.relname::text FROM pg_inherits i JOIN pg_class c ON c.oid = i.inhrelid
         WHERE i.inhparent = 'events'::regclass ORDER BY 1",
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap();

    assert_eq!(partitions, ["events_2026_10", "events_2026_11"]);
}

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
            is_ready_for_review: false,
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
        "installation" => ScmEvent::ConnectionChanged,
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
        payload,
        ..delivery(id, 0)
    }
}

type RouteRow = (String, Option<String>, Option<String>, Option<String>);

async fn routes(pool: &PgPool) -> Vec<RouteRow> {
    let mut tx = begin_system(pool).await.unwrap();
    sqlx::query_as(
        "SELECT delivery_id, org_id, connection_id, queue_id
         FROM webhook_inbox ORDER BY delivery_id",
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap()
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_router_routes_each_delivery_to_its_connection_and_sure_queue(pool: PgPool) {
    seed(&pool).await;
    let cancel = CancellationToken::new();
    let router = tokio::spawn(run_router(
        pool.clone(),
        fake_translate,
        || at(5),
        std::time::Duration::from_mins(1),
        cancel.clone(),
    ));

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
            "i-repos",
            "repos",
            json!({ "installation": "1", "repo": "2" }),
        ),
        unrouted("j-repos-unknown", "repos", json!({ "installation": "99" })),
        unrouted(
            "k-installation",
            "installation",
            json!({ "installation": "1" }),
        ),
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
        ),
        (
            "b-push".into(),
            o.clone(),
            c.clone(),
            Some(queue(3).to_string()),
        ),
        ("c-other-branch".into(), o.clone(), c.clone(), None),
        ("d-rules".into(), o.clone(), c.clone(), None),
        ("i-repos".into(), o, c, None),
    ];
    let _ = tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while routes(&pool).await != expected {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await;

    assert_eq!(routes(&pool).await, expected, "routed within a second");
    assert!(queue_state(&pool, queue(2)).await.0);
    assert!(queue_state(&pool, queue(3)).await.0);
    assert_eq!(
        sync_jobs(&pool).await,
        [format!("sync_repos:{}-i-repos:1", connection())]
    );
    assert_eq!(
        installation_jobs(&pool).await,
        [
            "sync_installation:1-k-installation:1",
            "sync_installation:99-e-unknown:1",
            "sync_installation:99-j-repos-unknown:1",
        ]
    );
    cancel.cancel();
    router.await.unwrap().unwrap();
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_repository_of_two_connections_routes_to_the_queue_of_each(pool: PgPool) {
    seed(&pool).await;
    let other_org = OrgId::from_ulid(Ulid(2));
    let other_connection = ConnectionId::from_ulid(Ulid(2));
    let other_queue = queue(6);
    let mut tx = begin_system(&pool).await.unwrap();
    sqlx::raw_sql(AssertSqlSafe(format!(
        "INSERT INTO orgs (id, slug) VALUES ('{other_org}', 'other');
         INSERT INTO scm_connections (id, org_id, account_external_id, kind, external_id,
                                      synced_at, permissions)
             VALUES ('{other_connection}', '{other_org}', '2', 'github_app', '2', now(), '{{}}');
         INSERT INTO repos (id, org_id, connection_id, external_id, owner, name, default_branch)
             VALUES ('r2', '{other_org}', '{other_connection}', '1', 'other', 'app', 'main');
         INSERT INTO queues (id, org_id, repo_id, target_branch)
             VALUES ('{other_queue}', '{other_org}', 'r2', 'main');"
    )))
    .execute(&mut *tx)
    .await
    .unwrap();

    let main_of = |installation: &str| {
        fake_translate(
            "pr",
            &json!({ "installation": installation, "repo": "1", "branch": "main" }),
        )
        .unwrap()
    };
    let first = find_route(&mut tx, &main_of("1")).await.unwrap();
    let second = find_route(&mut tx, &main_of("2")).await.unwrap();

    assert_eq!(first.org_id, Some(org()));
    assert_eq!(first.queue_id, Some(queue(1)));
    assert_eq!(second.org_id, Some(other_org));
    assert_eq!(second.queue_id, Some(other_queue));
}

async fn installation_jobs(pool: &PgPool) -> Vec<String> {
    let mut tx = begin_system(pool).await.unwrap();
    sqlx::query_scalar(
        "SELECT idempotency_key FROM jobs WHERE type IN ('sync_installation', 'sync_installations')
         ORDER BY idempotency_key",
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap()
}

async fn route_at(pool: &PgPool, now: OffsetDateTime, deliveries: &[Delivery]) {
    let cancel = CancellationToken::new();
    let router = tokio::spawn(run_router(
        pool.clone(),
        fake_translate,
        move || now,
        std::time::Duration::from_millis(20),
        cancel.clone(),
    ));

    for delivery in deliveries {
        store(pool, delivery).await;
    }

    for _ in 0..50 {
        if routes(pool).await.iter().all(|route| route.2.is_some()) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    cancel.cancel();
    router.await.unwrap().unwrap();

    assert!(
        routes(pool).await.iter().all(|route| route.2.is_some()),
        "routed"
    );
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_delivery_of_a_connection_read_more_than_an_hour_ago_asks_for_one_sync(pool: PgPool) {
    seed(&pool).await;
    let push = |id: &str| unrouted(id, "push", json!({ "installation": "1", "repo": "1" }));

    route_at(&pool, at(3599), &[push("a")]).await;

    assert_eq!(installation_jobs(&pool).await, [] as [String; 0]);

    route_at(&pool, at(3601), &[push("b"), push("c")]).await;

    assert_eq!(
        installation_jobs(&pool).await,
        ["sync_installation:1-20260910T01:1"]
    );
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

type OrgRow = (String, String, String);

async fn orgs(pool: &PgPool) -> Vec<OrgRow> {
    let mut tx = begin_system(pool).await.unwrap();
    sqlx::query_as(
        "SELECT o.slug, c.account_external_id, c.external_id
         FROM orgs o
         JOIN scm_connections c ON c.org_id = o.id
         ORDER BY o.slug",
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap()
}

type ConnectionRow = (String, OffsetDateTime, Option<OffsetDateTime>);

async fn connection_of(pool: &PgPool, external_id: &str) -> ConnectionRow {
    let mut tx = begin_system(pool).await.unwrap();
    sqlx::query_as(
        "SELECT permissions::text, synced_at, suspended_at FROM scm_connections
         WHERE external_id = $1",
    )
    .bind(external_id)
    .fetch_one(&mut *tx)
    .await
    .unwrap()
}

async fn given_facts(pool: &PgPool) -> Vec<(String, String)> {
    let mut tx = begin_system(pool).await.unwrap();
    sqlx::query_as("SELECT queue_id, type::text FROM queue_inbox ORDER BY type, queue_id")
        .fetch_all(&mut *tx)
        .await
        .unwrap()
}

fn account(external_id: &str, path: &str) -> ScmAccount {
    ScmAccount {
        external_id: external_id.into(),
        path: path.into(),
    }
}

const KIND: ConnectionKind = ConnectionKind::GithubApp;

async fn sync_installation(pool: &PgPool, sim: &Sim, external_id: &str, now: OffsetDateTime) {
    let job = sync_installation_job(external_id, "test", JobClass::Interactive);
    run_sync_installation(pool, sim, KIND, &job, now)
        .await
        .unwrap();
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_full_sync_creates_the_organization_of_a_new_installation_once(pool: PgPool) {
    seed(&pool).await;
    let sim = Sim::new();
    sim.add_installation("1", account("1", "org"), &[("contents", "write")]);
    sim.add_installation("7", account("70", "globex"), &[("metadata", "read")]);
    sim.add_repo(&sim_repo("7", "5", "web"), "main", &[]);

    run_sync_installations(&pool, &sim, KIND, at(0))
        .await
        .unwrap();
    let globex = ("globex".to_owned(), "70".to_owned(), "7".to_owned());
    let acme = ("org".to_owned(), "1".to_owned(), "1".to_owned());

    assert_eq!(orgs(&pool).await, [globex.clone(), acme.clone()]);
    assert_eq!(
        connection_of(&pool, "7").await,
        (r#"{"metadata": "read"}"#.to_owned(), at(0), None)
    );
    assert_eq!(
        connection_of(&pool, "1").await,
        (r#"{"contents": "write"}"#.to_owned(), at(0), None)
    );

    let mut tx = begin_system(&pool).await.unwrap();
    let (org_id, connection_id): (String, String) =
        sqlx::query_as("SELECT org_id, id FROM scm_connections WHERE external_id = '7'")
            .fetch_one(&mut *tx)
            .await
            .unwrap();

    drop(tx);
    let read = format!("sync_repos:{connection_id}-20260910T00:1");
    assert_eq!(sync_jobs(&pool).await, std::slice::from_ref(&read));
    let job = sync_repos_job(
        org_id.parse().unwrap(),
        connection_id.parse().unwrap(),
        "20260910T00",
    );

    assert_eq!(run_sync_repos(&pool, &sim, &job, at(0)).await.unwrap(), 1);
    assert!(repos(&pool).await.iter().any(|repo| repo.0 == "5"));

    run_sync_installations(&pool, &sim, KIND, at(60))
        .await
        .unwrap();

    assert_eq!(orgs(&pool).await, [globex, acme]);
    assert_eq!(sync_jobs(&pool).await, [read]);
    assert_eq!(connection_of(&pool, "7").await.1, at(60));
    assert_eq!(given_facts(&pool).await, [] as [(String, String); 0]);
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_delivery_of_a_new_installation_creates_its_organization(pool: PgPool) {
    seed(&pool).await;
    let sim = Sim::new();
    sim.add_installation("7", account("70", "globex"), &[]);
    sim.add_installation("8", account("80", "initech"), &[]);

    route_at(
        &pool,
        at(0),
        &[
            unrouted("a-created", "installation", json!({ "installation": "7" })),
            unrouted(
                "b-pr",
                "pr",
                json!({ "installation": "8", "repo": "9", "branch": "main" }),
            ),
        ],
    )
    .await;
    let mut tx = begin_system(&pool).await.unwrap();
    let waiting: i64 = sqlx::query_scalar("SELECT count(*) FROM webhook_inbox")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    let jobs = claim_jobs(&mut tx, "w", at(0), 10).await.unwrap();

    tx.commit().await.unwrap();

    assert_eq!(waiting, 0);

    for job in &jobs {
        run_sync_installation(&pool, &sim, KIND, job, at(0))
            .await
            .unwrap();
    }

    let slugs: Vec<String> = orgs(&pool).await.into_iter().map(|org| org.0).collect();
    assert_eq!(slugs, ["globex", "initech", "org"]);
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_suspension_gives_access_lost_to_each_queue_then_access_restored(pool: PgPool) {
    seed(&pool).await;
    let sim = Sim::new();
    sim.add_installation("1", account("1", "org"), &[]);
    let facts = |kind: &str| {
        [queue(1), queue(2), queue(3)]
            .map(|queue| (queue.to_string(), kind.to_owned()))
            .to_vec()
    };

    sim.set_access("1", false);
    sync_installation(&pool, &sim, "1", at(10)).await;
    sync_installation(&pool, &sim, "1", at(20)).await;

    assert_eq!(connection_of(&pool, "1").await.2, Some(at(10)));
    assert_eq!(given_facts(&pool).await, facts("access.lost"));
    for queue in [queue(1), queue(2), queue(3)] {
        assert!(queue_state(&pool, queue).await.0, "{queue} is dirty");
    }

    sim.set_access("1", true);
    sync_installation(&pool, &sim, "1", at(30)).await;

    assert_eq!(connection_of(&pool, "1").await.2, None);
    let mut expected = facts("access.lost");
    expected.extend(facts("access.restored"));

    assert_eq!(given_facts(&pool).await, expected);
}

/// A repository that the installation no longer lists keeps its row with
/// `removed_at`, and its queues lose their access; they get it back when the
/// repository comes back. Only a read that changes the row changes
/// `updated_at`.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_removed_repository_gives_access_lost_to_its_queues_until_it_comes_back(pool: PgPool) {
    seed(&pool).await;
    let sim = Sim::new();
    let job = sync_repos_job(org(), connection(), "w");
    let facts = |kind: &str| {
        [queue(1), queue(2), queue(3)]
            .map(|queue| (queue.to_string(), kind.to_owned()))
            .to_vec()
    };
    let removed_at = || async {
        let mut tx = begin_system(&pool).await.unwrap();
        let at: (Option<OffsetDateTime>, OffsetDateTime) =
            sqlx::query_as("SELECT removed_at, updated_at FROM repos WHERE id = 'r'")
                .fetch_one(&mut *tx)
                .await
                .unwrap();
        at
    };
    sim.add_installation("1", account("1", "org"), &[]);
    sim.add_repo(&sim_repo("1", "1", "app"), "main", &[]);

    run_sync_repos(&pool, &sim, &job, at(0)).await.unwrap();

    assert_eq!(removed_at().await.0, None);
    assert_eq!(given_facts(&pool).await, [] as [(String, String); 0]);

    sim.move_repo("1", "2");
    run_sync_repos(&pool, &sim, &job, at(10)).await.unwrap();
    run_sync_repos(&pool, &sim, &job, at(20)).await.unwrap();

    assert_eq!(removed_at().await, (Some(at(10)), at(10)));
    assert_eq!(given_facts(&pool).await, facts("access.lost"));

    sim.move_repo("1", "1");
    run_sync_repos(&pool, &sim, &job, at(30)).await.unwrap();
    run_sync_repos(&pool, &sim, &job, at(40)).await.unwrap();

    let mut expected = facts("access.lost");
    expected.extend(facts("access.restored"));
    assert_eq!(removed_at().await, (None, at(30)));
    assert_eq!(given_facts(&pool).await, expected);
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_deleted_installation_suspends_its_connection(pool: PgPool) {
    seed(&pool).await;
    let sim = Sim::new();

    sync_installation(&pool, &sim, "1", at(10)).await;

    assert_eq!(connection_of(&pool, "1").await.2, Some(at(10)));
    assert_eq!(given_facts(&pool).await.len(), 3);

    run_sync_installations(&pool, &sim, KIND, at(20))
        .await
        .unwrap();

    assert_eq!(
        connection_of(&pool, "1").await,
        ("{}".to_owned(), at(20), Some(at(10)))
    );
    assert_eq!(given_facts(&pool).await.len(), 3);
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_reinstall_on_the_same_account_reconnects_its_organization(pool: PgPool) {
    seed(&pool).await;
    let sim = Sim::new();
    sim.add_installation("1", account("1", "org"), &[]);
    sync_installation(&pool, &sim, "1", at(10)).await;
    let mut tx = begin_system(&pool).await.unwrap();
    request_repo_syncs(&mut tx, at(10)).await.unwrap();
    tx.commit().await.unwrap();

    sim.remove_installation("1");
    sync_installation(&pool, &sim, "1", at(20)).await;

    assert_eq!(connection_of(&pool, "1").await.2, Some(at(20)));

    sim.add_installation("2", account("1", "org"), &[("contents", "read")]);
    sync_installation(&pool, &sim, "2", at(30)).await;
    sync_installation(&pool, &sim, "1", at(40)).await;

    assert_eq!(
        orgs(&pool).await,
        [("org".to_owned(), "1".to_owned(), "2".to_owned())]
    );
    assert_eq!(
        connection_of(&pool, "2").await,
        (r#"{"contents": "read"}"#.to_owned(), at(30), None)
    );
    assert_eq!(
        sync_jobs(&pool).await,
        [
            format!("sync_repos:{}-20260910T00:1", connection()),
            format!("sync_repos:{}-installation-2:1", connection()),
        ]
    );

    let facts = given_facts(&pool).await;
    let count = |kind| facts.iter().filter(|fact| fact.1 == kind).count();
    assert_eq!(count("access.lost"), 3);
    assert_eq!(count("access.restored"), 3);
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn new_permissions_accepted_are_written_on_the_connection(pool: PgPool) {
    seed(&pool).await;
    let sim = Sim::new();
    sim.add_installation("1", account("1", "org"), &[("contents", "read")]);

    sync_installation(&pool, &sim, "1", at(10)).await;
    sim.set_permissions("1", &[("contents", "write"), ("actions", "write")]);
    sync_installation(&pool, &sim, "1", at(20)).await;

    assert_eq!(
        connection_of(&pool, "1").await,
        (
            r#"{"actions": "write", "contents": "write"}"#.to_owned(),
            at(20),
            None
        )
    );
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_slug_already_taken_fails_the_sync_and_creates_nothing(pool: PgPool) {
    seed(&pool).await;
    let sim = Arc::new(Sim::new());
    sim.add_installation("1", account("1", "org"), &[]);
    sim.add_installation("7", account("70", "org"), &[]);
    let execute = sync_installation_executor(sim.clone(), KIND);

    let job = sync_installation_job("7", "d-1", JobClass::Interactive);
    let outcome = execute(pool.clone(), job, at(0)).await;

    assert!(matches!(outcome, Outcome::Failed(_)), "{outcome:?}");
    assert_eq!(orgs(&pool).await.len(), 1);

    sim.add_installation("8", account("80", "globex"), &[]);
    let synced = run_sync_installations(&pool, sim.as_ref(), KIND, at(0)).await;
    assert!(
        matches!(synced, Err(PlatformError::SlugTaken { .. })),
        "{synced:?}"
    );
    let slugs: Vec<String> = orgs(&pool).await.into_iter().map(|org| org.0).collect();
    assert_eq!(slugs, ["globex", "org"]);
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_read_of_repositories_without_access_asks_for_the_sync_of_the_installation(pool: PgPool) {
    seed(&pool).await;
    let sim = Sim::new();
    sim.add_installation("1", account("1", "org"), &[]);
    sim.set_access("1", false);

    let job = sync_repos_job(org(), connection(), "w");
    let read = run_sync_repos(&pool, &sim, &job, at(0)).await;

    assert!(
        matches!(read, Err(PlatformError::Scm(ScmError::AccessLost(_)))),
        "{read:?}"
    );
    assert_eq!(
        installation_jobs(&pool).await,
        ["sync_installation:1-20260910T00:1"]
    );
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_full_sync_is_requested_once_a_day_and_at_each_start(pool: PgPool) {
    let request = |now, at_start| {
        let pool = pool.clone();
        async move {
            let mut tx = begin_system(&pool).await.unwrap();
            let requested = if at_start {
                request_installations_sync_at_start(&mut tx, now).await
            } else {
                request_installations_sync(&mut tx, now).await
            };
            tx.commit().await.unwrap();
            requested.unwrap()
        }
    };

    assert!(request(at(0), false).await);
    assert!(!request(at(3600), false).await);
    assert!(request(at(0), true).await);
    assert_eq!(
        installation_jobs(&pool).await,
        [
            "sync_installations:20260910:1",
            "sync_installations:20260910T002640:1",
        ]
    );
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_token_of_an_organization_has_every_scope_and_is_audited(pool: PgPool) {
    seed(&pool).await;

    let (org_id, token) = create_token(&pool, "org", at(0)).await.unwrap();
    assert_eq!(org_id, org());
    let grant = authenticate(&pool, &token, at(0)).await.unwrap().unwrap();
    assert_eq!(grant.org_id, org());
    assert_eq!(grant.scopes.len(), 3, "{:?}", grant.scopes);
    assert!(grant.scopes.contains(&ApiScope::QueuesWrite));
    let mut tx = begin_system(&pool).await.unwrap();
    let audited: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE action = 'token.created'")
            .fetch_one(&mut *tx)
            .await
            .unwrap();

    drop(tx);

    assert_eq!(audited, 1);

    let unknown = create_token(&pool, "missing", at(0)).await;
    assert!(
        matches!(unknown, Err(PlatformError::UnknownOrganization(_))),
        "{:?}",
        unknown.err()
    );
}

type RepoRow = (String, String, String, String, String, String);

async fn repos(pool: &PgPool) -> Vec<RepoRow> {
    let mut tx = begin_system(pool).await.unwrap();
    sqlx::query_as(
        "SELECT external_id, org_id, connection_id, owner, name, default_branch
         FROM repos ORDER BY external_id, org_id",
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
        owner: "acme".into(),
        name: name.into(),
    }
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_read_of_the_repositories_creates_and_updates_those_of_its_connection(pool: PgPool) {
    seed(&pool).await;
    let mut tx = begin_system(&pool).await.unwrap();
    sqlx::raw_sql(
        "INSERT INTO orgs (id, slug) VALUES ('other', 'other');
         INSERT INTO scm_connections (id, org_id, account_external_id, kind, external_id,
                                      synced_at, permissions)
             VALUES ('c2', 'other', '2', 'github_app', '2', now(), '{}');
         INSERT INTO repos (id, org_id, connection_id, external_id, owner, name, default_branch)
             VALUES ('r4', 'other', 'c2', '4', 'other', 'moved', 'main');",
    )
    .execute(&mut *tx)
    .await
    .unwrap();

    tx.commit().await.unwrap();
    let sim = Sim::new();
    sim.add_repo(&sim_repo("1", "1", "renamed"), "trunk", &[]);
    sim.add_repo(&sim_repo("1", "2", "lib"), "main", &[]);
    sim.add_repo(&sim_repo("1", "4", "moved"), "main", &[]);
    sim.add_repo(&sim_repo("9", "3", "elsewhere"), "main", &[]);

    let job = sync_repos_job(org(), connection(), "w");
    assert_eq!(run_sync_repos(&pool, &sim, &job, at(0)).await.unwrap(), 3);
    let row = |id: &str, org: &str, connection: &str, owner: &str, name: &str, branch: &str| {
        (
            id.to_owned(),
            org.to_owned(),
            connection.to_owned(),
            owner.to_owned(),
            name.to_owned(),
            branch.to_owned(),
        )
    };
    let (o, c) = (org().to_string(), connection().to_string());
    let expected = [
        row("1", &o, &c, "acme", "renamed", "trunk"),
        row("2", &o, &c, "acme", "lib", "main"),
        row("4", &o, &c, "acme", "moved", "main"),
        row("4", "other", "c2", "other", "moved", "main"),
    ];

    assert_eq!(repos(&pool).await, expected);
    let mut tx = begin_system(&pool).await.unwrap();
    let id: String = sqlx::query_scalar("SELECT id FROM repos WHERE external_id = '1'")
        .fetch_one(&mut *tx)
        .await
        .unwrap();

    tx.commit().await.unwrap();

    assert_eq!(id, "r");

    let mut tx = begin_system(&pool).await.unwrap();
    sqlx::query("UPDATE scm_connections SET suspended_at = now() WHERE id = $1")
        .bind(connection().to_string())
        .execute(&mut *tx)
        .await
        .unwrap();

    tx.commit().await.unwrap();
    let calls = sim.calls().len();
    assert_eq!(run_sync_repos(&pool, &sim, &job, at(0)).await.unwrap(), 0);
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

fn cipher() -> DatabaseCipher {
    DatabaseCipher::new(&SecretString::from("a database secret of 32 bytes ab")).unwrap()
}

fn person_token() -> ScmPersonToken {
    ScmPersonToken {
        access: SecretString::from("token"),
        access_lifetime: Some(Duration::hours(8)),
        refresh: None,
        refresh_lifetime: None,
    }
}

fn person(external_id: &str, login: &str) -> ScmUser {
    ScmUser {
        external_id: external_id.into(),
        login: login.into(),
    }
}

/// The login and the role of each membership, by login.
async fn roles(pool: &PgPool) -> Vec<(String, String)> {
    let mut tx = begin_system(pool).await.unwrap();
    sqlx::query_as(
        "SELECT u.login, m.role::text FROM memberships m JOIN users u ON u.id = m.user_id
         ORDER BY u.login",
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap()
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_sign_in_adds_the_roles_that_the_scm_gives_and_deletes_none(pool: PgPool) {
    seed(&pool).await;
    let globex = OrgId::from_ulid(Ulid(2));
    let membership = |installation: &str, role| ScmMembership {
        connection_external_id: installation.into(),
        role,
    };
    let signed_in = |memberships| SignedIn {
        user: person("5", "ada"),
        profile: ScmProfile::default(),
        installations: Vec::new(),
        memberships,
    };
    let first = signed_in(vec![
        membership("1", MembershipRole::Admin),
        membership("99", MembershipRole::Member),
    ]);
    let second = signed_in(vec![membership("2", MembershipRole::Member)]);
    let in_org = Membership {
        org_id: org(),
        org_slug: "org".into(),
        role: MembershipRole::Admin,
        is_org_suspended: false,
        refreshed_at: at(0),
    };
    let in_globex = Membership {
        org_id: globex,
        org_slug: "globex".into(),
        role: MembershipRole::Member,
        is_org_suspended: false,
        refreshed_at: at(60),
    };
    let mut tx = begin_system(&pool).await.unwrap();

    sqlx::raw_sql(AssertSqlSafe(format!(
        "INSERT INTO orgs (id, slug) VALUES ('{globex}', 'globex');
         INSERT INTO scm_connections (id, org_id, account_external_id, kind, external_id,
                                      synced_at, permissions)
             VALUES ('{c}', '{globex}', '2', 'github_app', '2', '{at}', '{{}}');",
        c = ConnectionId::from_ulid(Ulid(2)),
        at = at(0),
    )))
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let user_id = record_sign_in(
        &pool,
        ConnectionKind::GithubApp,
        &cipher(),
        (&first, &person_token()),
        at(0),
    )
    .await
    .unwrap();

    assert_eq!(
        memberships_of(&pool, user_id).await.unwrap(),
        std::slice::from_ref(&in_org)
    );

    let again = record_sign_in(
        &pool,
        ConnectionKind::GithubApp,
        &cipher(),
        (&second, &person_token()),
        at(60),
    )
    .await
    .unwrap();

    assert_eq!(again, user_id);
    assert_eq!(
        memberships_of(&pool, user_id).await.unwrap(),
        [in_globex, in_org]
    );
}

/// A sign-in connects at once the installations of the person that CIHub does
/// not know yet, as a sync would, and gives the role of the person in them; a
/// suspended installation, or one whose slug is taken, gives nothing and stops
/// nothing.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_sign_in_connects_the_installations_that_cihub_does_not_know(pool: PgPool) {
    seed(&pool).await;
    let installation = |external_id: &str, path: &str, is_suspended| ScmInstallation {
        external_id: external_id.into(),
        account: account(&format!("10{external_id}"), path),
        permissions: BTreeMap::new(),
        is_suspended,
    };
    let membership = |installation: &str| ScmMembership {
        connection_external_id: installation.into(),
        role: MembershipRole::Admin,
    };
    let signed_in = SignedIn {
        user: person("5", "ada"),
        profile: ScmProfile::default(),
        installations: vec![
            installation("1", "org", false),
            installation("7", "acme", false),
            installation("8", "paused", true),
            installation("9", "org", false),
        ],
        memberships: ["1", "7", "8", "9"].map(membership).to_vec(),
    };

    let user_id = record_sign_in(&pool, KIND, &cipher(), (&signed_in, &person_token()), at(0))
        .await
        .unwrap();
    let again = record_sign_in(
        &pool,
        KIND,
        &cipher(),
        (&signed_in, &person_token()),
        at(60),
    )
    .await
    .unwrap();

    let slugs: Vec<String> = memberships_of(&pool, user_id)
        .await
        .unwrap()
        .into_iter()
        .map(|membership| membership.org_slug)
        .collect();
    assert_eq!(again, user_id);
    assert_eq!(slugs, ["acme", "org"], "the slug of `9` is taken");
    let mut tx = begin_system(&pool).await.unwrap();
    let connections: Vec<String> =
        sqlx::query_scalar("SELECT external_id FROM scm_connections ORDER BY 1")
            .fetch_all(&mut *tx)
            .await
            .unwrap();
    let jobs: Vec<String> = sqlx::query_scalar(
        "SELECT j.type FROM jobs j JOIN orgs o ON o.id = j.org_id WHERE o.slug = 'acme'",
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap();
    assert_eq!(connections, ["1", "7"]);
    assert_eq!(
        jobs,
        ["sync_repos"],
        "the new organization reads its repositories"
    );
}

/// The sync of a person reads with their token the repositories that they
/// reach, connects the installations that CIHub does not know, and replaces
/// their permissions; a token that the SCM refuses is forgotten with what it
/// read.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_sync_of_a_person_records_what_their_token_reaches(pool: PgPool) {
    seed(&pool).await;
    let sim = Sim::new();
    let ada = person("5", "ada");
    let repo = |connection: &str, external_id: &str, name: &str| RepoRef {
        connection: ConnectionRef {
            id: connection_ref_id(),
            external_id: connection.into(),
        },
        external_id: external_id.into(),
        owner: "acme".into(),
        name: name.into(),
    };
    sim.add_installation("1", account("1", "org"), &[]);
    sim.add_installation("7", account("70", "acme"), &[]);
    sim.set_member("1", &ada, MembershipRole::Member);
    sim.set_member("7", &ada, MembershipRole::Member);
    let lib = RepoId::from_ulid(Ulid(90));
    let mut tx = begin_system(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO repos (id, org_id, connection_id, external_id, owner, name, default_branch)
         VALUES ($1, $2, $3, '9', 'acme', 'lib', 'main')",
    )
    .bind(lib.to_string())
    .bind(org().to_string())
    .bind(connection().to_string())
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    sim.add_repo(&repo("1", "9", "lib"), "main", &[]);
    sim.set_permission(&repo("1", "9", "lib"), &ada, Permission::Write)
        .unwrap();
    let user_id = signed_in_on(&pool, &sim, &ada, at(0)).await;
    let job = sync_user_job(user_id, "w");

    let read = run_sync_user(&pool, &sim, &cipher(), KIND, &job, at(10))
        .await
        .unwrap();

    let mut tx = begin_system(&pool).await.unwrap();
    let permissions: Vec<(String, String)> =
        sqlx::query_as("SELECT repo_id, permission::text FROM repo_permissions")
            .fetch_all(&mut *tx)
            .await
            .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(read, 1);
    assert_eq!(permissions, [(lib.to_string(), "write".to_owned())]);
    let slugs: Vec<String> = orgs(&pool).await.into_iter().map(|org| org.0).collect();
    assert_eq!(
        slugs,
        ["acme", "org"],
        "the sync connects the installation 7"
    );

    sim.revoke_person(&ada);
    let read = run_sync_user(&pool, &sim, &cipher(), KIND, &job, at(20))
        .await
        .unwrap();

    let mut tx = begin_system(&pool).await.unwrap();
    let left: (i64, bool) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM repo_permissions),
                (SELECT scm_token IS NULL FROM users WHERE login = 'ada')",
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(read, 0);
    assert_eq!(
        left,
        (0, true),
        "a refused token is forgotten with what it read"
    );
}

/// A refusal of the SCM serves as long as a grant: the next request within
/// the hour asks the SCM nothing.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_refused_repository_is_not_asked_again_within_the_hour(pool: PgPool) {
    seed(&pool).await;
    let sim = Sim::new();
    let ada = person("5", "ada");
    sim.add_installation("1", account("1", "org"), &[]);
    sim.set_member("1", &ada, MembershipRole::Member);
    sim.add_repo(&sim_repo("1", "9", "lib"), "main", &[]);
    let lib = RepoId::from_ulid(Ulid(90));
    let mut tx = begin_system(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO repos (id, org_id, connection_id, external_id, owner, name, default_branch)
         VALUES ($1, $2, $3, '9', 'acme', 'lib', 'main')",
    )
    .bind(lib.to_string())
    .bind(org().to_string())
    .bind(connection().to_string())
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let user_id = signed_in_on(&pool, &sim, &ada, at(0)).await;
    let cipher = cipher();
    let read = |now| repo_permission_now(&pool, &sim, &cipher, user_id, org(), "lib", now);

    let first = read(at(10)).await.unwrap();
    let calls = sim.calls().len();
    let second = read(at(20)).await.unwrap();

    assert_eq!(first, Access::Granted(Some((lib, Permission::None))));
    assert_eq!(second, first);
    assert_eq!(sim.calls().len(), calls, "the refusal is kept for an hour");
}

/// A token that ends within minutes is refreshed once, and the refresh
/// token works no more; a refused refresh forgets the token.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_token_that_ends_is_refreshed_once(pool: PgPool) {
    seed(&pool).await;
    let sim = Sim::new();
    let ada = person("5", "ada");
    sim.add_installation("1", account("1", "org"), &[]);
    sim.set_member("1", &ada, MembershipRole::Member);
    let user_id = signed_in_on(&pool, &sim, &ada, at(0)).await;
    let late = at(0) + Duration::hours(8);

    let early = token_of_person(&pool, &sim, &cipher(), user_id, at(60))
        .await
        .unwrap();
    let refreshed = token_of_person(&pool, &sim, &cipher(), user_id, late)
        .await
        .unwrap();
    let kept = token_of_person(&pool, &sim, &cipher(), user_id, late)
        .await
        .unwrap();

    let refreshes = || {
        sim.calls()
            .iter()
            .filter(|call| call.name == "refresh_person_token")
            .count()
    };
    assert_eq!(
        early.as_ref().map(ExposeSecret::expose_secret),
        Some("sim-token-1")
    );
    assert_eq!(
        refreshed.as_ref().map(ExposeSecret::expose_secret),
        Some("sim-token-2")
    );
    assert_eq!(
        kept.as_ref().map(ExposeSecret::expose_secret),
        Some("sim-token-2")
    );
    assert_eq!(refreshes(), 1, "the new token serves until it ends");

    sim.revoke_person(&ada);
    let refused = token_of_person(&pool, &sim, &cipher(), user_id, late + Duration::hours(8))
        .await
        .unwrap();

    assert!(refused.is_none(), "a refused refresh forgets the token");
}

fn connection_ref_id() -> ConnectionId {
    ConnectionId::from_ulid(Ulid(9))
}

/// Signs `person` in on `sim` at `now` and records it, their token
/// included; returns the user.
async fn signed_in_on(pool: &PgPool, sim: &Sim, person: &ScmUser, now: OffsetDateTime) -> UserId {
    let request = SignInRequest {
        redirect_url: "https://ci.example.com/auth/github/callback".into(),
        state: "state".into(),
        code_challenge: code_challenge(&SecretString::from("verifier")),
    };
    let code = SignInCode {
        code: SecretString::from(sim.authorize(person, &request)),
        code_verifier: SecretString::from("verifier"),
        redirect_url: request.redirect_url.clone(),
    };
    let (signed_in, token) = Scm::sign_in(sim, &code, JobClass::Interactive)
        .await
        .unwrap();

    record_sign_in(pool, KIND, &cipher(), (&signed_in, &token), now)
        .await
        .unwrap()
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_hourly_read_of_the_members_follows_the_account(pool: PgPool) {
    seed(&pool).await;
    let sim = Sim::new();
    let (ada, grace) = (person("5", "ada"), person("6", "grace"));
    let job = sync_members_job(org(), connection(), "20260910T00");
    let mut tx = begin_system(&pool).await.unwrap();

    assert_eq!(request_member_syncs(&mut tx, at(0)).await.unwrap(), 1);
    assert_eq!(request_member_syncs(&mut tx, at(60)).await.unwrap(), 0);
    tx.commit().await.unwrap();

    sim.add_installation("1", account("1", "org"), &[]);
    sim.set_member("1", &ada, MembershipRole::Admin);
    sim.set_member("1", &grace, MembershipRole::Member);

    let read = run_sync_members(&pool, &sim, &job, at(0)).await.unwrap();

    assert_eq!(read, 2);
    assert_eq!(
        roles(&pool).await,
        [
            ("ada".to_owned(), "admin".to_owned()),
            ("grace".to_owned(), "member".to_owned()),
        ]
    );

    sim.remove_member("1", &grace);
    let read = run_sync_members(&pool, &sim, &job, at(3600)).await.unwrap();

    assert_eq!(read, 1);
    assert_eq!(roles(&pool).await, [("ada".to_owned(), "admin".to_owned())]);
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_sign_in_gives_the_profile_and_the_members_give_the_avatar_only(pool: PgPool) {
    seed(&pool).await;
    let sim = Sim::new();
    let ada = person("5", "ada");
    let job = sync_members_job(org(), connection(), "20260910T00");
    let signed_in = SignedIn {
        user: ada.clone(),
        profile: ScmProfile {
            name: Some("Ada Lovelace".into()),
            avatar_url: Some("https://avatars.example.com/u/5?v=1".into()),
        },
        installations: Vec::new(),
        memberships: Vec::new(),
    };
    let profile = |avatar_url: &str| UserProfile {
        login: "ada".into(),
        name: Some("Ada Lovelace".into()),
        avatar_url: Some(avatar_url.into()),
        synced_at: None,
    };

    let user_id = record_sign_in(
        &pool,
        ConnectionKind::GithubApp,
        &cipher(),
        (&signed_in, &person_token()),
        at(0),
    )
    .await
    .unwrap();

    assert_eq!(
        profile_of(&pool, user_id).await.unwrap(),
        Some(profile("https://avatars.example.com/u/5?v=1"))
    );

    sim.add_installation("1", account("1", "org"), &[]);
    sim.set_member("1", &ada, MembershipRole::Admin);
    sim.set_profile(
        &ada,
        ScmProfile {
            name: None,
            avatar_url: Some("https://avatars.example.com/u/5?v=2".into()),
        },
    );
    run_sync_members(&pool, &sim, &job, at(60)).await.unwrap();

    assert_eq!(
        profile_of(&pool, user_id).await.unwrap(),
        Some(profile("https://avatars.example.com/u/5?v=2"))
    );
}

/// SEC-7: the account is renamed on the SCM while `orgs.slug` keeps the old
/// login, so the read of the role asks for a membership of a name that no
/// longer exists.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_renamed_account_keeps_the_membership_of_its_members(pool: PgPool) {
    seed(&pool).await;
    let sim = Sim::new();
    let ada = person("5", "ada");

    sim.add_installation("1", account("1", "org"), &[]);
    sim.set_member("1", &ada, MembershipRole::Admin);
    let user_id = signed_in_on(&pool, &sim, &ada, at(0)).await;
    sim.add_installation("1", account("1", "org-renamed"), &[]);
    let membership = memberships_of(&pool, user_id).await.unwrap().remove(0);

    let role = role_now(
        &pool,
        &sim,
        &cipher(),
        user_id,
        &membership,
        at(0) + Duration::hours(2),
    )
    .await
    .unwrap();

    assert!(matches!(role, Access::Granted(Some(MembershipRole::Admin))));
    assert_eq!(memberships_of(&pool, user_id).await.unwrap().len(), 1);
}

/// SEC-7: a refused read of the membership (single sign-on not authorized)
/// does not tell that the person left.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_refused_read_of_the_role_keeps_the_membership(pool: PgPool) {
    seed(&pool).await;
    let sim = Sim::new();
    let ada = person("5", "ada");

    sim.add_installation("1", account("1", "org"), &[]);
    sim.set_member("1", &ada, MembershipRole::Admin);
    let user_id = signed_in_on(&pool, &sim, &ada, at(0)).await;
    let membership = memberships_of(&pool, user_id).await.unwrap().remove(0);
    sim.fail(Fault {
        call: Some("person_role"),
        error: ScmError::Forbidden("single sign-on not authorized".into()),
        times: 1,
    });

    let role = role_now(
        &pool,
        &sim,
        &cipher(),
        user_id,
        &membership,
        at(0) + Duration::hours(2),
    )
    .await;

    assert!(matches!(
        role,
        Err(PlatformError::Scm(ScmError::Forbidden(_)))
    ));
    assert_eq!(memberships_of(&pool, user_id).await.unwrap().len(), 1);
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_lost_access_keeps_the_members_and_syncs_the_installation(pool: PgPool) {
    seed(&pool).await;
    let sim = Sim::new();
    let job = sync_members_job(org(), connection(), "20260910T00");

    sim.add_installation("1", account("1", "org"), &[]);
    sim.set_member("1", &person("5", "ada"), MembershipRole::Admin);
    run_sync_members(&pool, &sim, &job, at(0)).await.unwrap();
    sim.set_access("1", false);

    let lost = run_sync_members(&pool, &sim, &job, at(60)).await;

    assert!(matches!(
        lost,
        Err(PlatformError::Scm(ScmError::AccessLost(_)))
    ));
    assert_eq!(roles(&pool).await.len(), 1);

    let mut tx = begin_system(&pool).await.unwrap();
    let syncs: i64 =
        sqlx::query_scalar("SELECT count(*) FROM jobs WHERE type = 'sync_installation'")
            .fetch_one(&mut *tx)
            .await
            .unwrap();

    assert_eq!(syncs, 1);
}

fn queue_event(n: u128, kind: EventKind) -> Event {
    Event {
        id: EventId::from_ulid(Ulid(n)),
        org_id: org(),
        queue_id: queue(1),
        actor: Actor::System,
        occurred_at: at(i64::try_from(n).unwrap()),
        cause_id: None,
        kind,
    }
}

async fn cursor(pool: &PgPool) -> Option<i64> {
    let mut tx = begin_system(pool).await.unwrap();
    sqlx::query_scalar("SELECT seq FROM log_cursors WHERE queue_id = $1 AND subscription = 'test'")
        .bind(queue(1).to_string())
        .fetch_optional(&mut *tx)
        .await
        .unwrap()
}

/// The cursor of a subscription advances in the transaction of its effect: a
/// subscription that fails reads the same events again at its next pass.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_subscription_that_fails_reads_its_events_again(pool: PgPool) {
    seed(&pool).await;
    create_event_partitions(&pool, at(0).date()).await.unwrap();
    let mut tx = begin_system(&pool).await.unwrap();
    let events = [
        queue_event(1, EventKind::AccessLost),
        queue_event(2, EventKind::AccessRestored),
    ];
    append_events(&mut tx, &events).await.unwrap();
    tx.commit().await.unwrap();

    let seen = Arc::new(Mutex::new(Vec::new()));
    let fails = Arc::new(AtomicBool::new(true));
    let (handler_seen, handler_fails) = (seen.clone(), fails.clone());
    let subscription = Subscription {
        name: "test",
        handle: Box::new(move |_, events| {
            handler_seen
                .lock()
                .unwrap()
                .extend(events.iter().map(|(seq, _)| *seq));
            let fails = handler_fails.load(Ordering::SeqCst);
            Box::pin(async move {
                if fails {
                    return Err("the effect failed".into());
                }
                Ok(())
            })
        }),
    };

    let failed = deliver_all(&pool, &subscription).await;

    assert!(matches!(failed, Err(PlatformError::Subscription { .. })));
    assert_eq!(cursor(&pool).await, None);

    fails.store(false, Ordering::SeqCst);
    deliver_all(&pool, &subscription).await.unwrap();
    deliver_all(&pool, &subscription).await.unwrap();

    assert_eq!(*seen.lock().unwrap(), [1, 2, 1, 2]);
    assert_eq!(cursor(&pool).await, Some(2));
}

//! Tests against Postgres: each `#[sqlx::test]` gets a new database.

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

use cihub_domain::{ConnectionId, OrgId, QueueId, ScmInstanceId};
use cihub_platform::{
    Delivery, LEASE_TTL, QueueWakeup, Wake, acquire_leases, begin_reconciliation, lock_queue,
    mark_processed, pending_deliveries, release_leases, schedule_tick, store_delivery,
};
use cihub_store::{begin_org, begin_system};
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
         INSERT INTO queues (id, org_id, repo_id, target_branch, dirty, next_tick_at) VALUES
             ('{q1}', '{org}', 'r', 'main', true, NULL),
             ('{q2}', '{org}', 'r', 'release', false, '{}'),
             ('{q3}', '{org}', 'r', 'next', false, NULL);",
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

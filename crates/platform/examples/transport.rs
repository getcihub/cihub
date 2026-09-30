//! Plays the internal transport of lot 3 on a real Postgres and prints what
//! happens: queue leases, the lock of a queue, webhook deliveries and the
//! wake-up of the schedulers, a write during a reconciliation, jobs taken back
//! from a stopped worker, retries and dead letters, the quota of a connection,
//! and the worker loop with the daily maintenance.
//!
//! The example creates its own database, `cihub_transport_demo`, on the server
//! of `DATABASE_URL`, applies the migrations, and drops it at the end: it never
//! touches the development database. The times of the leases and the jobs come
//! from a simulated clock, so "+60 s" does not wait; the latency of a
//! notification is real.
//!
//! After each scenario, the example prints the rows of the tables it changed.
//! With `--keep`, it keeps the database at the end, to explore it with `psql`.
//!
//! ```sh
//! mise run db
//! cargo run -p cihub-platform --example transport
//! cargo run -p cihub-platform --example transport -- --keep
//! ```

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a command-line example writes its result to the terminal"
)]

use std::error::Error;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration as StdDuration, Instant};

use cihub_domain::{ConnectionId, JobClass, OrgId, QueueId, ScmInstanceId};
use cihub_platform::{
    Delivery, Executors, JOB_LOCK, JOBS_PER_CONNECTION, Job, LEASE_TTL, MAINTENANCE, Outcome,
    QueueWakeup, Retry, acquire_leases, backoff, begin_reconciliation, claim_jobs, complete_job,
    enqueue_job, execute_maintenance, fail_job, lock_queue, mark_processed, pending_deliveries,
    release_leases, request_maintenance, run_worker, store_delivery,
};
use cihub_store::{MIGRATOR, begin_system, migrate};
use serde_json::json;
use sqlx::postgres::PgConnectOptions;
use sqlx::{Column, PgPool, Row};
use time::macros::datetime;
use time::{Duration, OffsetDateTime};
use tokio_util::sync::CancellationToken;
use ulid::Ulid;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Also written in the `CREATE DATABASE` and `DROP DATABASE` statements.
const DEMO_DATABASE: &str = "cihub_transport_demo";

/// The simulated clock starts here.
const T0: OffsetDateTime = datetime!(2026-10-01 09:00 UTC);

fn org() -> OrgId {
    OrgId::from_ulid(Ulid(1))
}

fn instance() -> ScmInstanceId {
    ScmInstanceId::from_ulid(Ulid(1))
}

fn connection() -> ConnectionId {
    ConnectionId::from_ulid(Ulid(1))
}

fn queue(n: u128) -> QueueId {
    QueueId::from_ulid(Ulid(n))
}

/// A short name for a queue in the output.
fn name(queue: QueueId) -> String {
    match queue.as_ulid().0 {
        1 => "main".to_owned(),
        2 => "release".to_owned(),
        n => format!("queue {n}"),
    }
}

fn names(queues: &[QueueId]) -> String {
    let names: Vec<String> = queues.iter().copied().map(name).collect();
    format!("[{}]", names.join(", "))
}

fn after(seconds: i64) -> OffsetDateTime {
    T0 + Duration::seconds(seconds)
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("error: {}", cihub_platform::error_chain(err.as_ref()));
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let url = std::env::var("DATABASE_URL").map_err(
        |_| "DATABASE_URL is not set: run the example through mise, after `mise run db`",
    )?;
    let server = PgPool::connect(&url).await?;
    drop_demo_database(&server).await?;
    sqlx::query("CREATE DATABASE cihub_transport_demo")
        .execute(&server)
        .await?;
    let options = PgConnectOptions::from_str(&url)?.database(DEMO_DATABASE);
    let pool = PgPool::connect_with(options).await?;
    let result = play(&pool).await;
    pool.close().await;
    if std::env::args().any(|arg| arg == "--keep") {
        println!("\ndatabase {DEMO_DATABASE} kept; to explore it:");
        println!("  docker compose exec postgres psql -U cihub {DEMO_DATABASE}");
        println!(
            "  SELECT set_config('cihub.system', 'on', false);  -- row level security: see every organization"
        );
        println!("  SELECT * FROM jobs;");
    } else {
        drop_demo_database(&server).await?;
        println!("\ndatabase {DEMO_DATABASE} dropped (keep it with --keep)");
    }
    result
}

async fn drop_demo_database(server: &PgPool) -> Result<()> {
    sqlx::query("DROP DATABASE IF EXISTS cihub_transport_demo WITH (FORCE)")
        .execute(server)
        .await?;
    Ok(())
}

async fn play(pool: &PgPool) -> Result<()> {
    migrate(&MIGRATOR, pool).await?;
    seed(pool).await?;
    println!(
        "database {DEMO_DATABASE}: migrated, one organization, queues main (dirty) and release (tick due)"
    );
    leases(pool).await?;
    lock(pool).await?;
    deliveries(pool).await?;
    write_during_reconciliation(pool).await?;
    stopped_worker(pool).await?;
    retries(pool).await?;
    quota(pool).await?;
    worker_loop(pool).await?;
    Ok(())
}

/// Prints the rows of `sql`, whose columns are all `text`, as a table.
async fn show(pool: &PgPool, table: &str, sql: &'static str) -> Result<()> {
    let mut tx = begin_system(pool).await?;
    let rows = sqlx::query(sql).fetch_all(&mut *tx).await?;
    tx.rollback().await?;
    println!("\n  table {table}");
    let Some(first) = rows.first() else {
        println!("    (no rows)");
        return Ok(());
    };
    let header: Vec<String> = first
        .columns()
        .iter()
        .map(|c| c.name().to_owned())
        .collect();
    let mut lines = vec![header];
    for row in &rows {
        let cells = (0..row.len())
            .map(|i| {
                row.try_get::<Option<String>, _>(i)
                    .map(Option::unwrap_or_default)
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;
        lines.push(cells);
    }
    let widths: Vec<usize> = (0..lines[0].len())
        .map(|i| {
            lines
                .iter()
                .map(|line| line[i].chars().count())
                .max()
                .unwrap_or(0)
        })
        .collect();
    for (n, line) in lines.iter().enumerate() {
        let cells: Vec<String> = line
            .iter()
            .zip(&widths)
            .map(|(cell, width)| format!("{cell:<width$}"))
            .collect();
        println!("    {}", cells.join(" | ").trim_end());
        if n == 0 {
            let rule: Vec<String> = widths.iter().map(|width| "-".repeat(*width)).collect();
            println!("    {}", rule.join("-+-"));
        }
    }
    Ok(())
}

async fn seed(pool: &PgPool) -> Result<()> {
    let (org, i, c, main, release) = (org(), instance(), connection(), queue(1), queue(2));
    let mut tx = begin_system(pool).await?;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "INSERT INTO scm_instances (id, kind, base_url) VALUES ('{i}', 'github', 'https://github.com');
         INSERT INTO orgs (id, slug) VALUES ('{org}', 'acme');
         INSERT INTO scm_accounts (id, org_id, scm_instance_id, external_id, path)
             VALUES ('account', '{org}', '{i}', '1', 'acme');
         INSERT INTO scm_connections (id, org_id, scm_instance_id, scm_account_id, kind, external_id)
             VALUES ('{c}', '{org}', '{i}', 'account', 'github_app', '1');
         INSERT INTO repos (id, org_id, scm_instance_id, connection_id, external_id, namespace, name,
                            default_branch)
             VALUES ('repo', '{org}', '{i}', '{c}', '1', 'acme', 'app', 'main');
         INSERT INTO queues (id, org_id, repo_id, target_branch, dirty, next_tick_at) VALUES
             ('{main}', '{org}', 'repo', 'main', true, NULL),
             ('{release}', '{org}', 'repo', 'release', false, '{T0}');"
    )))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

async fn acquire(pool: &PgPool, owner: &str, now: OffsetDateTime) -> Result<Vec<QueueId>> {
    let mut tx = begin_system(pool).await?;
    let queues = acquire_leases(&mut tx, owner, now, 10).await?;
    tx.commit().await?;
    Ok(queues)
}

async fn leases(pool: &PgPool) -> Result<()> {
    println!(
        "\n== queue leases (a lease lasts {} s)",
        LEASE_TTL.whole_seconds()
    );
    let a = acquire(pool, "scheduler-a", after(0)).await?;
    println!("  +0 s   scheduler-a takes {}", names(&a));
    let b = acquire(pool, "scheduler-b", after(1)).await?;
    println!(
        "  +1 s   scheduler-b takes {}   leases held by scheduler-a",
        names(&b)
    );
    let a = acquire(pool, "scheduler-a", after(10)).await?;
    println!("  +10 s  scheduler-a renews {}", names(&a));
    let b = acquire(pool, "scheduler-b", after(24)).await?;
    println!(
        "  +24 s  scheduler-b takes {}   renewed at +10 s, still valid",
        names(&b)
    );
    let b = acquire(pool, "scheduler-b", after(25)).await?;
    println!(
        "  +25 s  scheduler-b takes {}   scheduler-a stopped renewing",
        names(&b)
    );
    show(
        pool,
        "queue_leases",
        "SELECT q.target_branch AS queue, l.owner,
                to_char(l.expires_at AT TIME ZONE 'UTC', 'HH24:MI:SS') AS expires_at
         FROM queue_leases l JOIN queues q ON q.id = l.queue_id ORDER BY 1",
    )
    .await?;
    let mut tx = begin_system(pool).await?;
    let released = release_leases(&mut tx, "scheduler-b").await?;
    tx.commit().await?;
    println!("  scheduler-b stops and gives back {released} leases");
    Ok(())
}

async fn lock(pool: &PgPool) -> Result<()> {
    println!("\n== lock of a queue: one reconciliation at a time");
    let mut first = begin_system(pool).await?;
    let mut second = begin_system(pool).await?;
    println!(
        "  transaction 1 locks main: {}",
        lock_queue(&mut first, queue(1)).await?
    );
    println!(
        "  transaction 2 locks main: {}",
        lock_queue(&mut second, queue(1)).await?
    );
    first.commit().await?;
    println!("  transaction 1 commits");
    println!(
        "  transaction 2 locks main: {}",
        lock_queue(&mut second, queue(1)).await?
    );
    second.rollback().await?;
    Ok(())
}

fn delivery(id: &str, queue_id: QueueId) -> Delivery {
    Delivery {
        scm_instance_id: instance(),
        delivery_id: id.to_owned(),
        org_id: Some(org()),
        connection_id: Some(connection()),
        queue_id: Some(queue_id),
        event: "pull_request".to_owned(),
        action: Some("opened".to_owned()),
        payload: json!({ "number": 12 }),
        received_at: T0,
    }
}

async fn store(pool: &PgPool, delivery: &Delivery) -> Result<bool> {
    let mut tx = begin_system(pool).await?;
    let stored = store_delivery(&mut tx, delivery).await?;
    tx.commit().await?;
    Ok(stored)
}

async fn deliveries(pool: &PgPool) -> Result<()> {
    println!("\n== deliveries and wake-up");
    let cancel = CancellationToken::new();
    let mut wakeup = QueueWakeup::listen(pool).await?;
    let d1 = delivery("d1", queue(2));
    #[expect(
        clippy::disallowed_methods,
        reason = "the example measures a real latency"
    )]
    let sent = Instant::now();
    store(pool, &d1).await?;
    let woken = wakeup.wait(StdDuration::from_secs(5), &cancel).await?;
    println!(
        "  d1 stored for release: queue dirty, scheduler woken ({woken:?}) after {} ms",
        sent.elapsed().as_millis()
    );
    let stored = store(pool, &d1).await?;
    let woken = wakeup.wait(StdDuration::from_millis(300), &cancel).await?;
    println!("  d1 again: stored = {stored}, scheduler: {woken:?} (no new work, no notification)");
    let mut tx = begin_system(pool).await?;
    let pending: Vec<String> = pending_deliveries(&mut tx, queue(2), 10)
        .await?
        .into_iter()
        .map(|d| d.delivery_id)
        .collect();
    println!("  pending for release: {pending:?}");
    mark_processed(&mut tx, instance(), "d1", after(1), None).await?;
    let left = pending_deliveries(&mut tx, queue(2), 10).await?.len();
    tx.commit().await?;
    println!("  d1 processed: {left} pending");
    show(
        pool,
        "webhook_inbox",
        "SELECT i.delivery_id, q.target_branch AS queue, i.event, i.action, i.payload::text,
                to_char(i.received_at AT TIME ZONE 'UTC', 'HH24:MI:SS') AS received_at,
                to_char(i.processed_at AT TIME ZONE 'UTC', 'HH24:MI:SS') AS processed_at
         FROM webhook_inbox i JOIN queues q ON q.id = i.queue_id ORDER BY 1",
    )
    .await?;
    show(
        pool,
        "queues",
        "SELECT target_branch, dirty::text,
                to_char(next_tick_at AT TIME ZONE 'UTC', 'HH24:MI:SS') AS next_tick_at
         FROM queues ORDER BY 1",
    )
    .await?;
    Ok(())
}

async fn dirty(pool: &PgPool, queue: QueueId) -> Result<bool> {
    let mut tx = begin_system(pool).await?;
    let dirty = sqlx::query_scalar("SELECT dirty FROM queues WHERE id = $1")
        .bind(queue.to_string())
        .fetch_one(&mut *tx)
        .await?;
    Ok(dirty)
}

async fn write_during_reconciliation(pool: &PgPool) -> Result<()> {
    println!("\n== a delivery during a reconciliation is not lost");
    let mut tx = begin_reconciliation(pool, queue(1))
        .await?
        .ok_or("main is locked")?;
    let seen: bool = sqlx::query_scalar("SELECT dirty FROM queues WHERE id = $1")
        .bind(queue(1).to_string())
        .fetch_one(&mut *tx)
        .await?;
    println!("  reconciliation of main starts: dirty = {seen} in its transaction");
    let writer = tokio::spawn({
        let pool = pool.clone();
        async move {
            store(&pool, &delivery("d2", queue(1)))
                .await
                .map_err(|e| e.to_string())
        }
    });
    tokio::time::sleep(StdDuration::from_millis(200)).await;
    println!(
        "  d2 arrives for main: still waiting after 200 ms = {}",
        !writer.is_finished()
    );
    tx.commit().await?;
    writer.await??;
    println!(
        "  reconciliation commits: d2 stored, dirty = {}",
        dirty(pool, queue(1)).await?
    );
    show(
        pool,
        "webhook_inbox",
        "SELECT i.delivery_id, q.target_branch AS queue, (i.processed_at IS NOT NULL)::text AS processed
         FROM webhook_inbox i JOIN queues q ON q.id = i.queue_id ORDER BY 1",
    )
    .await?;
    Ok(())
}

fn job(key: &str, job_type: &str) -> Job {
    Job {
        idempotency_key: key.to_owned(),
        job_type: job_type.to_owned(),
        class: JobClass::Merge,
        org_id: Some(org()),
        connection: Some((instance(), connection())),
        queue_id: Some(queue(1)),
        payload: json!({}),
        attempts: 0,
    }
}

async fn enqueue(pool: &PgPool, job: &Job) -> Result<bool> {
    let mut tx = begin_system(pool).await?;
    let enqueued = enqueue_job(&mut tx, job, T0, T0).await?;
    tx.commit().await?;
    Ok(enqueued)
}

async fn claim(pool: &PgPool, worker: &str, now: OffsetDateTime, limit: i64) -> Result<Vec<Job>> {
    let mut tx = begin_system(pool).await?;
    let jobs = claim_jobs(&mut tx, worker, now, limit).await?;
    tx.commit().await?;
    Ok(jobs)
}

fn keys(jobs: &[Job]) -> String {
    let keys: Vec<String> = jobs
        .iter()
        .map(|job| format!("{} (attempt {})", job.idempotency_key, job.attempts))
        .collect();
    format!("[{}]", keys.join(", "))
}

async fn stopped_worker(pool: &PgPool) -> Result<()> {
    println!(
        "\n== a worker stops during a job (a worker holds a job {} s)",
        JOB_LOCK.whole_seconds()
    );
    let key = "merge_pr:item_1:1";
    println!(
        "  request {key}: {}",
        enqueue(pool, &job(key, "merge_pr")).await?
    );
    println!(
        "  request {key} again: {}",
        enqueue(pool, &job(key, "merge_pr")).await?
    );
    println!(
        "  +0 s   worker-a takes {}",
        keys(&claim(pool, "worker-a", after(0), 10).await?)
    );
    println!("         worker-a stops without an answer");
    println!(
        "  +30 s  worker-b takes {}",
        keys(&claim(pool, "worker-b", after(30), 10).await?)
    );
    println!(
        "  +60 s  worker-b takes {}",
        keys(&claim(pool, "worker-b", after(60), 10).await?)
    );
    let mut tx = begin_system(pool).await?;
    let late = complete_job(&mut tx, key, "worker-a", after(61)).await?;
    let done = complete_job(&mut tx, key, "worker-b", after(62)).await?;
    tx.commit().await?;
    println!("  late answer of worker-a accepted: {late}");
    println!("  worker-b finishes: {done}");
    println!(
        "  +1 h   worker-c takes {}",
        keys(&claim(pool, "worker-c", after(3600), 10).await?)
    );
    show(
        pool,
        "jobs",
        "SELECT idempotency_key, type, class::text, state::text, attempts::text, locked_by,
                to_char(locked_until AT TIME ZONE 'UTC', 'HH24:MI:SS') AS locked_until,
                to_char(run_after AT TIME ZONE 'UTC', 'HH24:MI:SS') AS run_after,
                to_char(finished_at AT TIME ZONE 'UTC', 'HH24:MI:SS') AS finished_at, last_error
         FROM jobs
         WHERE idempotency_key = 'merge_pr:item_1:1'",
    )
    .await?;
    Ok(())
}

async fn retries(pool: &PgPool) -> Result<()> {
    println!("\n== a job that keeps failing");
    let key = "post_status:item_2:1";
    enqueue(pool, &job(key, "post_status")).await?;
    let mut now = T0;
    let mut waits = Vec::new();
    let mut attempts = 0;
    loop {
        let claimed = claim(pool, "worker-a", now, 1).await?;
        let Some(claimed) = claimed.first() else {
            break;
        };
        attempts = claimed.attempts;
        let mut tx = begin_system(pool).await?;
        let state = fail_job(
            &mut tx,
            key,
            "worker-a",
            now,
            "503 from the SCM",
            Retry::Later,
        )
        .await?;
        tx.commit().await?;
        if state == Some(cihub_domain::JobState::Dead) {
            println!("  attempt {attempts} fails: dead, waits for an operator");
            break;
        }
        waits.push(backoff(attempts).whole_seconds());
        now += backoff(attempts);
    }
    println!("  waits between the {attempts} attempts (s): {waits:?}");
    let key = "merge_pr:item_3:1";
    enqueue(pool, &job(key, "merge_pr")).await?;
    claim(pool, "worker-a", now, 1).await?;
    let mut tx = begin_system(pool).await?;
    let state = fail_job(
        &mut tx,
        key,
        "worker-a",
        now,
        "the PR is closed",
        Retry::Never,
    )
    .await?;
    tx.commit().await?;
    let state = state.map_or("not held", |state| state.as_str());
    println!("  {key}: an error that a retry cannot fix -> {state}");
    show(
        pool,
        "jobs",
        "SELECT idempotency_key, type, class::text, state::text, attempts::text, locked_by,
                to_char(locked_until AT TIME ZONE 'UTC', 'HH24:MI:SS') AS locked_until,
                to_char(run_after AT TIME ZONE 'UTC', 'HH24:MI:SS') AS run_after,
                to_char(finished_at AT TIME ZONE 'UTC', 'HH24:MI:SS') AS finished_at, last_error
         FROM jobs
         WHERE idempotency_key IN ('post_status:item_2:1', 'merge_pr:item_3:1') ORDER BY 1",
    )
    .await?;
    Ok(())
}

async fn quota(pool: &PgPool) -> Result<()> {
    println!("\n== quota of a connection ({JOBS_PER_CONNECTION} jobs in progress)");
    let quota = usize::try_from(JOBS_PER_CONNECTION)?;
    for n in 0..=quota {
        enqueue(pool, &job(&format!("speculate:item_{n}:1"), "speculate")).await?;
    }
    let first = claim(pool, "worker-a", after(10_000), 1_000).await?;
    let second = claim(pool, "worker-b", after(10_001), 1_000).await?;
    println!("  {} jobs requested", quota + 1);
    println!(
        "  worker-a takes {}, worker-b takes {}: the last one waits for a free place",
        first.len(),
        second.len()
    );
    show(
        pool,
        "jobs (speculate, counted by state)",
        "SELECT state::text, locked_by, count(*)::text AS jobs
         FROM jobs WHERE type = 'speculate' GROUP BY 1, 2 ORDER BY 1",
    )
    .await?;
    let mut tx = begin_system(pool).await?;
    sqlx::query("DELETE FROM jobs WHERE type = 'speculate'")
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    println!("  the example deletes these jobs, so that the next scenario starts clean");
    Ok(())
}

async fn worker_loop(pool: &PgPool) -> Result<()> {
    println!("\n== worker loop and daily maintenance");
    let mut tx = begin_system(pool).await?;
    let first = request_maintenance(&mut tx, after(20_000)).await?;
    let again = request_maintenance(&mut tx, after(23_600)).await?;
    tx.commit().await?;
    println!("  scheduler-a requests the maintenance of the day: {first}");
    println!("  scheduler-b requests it one hour later: {again}");
    enqueue(pool, &job("hello:demo:1", "hello")).await?;

    let mut executors = Executors::default();
    executors.register(MAINTENANCE, execute_maintenance);
    executors.register("hello", |_, _, _| async { Outcome::Done });
    let cancel = CancellationToken::new();
    let worker = tokio::spawn(run_worker(
        pool.clone(),
        "worker-loop".to_owned(),
        Arc::new(executors),
        || after(20_000),
        StdDuration::from_millis(100),
        cancel.clone(),
    ));
    tokio::time::sleep(StdDuration::from_millis(500)).await;
    cancel.cancel();
    worker.await??;

    let mut tx = begin_system(pool).await?;
    let done: Vec<String> = sqlx::query_scalar(
        "SELECT idempotency_key FROM jobs WHERE locked_by IS NULL AND state = 'done'
           AND idempotency_key LIKE ANY (ARRAY['maintenance:%', 'hello:%']) ORDER BY 1",
    )
    .fetch_all(&mut *tx)
    .await?;
    let partitions: Vec<String> = sqlx::query_scalar(
        "SELECT c.relname::text FROM pg_inherits i JOIN pg_class c ON c.oid = i.inhrelid
         WHERE i.inhparent = 'events'::regclass ORDER BY 1",
    )
    .fetch_all(&mut *tx)
    .await?;
    println!("  the worker loop runs {done:?}");
    println!("  partitions of events after the maintenance: {partitions:?}");
    show(
        pool,
        "jobs",
        "SELECT idempotency_key, type, class::text, state::text, attempts::text, locked_by,
                to_char(locked_until AT TIME ZONE 'UTC', 'HH24:MI:SS') AS locked_until,
                to_char(run_after AT TIME ZONE 'UTC', 'HH24:MI:SS') AS run_after,
                to_char(finished_at AT TIME ZONE 'UTC', 'HH24:MI:SS') AS finished_at, last_error
         FROM jobs
         WHERE type IN ('maintenance', 'hello') ORDER BY 1",
    )
    .await?;
    Ok(())
}

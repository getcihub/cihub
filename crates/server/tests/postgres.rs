//! Starts the `cihub-server` binary against Postgres: each `#[sqlx::test]`
//! gets a new database.

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

use ring::hmac;
use sqlx::PgPool;

/// The URL of the database of `pool`, on the server of `DATABASE_URL`.
fn database_url(pool: &PgPool) -> String {
    let server = std::env::var("DATABASE_URL").unwrap();
    let (server, _) = server.rsplit_once('/').unwrap();
    let database = pool.connect_options().get_database().unwrap().to_owned();
    format!("{server}/{database}")
}

fn free_addr() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().to_string()
}

fn command(pool: &PgPool) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cihub-server"));
    command
        .env_clear()
        .env("CIHUB_DATABASE_URL", database_url(pool))
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

const TEST_KEY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../github/tests/test-only-key.pem"
);

/// `command` with a GitHub App: the key of the tests, and an address of
/// GitHub where nothing listens, so that no test reaches the real GitHub.
fn with_github(pool: &PgPool) -> Command {
    let mut command = command(pool);
    command
        .env("CIHUB_GITHUB_URL", format!("http://{}", free_addr()))
        .env("CIHUB_GITHUB_APP_ID", "1")
        .env("CIHUB_GITHUB_PRIVATE_KEY_FILE", TEST_KEY)
        .env("CIHUB_GITHUB_WEBHOOK_SECRET", "secret");
    command
}

fn start(pool: &PgPool, addr: &str, roles: Option<&str>) -> Child {
    let mut command = with_github(pool);
    command
        .env("CIHUB_INTERNAL_ADDR", addr)
        .env("CIHUB_HTTP_ADDR", free_addr());
    if let Some(roles) = roles {
        command.env("CIHUB_ROLES", roles);
    }
    command.spawn().unwrap()
}

/// Sends `GET path` and returns the status line and the body.
fn get(addr: &str, path: &str) -> Option<(String, String)> {
    let mut stream = TcpStream::connect(addr).ok()?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    )
    .ok()?;
    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    let (head, body) = response.split_once("\r\n\r\n")?;
    Some((head.lines().next()?.to_owned(), body.to_owned()))
}

/// Sends `POST path` with `headers` and `body`, and returns the status line.
fn post(addr: &str, path: &str, headers: &[(&str, &str)], body: &str) -> Option<String> {
    let mut stream = TcpStream::connect(addr).ok()?;
    write!(
        stream,
        "POST {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    )
    .ok()?;
    for (name, value) in headers {
        write!(stream, "{name}: {value}\r\n").ok()?;
    }
    write!(stream, "\r\n{body}").ok()?;
    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    Some(response.lines().next()?.to_owned())
}

fn wait_for(addr: &str, path: &str) -> (String, String) {
    for _ in 0..100 {
        if let Some(response) = get(addr, path) {
            return response;
        }
        sleep(Duration::from_millis(50));
    }
    panic!("{addr}{path} never answered");
}

fn stop(mut child: Child) {
    let status = Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .unwrap();
    assert!(status.success());
    let status = child.wait().unwrap();
    assert!(status.success(), "{status}");
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
fn all_in_one_answers_and_stops(pool: PgPool) {
    let addr = free_addr();
    let child = start(&pool, &addr, None);
    for path in ["/healthz", "/readyz"] {
        let (status, body) = wait_for(&addr, path);
        assert_eq!(status, "HTTP/1.1 200 OK");
        assert!(
            body.contains(r#""roles":["web","scheduler","worker","notifier"]"#),
            "{body}"
        );
        assert!(body.contains(r#""name":"cihub-server""#), "{body}");
    }
    stop(child);
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
fn one_role_per_process(pool: PgPool) {
    let addr = free_addr();
    let child = start(&pool, &addr, Some("worker"));
    let (status, body) = wait_for(&addr, "/readyz");
    assert_eq!(status, "HTTP/1.1 200 OK");
    assert!(body.contains(r#""roles":["worker"]"#), "{body}");
    stop(child);
}

#[sqlx::test(migrations = false)]
fn migrate_twice_succeeds(pool: PgPool) {
    for _ in 0..2 {
        let status = command(&pool).arg("migrate").status().unwrap();
        assert!(status.success(), "{status}");
    }
}

#[sqlx::test(migrations = false)]
fn a_missing_migration_stops_the_start(pool: PgPool) {
    let status = with_github(&pool).status().unwrap();
    assert!(!status.success(), "{status}");
    assert!(command(&pool).arg("migrate").status().unwrap().success());
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn all_in_one_runs_the_maintenance_of_the_day(pool: PgPool) {
    let addr = free_addr();
    let child = start(&pool, &addr, None);
    let mut done: Vec<String> = Vec::new();
    for _ in 0..100 {
        let mut tx = cihub_store::begin_system(&pool).await.unwrap();
        done = sqlx::query_scalar(
            "SELECT idempotency_key FROM jobs WHERE type = 'maintenance' AND state = 'done'",
        )
        .fetch_all(&mut *tx)
        .await
        .unwrap();
        tx.rollback().await.unwrap();
        if !done.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    stop(child);
    assert_eq!(done.len(), 1, "{done:?}");
    assert!(done[0].starts_with("maintenance:"), "{done:?}");
    let partitions: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pg_inherits WHERE inhparent = 'events'::regclass")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(partitions, 2);
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn with_a_github_app_the_web_role_stores_a_signed_webhook(pool: PgPool) {
    let (internal, public) = (free_addr(), free_addr());
    let child = with_github(&pool)
        .env("CIHUB_INTERNAL_ADDR", &internal)
        .env("CIHUB_HTTP_ADDR", &public)
        .env("CIHUB_ROLES", "web")
        .spawn()
        .unwrap();
    wait_for(&internal, "/readyz");
    let body = r#"{"zen":"Keep it logically awesome."}"#;
    let tag = hmac::sign(
        &hmac::Key::new(hmac::HMAC_SHA256, b"secret"),
        body.as_bytes(),
    );
    let signature = tag
        .as_ref()
        .iter()
        .fold(String::from("sha256="), |mut hex, b| {
            let _ = std::fmt::Write::write_fmt(&mut hex, format_args!("{b:02x}"));
            hex
        });
    let headers = [
        ("X-GitHub-Delivery", "d1"),
        ("X-GitHub-Event", "ping"),
        ("X-Hub-Signature-256", signature.as_str()),
    ];
    let mut status = None;
    for _ in 0..100 {
        status = post(&public, "/webhooks/github", &headers, body);
        if status.is_some() {
            break;
        }
        sleep(Duration::from_millis(50));
    }
    assert_eq!(status.as_deref(), Some("HTTP/1.1 202 Accepted"));
    // The routing loop routes the delivery; CIHub does not use a ping.
    let mut stored = None;
    for _ in 0..100 {
        let mut tx = cihub_store::begin_system(&pool).await.unwrap();
        let row: (String, String, bool) = sqlx::query_as(
            "SELECT i.base_url, w.event, w.processed_at IS NOT NULL FROM webhook_inbox w
             JOIN scm_instances i ON i.id = w.scm_instance_id",
        )
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        tx.rollback().await.unwrap();
        let done = row.2;
        stored = Some(row);
        if done {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    stop(child);
    let (base_url, event, processed) = stored.unwrap();
    assert!(base_url.starts_with("http://127.0.0.1:"), "{base_url}");
    assert_eq!((event.as_str(), processed), ("ping", true));
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
fn without_github_the_server_stops_and_migrate_runs(pool: PgPool) {
    let output = command(&pool).stderr(Stdio::piped()).output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("GitHub is not configured"), "{stderr}");
    assert!(command(&pool).arg("migrate").status().unwrap().success());
}

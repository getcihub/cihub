//! Starts the `cihub-server` binary against Postgres: each `#[sqlx::test]`
//! gets a new database.

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

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

fn start(pool: &PgPool, addr: &str, roles: Option<&str>) -> Child {
    let mut command = command(pool);
    command.env("CIHUB_INTERNAL_ADDR", addr);
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
            body.contains(r#""roles":["api","scheduler","worker","notifier"]"#),
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
    let status = command(&pool).status().unwrap();
    assert!(!status.success(), "{status}");
    assert!(command(&pool).arg("migrate").status().unwrap().success());
}

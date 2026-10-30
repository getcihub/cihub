//! Starts the `cihub-server` binary against Postgres: each `#[sqlx::test]`
//! gets a new database. The test of the queue commands also runs the `cihub`
//! binary of `cihub-cli` against it.

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

use cihub_domain::{ApiScope, ApiTokenId, ConnectionId, OrgId, QueueId, RepoId, SessionId, UserId};
use ring::hmac;
use secrecy::{ExposeSecret, SecretString};
use sqlx::{AssertSqlSafe, PgPool};
use ulid::Ulid;

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

fn with_github(pool: &PgPool) -> Command {
    let mut command = command(pool);
    command
        .env("CIHUB_GITHUB_URL", format!("http://{}", free_addr()))
        .env("CIHUB_GITHUB_APP_ID", "1")
        .env("CIHUB_GITHUB_PRIVATE_KEY_FILE", TEST_KEY)
        .env("CIHUB_GITHUB_WEBHOOK_SECRET", "secret");
    command
}

fn start(pool: &PgPool, addr: &str) -> Server {
    Server(
        with_github(pool)
            .arg("start")
            .env("CIHUB_INTERNAL_ADDR", addr)
            .env("CIHUB_HTTP_ADDR", free_addr())
            .spawn()
            .unwrap(),
    )
}

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

/// A started `cihub-server`, killed when a test panics before [`stop`]: left
/// running, it would hold the database of the test, whose name is the same at
/// each run, and the next run could not drop it.
struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        if let Ok(None) = self.0.try_wait() {
            self.0.kill().ok();
            self.0.wait().ok();
        }
    }
}

fn stop(mut server: Server) {
    let status = Command::new("kill")
        .args(["-TERM", &server.0.id().to_string()])
        .status()
        .unwrap();
    assert!(status.success());
    let status = server.0.wait().unwrap();
    assert!(status.success(), "{status}");
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
fn all_in_one_answers_and_stops(pool: PgPool) {
    let addr = free_addr();
    let child = start(&pool, &addr);
    for path in ["/healthz", "/readyz"] {
        let (status, body) = wait_for(&addr, path);
        assert_eq!(status, "HTTP/1.1 200 OK");
        assert!(body.contains(r#""name":"cihub-server""#), "{body}");
    }
    stop(child);
}

#[sqlx::test(migrations = false)]
fn migrate_up_twice_succeeds(pool: PgPool) {
    for _ in 0..2 {
        let status = command(&pool).args(["migrate", "up"]).status().unwrap();
        assert!(status.success(), "{status}");
    }
}

#[sqlx::test(migrations = false)]
fn migrate_down_reverts_what_migrate_up_applied(pool: PgPool) {
    assert!(
        command(&pool)
            .args(["migrate", "up"])
            .status()
            .unwrap()
            .success()
    );

    let status = command(&pool)
        .args(["migrate", "down", "--to", "0"])
        .status()
        .unwrap();

    assert!(status.success(), "{status}");
    assert!(!with_github(&pool).arg("start").status().unwrap().success());
    assert!(
        command(&pool)
            .args(["migrate", "up"])
            .status()
            .unwrap()
            .success()
    );
}

#[sqlx::test(migrations = false)]
fn a_missing_migration_stops_the_start(pool: PgPool) {
    let status = with_github(&pool).arg("start").status().unwrap();
    assert!(!status.success(), "{status}");
    assert!(
        command(&pool)
            .args(["migrate", "up"])
            .status()
            .unwrap()
            .success()
    );
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn all_in_one_runs_the_maintenance_of_the_day(pool: PgPool) {
    let addr = free_addr();
    let child = start(&pool, &addr);
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
    let child = Server(
        with_github(&pool)
            .arg("start")
            .env("CIHUB_INTERNAL_ADDR", &internal)
            .env("CIHUB_HTTP_ADDR", &public)
            .spawn()
            .unwrap(),
    );

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
    let mut waiting = None;
    for _ in 0..100 {
        let mut tx = cihub_store::begin_system(&pool).await.unwrap();
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM webhook_inbox")
            .fetch_one(&mut *tx)
            .await
            .unwrap();

        tx.rollback().await.unwrap();
        waiting = Some(count);
        if count == 0 {
            break;
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    stop(child);

    assert_eq!(waiting, Some(0), "the router deletes the ping");
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
fn without_github_the_server_stops_and_migrate_up_runs(pool: PgPool) {
    let output = command(&pool)
        .arg("start")
        .stderr(Stdio::piped())
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("GitHub is not configured"), "{stderr}");
    assert!(
        command(&pool)
            .args(["migrate", "up"])
            .status()
            .unwrap()
            .success()
    );
}

/// Cargo gives the path of the binaries of this crate only; `cargo nextest
/// run --workspace` builds the one of `cihub-cli` in the same directory.
fn cli() -> PathBuf {
    let path = Path::new(env!("CARGO_BIN_EXE_cihub-server")).with_file_name("cihub");
    assert!(
        path.exists(),
        "{} is missing: build cihub-cli first",
        path.display()
    );
    path
}

async fn two_queues(pool: &PgPool) -> String {
    let org = OrgId::from_ulid(Ulid(1));
    let connection = ConnectionId::from_ulid(Ulid(4));
    let repo = RepoId::from_ulid(Ulid(5));
    let mut tx = cihub_store::begin_system(pool).await.unwrap();

    sqlx::raw_sql(AssertSqlSafe(format!(
        "INSERT INTO orgs (id, slug) VALUES ('{org}', 'acme');
         INSERT INTO scm_connections (id, org_id, account_external_id, kind, external_id,
                                      synced_at, permissions)
             VALUES ('{connection}', '{org}', '1', 'github_app', '1', now(), '{{}}');
         INSERT INTO repos (id, org_id, connection_id, external_id, owner, name, default_branch)
             VALUES ('{repo}', '{org}', '{connection}', '200', 'acme', 'app', 'main');
         INSERT INTO queues (id, org_id, repo_id, target_branch)
             VALUES ('{main}', '{org}', '{repo}', 'main'),
                    ('{release}', '{org}', '{repo}', 'release/1');",
        main = QueueId::from_ulid(Ulid(6)),
        release = QueueId::from_ulid(Ulid(7)),
    )))
    .execute(&mut *tx)
    .await
    .unwrap();
    let secret = cihub_platform::new_api_token_secret().unwrap();
    let token = cihub_platform::NewApiToken {
        id: ApiTokenId::from_ulid(Ulid(8)),
        org_id: org,
        name: "test",
        scopes: &[
            ApiScope::QueuesRead,
            ApiScope::QueuesWrite,
            ApiScope::PrsWrite,
        ],
        expires_at: None,
    };

    cihub_platform::create_api_token(&mut tx, &token, &secret)
        .await
        .unwrap();

    tx.commit().await.unwrap();
    secret.expose_secret().to_owned()
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_queue_commands_drive_the_api(pool: PgPool) {
    let token = two_queues(&pool).await;
    let (internal, public) = (free_addr(), free_addr());
    let child = Server(
        with_github(&pool)
            .arg("start")
            .env("CIHUB_INTERNAL_ADDR", &internal)
            .env("CIHUB_HTTP_ADDR", &public)
            .spawn()
            .unwrap(),
    );

    wait_for(&public, "/api/v1/openapi.json");
    let cihub = |args: &[&str]| {
        Command::new(cli())
            .args(["queue", "--repo", "acme/app"])
            .args(args)
            .env_clear()
            .env("CIHUB_HOST", &public)
            .env("CIHUB_TOKEN", &token)
            .output()
            .unwrap()
    };
    let drained = cihub(&["drain"]);
    let stdout = String::from_utf8_lossy(&drained.stdout);

    assert!(drained.status.success(), "{stdout}");
    assert_eq!(
        stdout, "queue of `main`: drain asked\n",
        "without `--branch`, the queue of the default branch"
    );
    let status = cihub(&["status", "--branch", "release/1", "--json"]);
    let read: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();

    assert_eq!(read["queue"]["target_branch"], "release/1");
    assert_eq!(read["queue"]["state"], "running");
    assert_eq!(read["prs"], serde_json::json!([]));
    stop(child);
}

/// A session of the dashboard of `ada`: its cookie and its CSRF token.
async fn dashboard_session(pool: &PgPool) -> (String, String) {
    let user_id = UserId::from_ulid(Ulid(5));
    let secret = cihub_web::new_session_secret().unwrap();
    let session = cihub_web::NewSession {
        id: SessionId::from_ulid(Ulid(5)),
        user_id,
        // The server reads the clock: the session outlives the test.
        created_at: time::OffsetDateTime::from_unix_timestamp(1_790_000_000).unwrap(),
        expires_at: time::OffsetDateTime::from_unix_timestamp(4_000_000_000).unwrap(),
    };
    let mut tx = cihub_store::begin_system(pool).await.unwrap();

    cihub_platform::ensure_user(&mut tx, user_id, "5", "ada")
        .await
        .unwrap();
    cihub_web::create_session(&mut tx, &session, &secret)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    (
        format!("__Host-cihub_session={}", secret.expose_secret()),
        cihub_web::csrf_token(&secret),
    )
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_command_line_signs_in_with_a_code_that_its_person_approves(pool: PgPool) {
    let (cookie, csrf) = dashboard_session(&pool).await;
    let (internal, public) = (free_addr(), free_addr());
    let child = Server(
        with_github(&pool)
            .arg("start")
            .env("CIHUB_INTERNAL_ADDR", &internal)
            .env("CIHUB_HTTP_ADDR", &public)
            .env("CIHUB_HTTP_PUBLIC_URL", format!("http://{public}"))
            .env("CIHUB_DATABASE_SECRET", "a database secret of 32 bytes ab")
            .spawn()
            .unwrap(),
    );
    let config = std::env::temp_dir().join(format!("cihub-{}", public.replace(':', "-")));
    let cihub = |args: &[&str]| {
        let mut command = Command::new(cli());
        command
            .args(args)
            .env_clear()
            .env("XDG_CONFIG_HOME", &config)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    };

    wait_for(&public, "/api/v1/openapi.json");
    let mut login = cihub(&["auth", "login", "--hostname", &public])
        .spawn()
        .unwrap();
    let mut prompt = String::new();
    let mut reader = BufReader::new(login.stdout.as_mut().unwrap());
    while !prompt.contains("type this code:\n\n  ") {
        reader.read_line(&mut prompt).unwrap();
    }
    let user_code = prompt
        .split("type this code:")
        .nth(1)
        .map(str::trim)
        .unwrap();

    assert!(
        prompt.starts_with(&format!(
            "To sign in, open this address (it expires in 15 minutes):\n\n  http://{public}/device\n\n"
        )),
        "{prompt}"
    );
    let approved = post(
        &public,
        &format!("/api/v1/device-codes/{user_code}/approve"),
        &[("Cookie", &cookie), ("X-CSRF-Token", &csrf)],
        "",
    );
    let login = login.wait_with_output().unwrap();

    assert_eq!(approved.as_deref(), Some("HTTP/1.1 204 No Content"));
    assert!(
        login.status.success(),
        "{}",
        String::from_utf8_lossy(&login.stderr)
    );
    assert!(
        String::from_utf8_lossy(&login.stdout)
            .starts_with(&format!("Signed in to {public} as ada;")),
        "{}",
        String::from_utf8_lossy(&login.stdout)
    );
    let status = cihub(&["auth", "status"]).output().unwrap();
    let token = cihub(&["auth", "token"]).output().unwrap();

    assert!(
        String::from_utf8_lossy(&status.stdout)
            .starts_with(&format!("{public}\tsigned in as ada\t")),
        "{}",
        String::from_utf8_lossy(&status.stdout)
    );
    assert!(String::from_utf8_lossy(&token.stdout).starts_with("chc_"));
    let logout = cihub(&["auth", "logout"]).output().unwrap();
    let after = cihub(&["auth", "status"]).output().unwrap();

    assert!(
        logout.status.success(),
        "{}",
        String::from_utf8_lossy(&logout.stderr)
    );
    assert!(!after.status.success());
    stop(child);
    std::fs::remove_dir_all(&config).unwrap();
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn token_create_shows_one_token_with_every_scope(pool: PgPool) {
    two_queues(&pool).await;
    let create = |slug: &str| {
        command(&pool)
            .args(["token", "create", "--org", slug])
            .stdout(Stdio::piped())
            .output()
            .unwrap()
    };

    let created = create("acme");

    assert!(created.status.success(), "{}", created.status);
    let stdout = String::from_utf8(created.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();

    assert_eq!(lines.len(), 1, "{stdout}");
    let token = SecretString::from(lines[0]);
    let now = time::OffsetDateTime::from_unix_timestamp(1_790_000_000).unwrap();
    let grant = cihub_platform::authenticate(&pool, &token, now)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(grant.org_id, OrgId::from_ulid(Ulid(1)));
    for scope in [
        ApiScope::QueuesRead,
        ApiScope::QueuesWrite,
        ApiScope::PrsWrite,
    ] {
        assert!(grant.scopes.contains(&scope), "{:?}", grant.scopes);
    }

    let mut tx = cihub_store::begin_system(&pool).await.unwrap();
    let audited: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE action = 'token.created'")
            .fetch_one(&mut *tx)
            .await
            .unwrap();

    drop(tx);

    assert_eq!(audited, 1);

    let unknown = create("missing");
    assert!(!unknown.status.success());
    assert_eq!(unknown.stdout, b"");
}

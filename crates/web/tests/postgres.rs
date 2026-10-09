//! Tests against Postgres: each `#[sqlx::test]` gets a new database.

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

use std::fmt::Write;

use cihub_domain::{ApiScope, ApiTokenId, OrgId};
use cihub_store::{NewApiToken, begin_system, create_api_token, new_api_token_secret};
use cihub_web::GithubWebhooks;
use ring::hmac;
use secrecy::{ExposeSecret, SecretString};
use sqlx::PgPool;
use time::OffsetDateTime;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;
use ulid::Ulid;

const SECRET: &str = "webhook secret";
const BODY: &str = r#"{"action":"opened","number":1}"#;

fn now() -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(1_789_000_000).unwrap()
}

async fn start(pool: &PgPool) -> String {
    let github = GithubWebhooks {
        secret: SecretString::from(SECRET),
    };
    let router = cihub_web::router(pool.clone(), now, github, Vec::new());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    tokio::spawn(cihub_web::serve(listener, router, CancellationToken::new()));
    addr
}

fn signature(secret: &str, body: &str) -> String {
    let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
    let tag = hmac::sign(&key, body.as_bytes());
    let hex = tag.as_ref().iter().fold(String::new(), |mut hex, b| {
        let _ = write!(hex, "{b:02x}");
        hex
    });
    format!("sha256={hex}")
}

async fn post(addr: &str, headers: &[(&str, &str)], body: &str) -> u16 {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let mut request = format!(
        "POST /webhooks/github HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\
         Content-Type: application/json\r\nContent-Length: {}\r\n",
        body.len()
    );
    for (name, value) in headers {
        let _ = write!(request, "{name}: {value}\r\n");
    }
    request.push_str("\r\n");
    request.push_str(body);
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    response.split(' ').nth(1).unwrap().parse().unwrap()
}

type InboxRow = (
    String,
    Option<String>,
    Option<String>,
    String,
    OffsetDateTime,
);

async fn inbox(pool: &PgPool) -> Vec<InboxRow> {
    let mut tx = begin_system(pool).await.unwrap();
    sqlx::query_as(
        "SELECT delivery_id, org_id, queue_id, event, received_at
         FROM webhook_inbox ORDER BY delivery_id",
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap()
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_signed_delivery_is_stored_once_and_unrouted(pool: PgPool) {
    let addr = start(&pool).await;
    let signature = signature(SECRET, BODY);
    let headers = [
        ("X-GitHub-Delivery", "d1"),
        ("X-GitHub-Event", "pull_request"),
        ("X-Hub-Signature-256", signature.as_str()),
    ];
    assert_eq!(post(&addr, &headers, BODY).await, 202);
    assert_eq!(post(&addr, &headers, BODY).await, 202);
    assert_eq!(
        inbox(&pool).await,
        [("d1".into(), None, None, "pull_request".into(), now())]
    );
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_delivery_without_a_valid_signature_or_its_headers_is_refused(pool: PgPool) {
    let addr = start(&pool).await;
    let wrong = signature("another secret", BODY);
    let unsigned = [("X-GitHub-Delivery", "d1"), ("X-GitHub-Event", "ping")];
    let wrongly_signed = [
        ("X-GitHub-Delivery", "d1"),
        ("X-GitHub-Event", "ping"),
        ("X-Hub-Signature-256", wrong.as_str()),
    ];
    assert_eq!(post(&addr, &unsigned, BODY).await, 401);
    assert_eq!(post(&addr, &wrongly_signed, BODY).await, 401);
    let right = signature(SECRET, BODY);
    let no_event = [
        ("X-GitHub-Delivery", "d1"),
        ("X-Hub-Signature-256", right.as_str()),
    ];
    assert_eq!(post(&addr, &no_event, BODY).await, 400);
    assert_eq!(inbox(&pool).await, []);
}

struct Answer {
    status: u16,
    head: String,
    body: String,
}

async fn call(addr: &str, method: &str, path: &str, token: Option<&str>) -> Answer {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let mut request = format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n");
    if let Some(token) = token {
        let _ = write!(request, "Authorization: Bearer {token}\r\n");
    }
    request.push_str("\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    let (head, body) = response.split_once("\r\n\r\n").unwrap();
    Answer {
        status: head.split(' ').nth(1).unwrap().parse().unwrap(),
        head: head.to_lowercase(),
        body: body.to_owned(),
    }
}

async fn token(pool: &PgPool, number: u128, scopes: &[ApiScope]) -> String {
    let org = OrgId::from_ulid(Ulid(7));
    let mut tx = begin_system(pool).await.unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "INSERT INTO orgs (id, slug) VALUES ('{org}', 'acme') ON CONFLICT DO NOTHING;
         INSERT INTO scm_connections (id, org_id, account_external_id, kind, external_id,
                                      synced_at, permissions)
             VALUES ('c', '{org}', '1', 'github_app', '1', now(), '{{}}') ON CONFLICT DO NOTHING;
         INSERT INTO repos (id, org_id, connection_id, external_id, namespace, name, default_branch)
             VALUES ('r', '{org}', 'c', '1', 'acme', 'app', 'main') ON CONFLICT DO NOTHING;"
    )))
    .execute(&mut *tx)
    .await
    .unwrap();
    let secret = new_api_token_secret().unwrap();
    let id = ApiTokenId::from_ulid(Ulid(number));
    let new = NewApiToken {
        id,
        org_id: org,
        name: "test",
        scopes,
        expires_at: None,
    };
    create_api_token(&mut tx, &new, &secret).await.unwrap();
    tx.commit().await.unwrap();
    secret.expose_secret().to_owned()
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_api_wants_a_token_with_the_scope_of_the_route(pool: PgPool) {
    let addr = start(&pool).await;
    let reader = token(&pool, 1, &[ApiScope::QueuesRead]).await;
    let writer = token(&pool, 2, &[ApiScope::PrsWrite]).await;

    let anonymous = call(&addr, "GET", "/api/v1/repos", None).await;
    assert_eq!(anonymous.status, 401);
    assert!(
        anonymous
            .head
            .contains("content-type: application/problem+json")
    );
    let problem: serde_json::Value = serde_json::from_str(&anonymous.body).unwrap();
    assert_eq!(problem["code"], "unauthorized");
    assert_eq!(problem["status"], 401);
    assert_eq!(
        call(&addr, "GET", "/api/v1/repos", Some("cht_unknown"))
            .await
            .status,
        401
    );

    let forbidden = call(&addr, "GET", "/api/v1/repos", Some(&writer)).await;
    assert_eq!(forbidden.status, 403, "{}", forbidden.body);
    let repos = call(&addr, "GET", "/api/v1/repos", Some(&reader)).await;
    assert_eq!(repos.status, 200, "{}", repos.body);
    let repos: serde_json::Value = serde_json::from_str(&repos.body).unwrap();
    assert_eq!(repos["repos"][0]["name"], "app");

    let unknown = call(&addr, "GET", "/api/v1/nothing", Some(&reader)).await;
    assert_eq!(unknown.status, 404);
    assert!(unknown.head.contains("application/problem+json"));

    let document = call(&addr, "GET", "/api/v1/openapi.json", None).await;
    assert_eq!(document.status, 200);
    let document: serde_json::Value = serde_json::from_str(&document.body).unwrap();
    assert!(document["openapi"].as_str().unwrap().starts_with("3.1"));
    assert!(document["paths"]["/repos"]["get"].is_object());
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_token_past_its_requests_of_the_minute_waits(pool: PgPool) {
    let addr = start(&pool).await;
    let reader = token(&pool, 1, &[ApiScope::QueuesRead]).await;
    for _ in 0..cihub_web::REQUESTS_PER_MINUTE {
        assert_eq!(
            call(&addr, "GET", "/api/v1/repos", Some(&reader))
                .await
                .status,
            200
        );
    }
    let limited = call(&addr, "GET", "/api/v1/repos", Some(&reader)).await;
    assert_eq!(limited.status, 429);
    assert!(limited.head.contains("retry-after: 20"), "{}", limited.head);
    let problem: serde_json::Value = serde_json::from_str(&limited.body).unwrap();
    assert_eq!(problem["code"], "rate_limited");
}

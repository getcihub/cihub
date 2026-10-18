//! Tests against Postgres: each `#[sqlx::test]` gets a new database.

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

use std::fmt::Write;
use std::sync::Arc;

use cihub_domain::{ApiScope, ApiTokenId, MembershipRole, OrgId, QueueId, SessionId, UserId};
use cihub_platform::{QueueNewsFeed, notify_queue_changed};
use cihub_scm::sim::Sim;
use cihub_scm::{ScmAccount, ScmUser, SignInRequest};
use cihub_store::{
    NewApiToken, NewSession, begin_org, begin_system, create_api_token, create_session, csrf_token,
    ensure_user, new_api_token_secret, new_session_secret, record_membership, record_profile,
};
use cihub_web::{Dashboard, GithubWebhooks};
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
    start_with(pool, Arc::new(Sim::new())).await
}

const PUBLIC_URL: &str = "https://ci.example.com";

async fn start_with(pool: &PgPool, sim: Arc<Sim>) -> String {
    start_until(pool, sim, CancellationToken::new()).await
}

/// Serves the web role until `cancel`.
async fn start_until(pool: &PgPool, sim: Arc<Sim>, cancel: CancellationToken) -> String {
    let github = GithubWebhooks {
        secret: SecretString::from(SECRET),
    };
    let dashboard = Dashboard {
        scm: sim,
        public_url: Some(PUBLIC_URL.to_owned()),
        news: QueueNewsFeed::start(pool.clone(), cancel.clone())
            .await
            .unwrap(),
    };
    let router = cihub_web::router(pool.clone(), now, github, Vec::new(), dashboard);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();

    tokio::spawn(cihub_web::serve(listener, router, cancel));
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
    /// The head in lower case, to compare the names of the headers.
    head: String,
    /// The head as the server sent it: a value keeps its case.
    raw_head: String,
    body: String,
}

async fn call(addr: &str, method: &str, path: &str, token: Option<&str>) -> Answer {
    let authorization = token.map(|token| format!("Bearer {token}"));
    let headers: Vec<(&str, &str)> = authorization
        .iter()
        .map(|value| ("Authorization", value.as_str()))
        .collect();
    call_with(addr, method, path, &headers).await
}

async fn call_with(addr: &str, method: &str, path: &str, headers: &[(&str, &str)]) -> Answer {
    call_with_body(addr, method, path, headers, "").await
}

async fn call_with_body(
    addr: &str,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> Answer {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let mut request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );

    if !body.is_empty() {
        request.push_str("Content-Type: application/json\r\n");
    }
    for (name, value) in headers {
        let _ = write!(request, "{name}: {value}\r\n");
    }

    request.push_str("\r\n");
    request.push_str(body);
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    let (head, body) = response.split_once("\r\n\r\n").unwrap();
    Answer {
        status: head.split(' ').nth(1).unwrap().parse().unwrap(),
        head: head.to_lowercase(),
        raw_head: head.to_owned(),
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
    let admin = token(&pool, 2, &[ApiScope::QueuesWrite]).await;

    let anonymous = call(&addr, "GET", "/api/v1/orgs/acme/audit", None).await;

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
        call(&addr, "GET", "/api/v1/orgs/acme/audit", Some("cht_unknown"))
            .await
            .status,
        401
    );

    let forbidden = call(&addr, "GET", "/api/v1/orgs/acme/audit", Some(&reader)).await;
    assert_eq!(forbidden.status, 403, "{}", forbidden.body);
    let audit = call(&addr, "GET", "/api/v1/orgs/acme/audit", Some(&admin)).await;
    assert_eq!(audit.status, 200, "{}", audit.body);
    let audit: serde_json::Value = serde_json::from_str(&audit.body).unwrap();
    assert_eq!(audit["entries"], serde_json::json!([]));

    let unknown = call(&addr, "GET", "/api/v1/nothing", Some(&reader)).await;
    assert_eq!(unknown.status, 404);
    assert!(unknown.head.contains("application/problem+json"));

    let document = call(&addr, "GET", "/api/v1/openapi.json", None).await;
    assert_eq!(document.status, 200);
    let document: serde_json::Value = serde_json::from_str(&document.body).unwrap();
    assert!(document["openapi"].as_str().unwrap().starts_with("3.1"));
    assert_eq!(
        document["paths"]["/orgs/{org}/audit"]["get"]["security"],
        serde_json::json!([{ "token": ["queues:write"] }, { "session": [] }])
    );
}

/// Reads `stream` until its text holds `marker`, at most 5 seconds; `None`
/// once the server closes it.
async fn read_until(stream: &mut TcpStream, marker: &str) -> Option<String> {
    let mut text = String::new();
    let mut buffer = [0; 4096];
    let reading = async {
        while !text.contains(marker) {
            let read = stream.read(&mut buffer).await.unwrap();
            if read == 0 {
                return None;
            }
            text.push_str(&String::from_utf8_lossy(&buffer[..read]));
        }
        Some(text.clone())
    };

    tokio::time::timeout(std::time::Duration::from_secs(5), reading)
        .await
        .unwrap()
}

/// The stream of an organization carries the changes of its queues only, and
/// ends when the server stops.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_stream_carries_the_changes_of_the_organization_only(pool: PgPool) {
    let cancel = CancellationToken::new();
    let addr = start_until(&pool, Arc::new(Sim::new()), cancel.clone()).await;
    let reader = token(&pool, 1, &[ApiScope::QueuesRead]).await;
    let mut stream = TcpStream::connect(&addr).await.unwrap();
    let request = format!(
        "GET /api/v1/orgs/acme/stream HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {reader}\r\n\r\n"
    );

    stream.write_all(request.as_bytes()).await.unwrap();

    let head = read_until(&mut stream, "retry: 1000").await.unwrap();

    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
    assert!(head.contains("content-type: text/event-stream"), "{head}");

    let acme_queue = QueueId::from_ulid(Ulid(30));
    let other_queue = QueueId::from_ulid(Ulid(31));
    let mut tx = begin_system(&pool).await.unwrap();

    notify_queue_changed(&mut tx, OrgId::from_ulid(Ulid(8)), other_queue, Some(3))
        .await
        .unwrap();
    notify_queue_changed(&mut tx, OrgId::from_ulid(Ulid(7)), acme_queue, None)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let events = read_until(&mut stream, &acme_queue.to_string())
        .await
        .unwrap();

    assert!(events.contains("event: queue_changed"), "{events}");
    assert!(
        events.contains(&format!(r#"data: {{"queue_id":"{acme_queue}","seq":0}}"#)),
        "{events}"
    );
    assert!(!events.contains(&other_queue.to_string()), "{events}");

    cancel.cancel();

    // The last chunk of the body, or a closed connection: the stream ended.
    let end = read_until(&mut stream, "\r\n0\r\n\r\n").await;
    assert!(
        end.as_deref()
            .is_none_or(|end| end.ends_with("\r\n0\r\n\r\n")),
        "{end:?}"
    );
}

/// The stream answers as the other routes: `404` on another organization,
/// `403` without `queues:read`, and `429` past the streams of a caller.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_stream_is_refused_as_the_other_routes(pool: PgPool) {
    let cancel = CancellationToken::new();
    let addr = start_until(&pool, Arc::new(Sim::new()), cancel.clone()).await;
    let reader = token(&pool, 1, &[ApiScope::QueuesRead]).await;
    let writer = token(&pool, 2, &[ApiScope::PrsWrite]).await;

    let other = call(&addr, "GET", "/api/v1/orgs/globex/stream", Some(&reader)).await;
    let forbidden = call(&addr, "GET", "/api/v1/orgs/acme/stream", Some(&writer)).await;

    assert_eq!(other.status, 404, "{}", other.body);
    assert_eq!(forbidden.status, 403, "{}", forbidden.body);

    let mut open = Vec::new();
    for _ in 0..8 {
        let mut stream = TcpStream::connect(&addr).await.unwrap();
        let request = format!(
            "GET /api/v1/orgs/acme/stream HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {reader}\r\n\r\n"
        );
        stream.write_all(request.as_bytes()).await.unwrap();
        read_until(&mut stream, "retry: 1000").await.unwrap();
        open.push(stream);
    }

    let ninth = call(&addr, "GET", "/api/v1/orgs/acme/stream", Some(&reader)).await;
    assert_eq!(ninth.status, 429, "{}", ninth.body);

    // A page that leaves is seen at the next ping; the server ends the streams
    // at once when it stops.
    cancel.cancel();
    drop(open);
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_token_past_its_requests_of_the_minute_waits(pool: PgPool) {
    let addr = start(&pool).await;
    let reader = token(&pool, 1, &[ApiScope::QueuesRead]).await;

    for _ in 0..cihub_web::REQUESTS_PER_MINUTE {
        assert_eq!(
            call(&addr, "GET", "/api/v1/orgs/acme", Some(&reader))
                .await
                .status,
            200
        );
    }

    let limited = call(&addr, "GET", "/api/v1/orgs/acme", Some(&reader)).await;
    assert_eq!(limited.status, 429);
    assert!(limited.head.contains("retry-after: 20"), "{}", limited.head);
    let problem: serde_json::Value = serde_json::from_str(&limited.body).unwrap();
    assert_eq!(problem["code"], "rate_limited");
}

/// A session of a user with `role` in `acme`, and its secret.
async fn session(pool: &PgPool, role: MembershipRole) -> SecretString {
    token(pool, 9, &[]).await;
    let org = OrgId::from_ulid(Ulid(7));
    let user_id = UserId::from_ulid(Ulid(5));
    let secret = new_session_secret().unwrap();
    let session = NewSession {
        id: SessionId::from_ulid(Ulid(1)),
        user_id,
        created_at: now(),
        expires_at: now() + time::Duration::days(14),
    };
    let mut tx = begin_system(pool).await.unwrap();

    ensure_user(&mut tx, user_id, "5", "ada").await.unwrap();
    record_profile(
        &mut tx,
        user_id,
        Some("Ada Lovelace"),
        Some("https://avatars.example.com/u/5"),
    )
    .await
    .unwrap();
    create_session(&mut tx, &session, &secret).await.unwrap();
    tx.commit().await.unwrap();
    let mut tx = begin_org(pool, org).await.unwrap();
    record_membership(&mut tx, org, user_id, role, now())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    secret
}

fn session_cookie(secret: &SecretString) -> String {
    format!(
        "theme=dark; __Host-cihub_session={}",
        secret.expose_secret()
    )
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_session_acts_with_the_role_of_its_user_in_the_organization(pool: PgPool) {
    let addr = start(&pool).await;
    let secret = session(&pool, MembershipRole::Member).await;
    let cookie = session_cookie(&secret);
    let with_session = [("Cookie", cookie.as_str())];
    let unknown = [("Cookie", "__Host-cihub_session=chs_unknown")];

    let me = call_with(&addr, "GET", "/api/v1/user", &with_session).await;
    let orgs = call_with(&addr, "GET", "/api/v1/user/orgs", &with_session).await;
    let org = call_with(&addr, "GET", "/api/v1/orgs/acme", &with_session).await;
    let upper = call_with(&addr, "GET", "/api/v1/orgs/ACME", &with_session).await;
    let audit = call_with(&addr, "GET", "/api/v1/orgs/acme/audit", &with_session).await;
    let other = call_with(&addr, "GET", "/api/v1/orgs/globex", &with_session).await;
    let anonymous = call_with(&addr, "GET", "/api/v1/user", &unknown).await;

    assert_eq!(me.status, 200, "{}", me.body);
    let me: serde_json::Value = serde_json::from_str(&me.body).unwrap();
    let orgs: serde_json::Value = serde_json::from_str(&orgs.body).unwrap();
    let org: serde_json::Value = serde_json::from_str(&org.body).unwrap();
    assert_eq!(
        me,
        serde_json::json!({
            "login": "ada",
            "name": "Ada Lovelace",
            "avatar_url": "https://avatars.example.com/u/5"
        })
    );
    assert_eq!(
        orgs,
        serde_json::json!({
            "orgs": [{ "slug": "acme", "role": "member", "is_suspended": false }],
            "has_more": false, "count": 1, "cursor": null
        })
    );
    assert_eq!(
        org,
        serde_json::json!({ "slug": "acme", "role": "member", "is_suspended": false })
    );
    assert_eq!(
        upper.status, 200,
        "the case of a slug does not matter: {}",
        upper.body
    );
    assert_eq!(audit.status, 403, "{}", audit.body);
    assert_eq!(other.status, 404, "{}", other.body);
    assert_eq!(anonymous.status, 401);
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_sign_out_needs_the_csrf_token_and_ends_the_session(pool: PgPool) {
    let addr = start(&pool).await;
    let secret = session(&pool, MembershipRole::Admin).await;
    let reader = token(&pool, 1, &[ApiScope::QueuesRead]).await;
    let cookie = session_cookie(&secret);
    let csrf = csrf_token(&secret);
    let forged = [("Cookie", cookie.as_str()), ("X-CSRF-Token", "guess")];
    let signed = [("Cookie", cookie.as_str()), ("X-CSRF-Token", csrf.as_str())];

    let refused = call_with(&addr, "POST", "/auth/sign-out", &forged).await;
    let by_token = call(&addr, "GET", "/api/v1/user", Some(&reader)).await;
    let instance = call(&addr, "GET", "/api/v1/instance", Some(&reader)).await;
    let org = call(&addr, "GET", "/api/v1/orgs/acme", Some(&reader)).await;
    let signed_out = call_with(&addr, "POST", "/auth/sign-out", &signed).await;
    let after = call_with(&addr, "GET", "/api/v1/user", &signed).await;

    assert_eq!(refused.status, 403, "{}", refused.body);
    assert_eq!(by_token.status, 401, "{}", by_token.body);
    assert_eq!(instance.status, 200, "{}", instance.body);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&org.body).unwrap(),
        serde_json::json!({ "slug": "acme", "role": null, "is_suspended": false }),
        "a token has no role"
    );
    assert_eq!(signed_out.status, 204);
    assert!(
        signed_out.head.contains(
            "set-cookie: __host-cihub_session=; path=/; secure; samesite=lax; max-age=0; httponly"
        ),
        "{}",
        signed_out.head
    );
    assert!(
        signed_out
            .head
            .contains("set-cookie: __host-cihub_csrf=; path=/; secure; samesite=lax; max-age=0\r"),
        "{}",
        signed_out.head
    );
    assert_eq!(after.status, 401);
}

/// A device code of the instance at `addr`: its secret and its user code.
async fn start_device_sign_in(addr: &str) -> (String, String) {
    let created = call_with(addr, "POST", "/api/v1/device-codes", &[]).await;

    assert_eq!(created.status, 200, "{}", created.body);
    let created: serde_json::Value = serde_json::from_str(&created.body).unwrap();
    assert_eq!(created["verification_url"], format!("{PUBLIC_URL}/device"));
    assert_eq!(
        (created["expires_in"].as_u64(), created["interval"].as_u64()),
        (Some(900), Some(5))
    );
    (
        created["device_code"].as_str().unwrap().to_owned(),
        created["user_code"].as_str().unwrap().to_owned(),
    )
}

async fn ask_session(addr: &str, device_code: &str) -> Answer {
    let body = serde_json::json!({ "device_code": device_code }).to_string();
    call_with_body(addr, "POST", "/api/v1/device-codes/token", &[], &body).await
}

fn code_of(answer: &Answer) -> String {
    let problem: serde_json::Value = serde_json::from_str(&answer.body).unwrap();
    problem["code"].as_str().unwrap_or_default().to_owned()
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_command_line_signs_in_once_its_person_approves_the_code(pool: PgPool) {
    let addr = start(&pool).await;
    let secret = session(&pool, MembershipRole::Member).await;
    let cookie = session_cookie(&secret);
    let csrf = csrf_token(&secret);
    let dashboard = [("Cookie", cookie.as_str()), ("X-CSRF-Token", csrf.as_str())];
    let (waiting, _) = start_device_sign_in(&addr).await;
    let (device_code, user_code) = start_device_sign_in(&addr).await;

    let pending = ask_session(&addr, &waiting).await;
    let too_fast = ask_session(&addr, &waiting).await;
    let typed = user_code.to_lowercase().replace('-', "%20");
    let approve = format!("/api/v1/device-codes/{typed}/approve");
    let forged = call_with(&addr, "POST", &approve, &[dashboard[0]]).await;
    let approved = call_with(&addr, "POST", &approve, &dashboard).await;
    let again = call_with(&addr, "POST", &approve, &dashboard).await;
    let signed_in = ask_session(&addr, &device_code).await;
    let used = ask_session(&addr, &device_code).await;

    assert_eq!(
        (pending.status, code_of(&pending)),
        (400, "device_code_pending".into())
    );
    assert_eq!(
        (too_fast.status, code_of(&too_fast)),
        (429, "polling_too_fast".into())
    );
    assert_eq!(forged.status, 403, "{}", forged.body);
    assert_eq!(approved.status, 204, "{}", approved.body);
    assert_eq!(
        (again.status, code_of(&again)),
        (404, "device_code_not_found".into())
    );
    assert_eq!(signed_in.status, 200, "{}", signed_in.body);
    assert_eq!(
        (used.status, code_of(&used)),
        (400, "device_code_expired".into())
    );
    let signed_in: serde_json::Value = serde_json::from_str(&signed_in.body).unwrap();
    let token = signed_in["token"].as_str().unwrap();
    assert!(token.starts_with("chc_"), "{token}");
    assert_eq!(signed_in["login"], "ada");

    let me = call(&addr, "GET", "/api/v1/user", Some(token)).await;
    let org = call(&addr, "GET", "/api/v1/orgs/acme", Some(token)).await;
    let as_cookie = format!("__Host-cihub_session={token}");
    let in_cookie = call_with(
        &addr,
        "GET",
        "/api/v1/user",
        &[("Cookie", as_cookie.as_str())],
    )
    .await;
    let (_, other_code) = start_device_sign_in(&addr).await;
    let by_command_line = call(
        &addr,
        "POST",
        &format!("/api/v1/device-codes/{other_code}/approve"),
        Some(token),
    )
    .await;
    let signed_out = call(&addr, "DELETE", "/api/v1/user/session", Some(token)).await;
    let after = call(&addr, "GET", "/api/v1/user", Some(token)).await;

    let me: serde_json::Value = serde_json::from_str(&me.body).unwrap();
    assert_eq!(me["login"], "ada");
    assert_eq!(
        org.body,
        r#"{"slug":"acme","role":"member","is_suspended":false}"#
    );
    assert_eq!(
        in_cookie.status, 401,
        "a session of the command line is no cookie"
    );
    assert_eq!(by_command_line.status, 401, "{}", by_command_line.body);
    assert_eq!(signed_out.status, 204, "{}", signed_out.body);
    assert_eq!(after.status, 401);
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_denied_code_signs_no_command_line_in(pool: PgPool) {
    let addr = start(&pool).await;
    let secret = session(&pool, MembershipRole::Member).await;
    let cookie = session_cookie(&secret);
    let csrf = csrf_token(&secret);
    let dashboard = [("Cookie", cookie.as_str()), ("X-CSRF-Token", csrf.as_str())];
    let (device_code, user_code) = start_device_sign_in(&addr).await;

    let unknown = call_with(
        &addr,
        "POST",
        "/api/v1/device-codes/BCDF-GHJ/deny",
        &dashboard,
    )
    .await;
    let denied = call_with(
        &addr,
        "POST",
        &format!("/api/v1/device-codes/{user_code}/deny"),
        &dashboard,
    )
    .await;
    let refused = ask_session(&addr, &device_code).await;
    let malformed = call_with_body(&addr, "POST", "/api/v1/device-codes/token", &[], "{}").await;

    assert_eq!(
        (unknown.status, code_of(&unknown)),
        (404, "device_code_not_found".into())
    );
    assert_eq!(denied.status, 204, "{}", denied.body);
    assert_eq!(
        (refused.status, code_of(&refused)),
        (403, "device_code_denied".into())
    );
    assert_eq!(
        (malformed.status, code_of(&malformed)),
        (400, "invalid_request".into())
    );
}

/// `text` with each `%XX` of a query decoded.
fn percent_decode(text: &str) -> String {
    let mut bytes = Vec::new();
    let mut rest = text.as_bytes();
    while let Some((&byte, tail)) = rest.split_first() {
        if byte == b'%' && tail.len() >= 2 {
            let hex = std::str::from_utf8(&tail[..2]).unwrap();
            bytes.push(u8::from_str_radix(hex, 16).unwrap());
            rest = &tail[2..];
        } else {
            bytes.push(byte);
            rest = tail;
        }
    }
    String::from_utf8(bytes).unwrap()
}

/// The values of the header `name` of the raw head of an answer.
fn header_values(raw_head: &str, name: &str) -> Vec<String> {
    raw_head
        .lines()
        .filter_map(|line| line.split_once(": "))
        .filter(|(header, _)| header.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.to_owned())
        .collect()
}

/// The value that a `Set-Cookie` header gives to the cookie `name`.
fn cookie_set(raw_head: &str, name: &str) -> Option<String> {
    let prefix = format!("{name}=");
    header_values(raw_head, "set-cookie")
        .iter()
        .find_map(|cookie| {
            cookie
                .strip_prefix(&prefix)?
                .split(';')
                .next()
                .map(str::to_owned)
        })
}

/// Starts a sign-in, signs `person` in on the simulator, and returns the
/// answer of the callback.
async fn sign_in(addr: &str, sim: &Sim, person: &ScmUser, next: &str) -> Answer {
    let started = call_with(addr, "GET", &format!("/auth/github?next={next}"), &[]).await;
    let location = header_values(&started.raw_head, "location").pop().unwrap();
    let query = location.split_once('?').unwrap().1;
    let param = |name: &str| {
        let value = query
            .split('&')
            .find_map(|pair| pair.strip_prefix(&format!("{name}=")))
            .unwrap();
        percent_decode(value)
    };
    let request = SignInRequest {
        redirect_url: param("redirect_uri"),
        state: param("state"),
        code_challenge: param("code_challenge"),
    };
    let kept = cookie_set(&started.raw_head, "__Host-cihub_sign_in").unwrap();
    let code = sim.authorize(person, &request);
    let cookie = format!("__Host-cihub_sign_in={kept}");
    let path = format!("/auth/github/callback?code={code}&state={}", request.state);

    assert_eq!(started.status, 303);
    assert_eq!(
        request.redirect_url,
        format!("{PUBLIC_URL}/auth/github/callback")
    );
    call_with(addr, "GET", &path, &[("Cookie", cookie.as_str())]).await
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_sign_in_creates_a_session_with_the_roles_that_the_scm_gives(pool: PgPool) {
    token(&pool, 9, &[]).await;
    let sim = Arc::new(Sim::new());
    let ada = ScmUser {
        external_id: "5".into(),
        login: "ada".into(),
    };
    let account = ScmAccount {
        external_id: "1".into(),
        path: "acme".into(),
    };

    sim.add_installation("1", account, &[]);
    sim.set_member("1", &ada, MembershipRole::Member);
    let addr = start_with(&pool, Arc::clone(&sim)).await;

    let callback = sign_in(&addr, &sim, &ada, "/orgs/acme").await;

    assert_eq!(callback.status, 303, "{}", callback.head);
    assert_eq!(
        header_values(&callback.raw_head, "location"),
        ["/orgs/acme"]
    );
    let session = cookie_set(&callback.raw_head, "__Host-cihub_session").unwrap();
    let csrf = cookie_set(&callback.raw_head, "__Host-cihub_csrf").unwrap();
    assert_eq!(csrf, csrf_token(&SecretString::from(session.clone())));
    assert_eq!(
        cookie_set(&callback.raw_head, "__Host-cihub_sign_in").as_deref(),
        Some("")
    );

    let cookie = format!("__Host-cihub_session={session}");
    let with_session = [("Cookie", cookie.as_str())];
    let me = call_with(&addr, "GET", "/api/v1/user", &with_session).await;
    let orgs = call_with(&addr, "GET", "/api/v1/user/orgs", &with_session).await;
    let instance = call_with(&addr, "GET", "/api/v1/instance", &with_session).await;
    let me: serde_json::Value = serde_json::from_str(&me.body).unwrap();
    let orgs: serde_json::Value = serde_json::from_str(&orgs.body).unwrap();
    let instance: serde_json::Value = serde_json::from_str(&instance.body).unwrap();

    assert_eq!(me["login"], "ada");
    assert_eq!(
        orgs["orgs"],
        serde_json::json!([{ "slug": "acme", "role": "member", "is_suspended": false }])
    );
    assert_eq!(instance["app_install_url"], "sim:install");
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_person_without_an_organization_gets_a_session_to_install_the_app(pool: PgPool) {
    let sim = Arc::new(Sim::new());
    let grace = ScmUser {
        external_id: "6".into(),
        login: "grace".into(),
    };
    let addr = start_with(&pool, Arc::clone(&sim)).await;

    let callback = sign_in(&addr, &sim, &grace, "/").await;

    assert_eq!(header_values(&callback.raw_head, "location"), ["/"]);
    let session = cookie_set(&callback.raw_head, "__Host-cihub_session").unwrap();
    let cookie = format!("__Host-cihub_session={session}");
    let with_session = [("Cookie", cookie.as_str())];
    let me = call_with(&addr, "GET", "/api/v1/user", &with_session).await;
    let orgs = call_with(&addr, "GET", "/api/v1/user/orgs", &with_session).await;
    let me: serde_json::Value = serde_json::from_str(&me.body).unwrap();
    let orgs: serde_json::Value = serde_json::from_str(&orgs.body).unwrap();

    assert_eq!(me["login"], "grace");
    assert_eq!(orgs["orgs"], serde_json::json!([]));
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_callback_that_is_not_the_one_of_the_sign_in_fails(pool: PgPool) {
    let sim = Arc::new(Sim::new());
    let addr = start_with(&pool, Arc::clone(&sim)).await;
    let started = call_with(&addr, "GET", "/auth/github", &[]).await;
    let kept = cookie_set(&started.raw_head, "__Host-cihub_sign_in").unwrap();
    let cookie = format!("__Host-cihub_sign_in={kept}");

    let without_sign_in =
        call_with(&addr, "GET", "/auth/github/callback?code=c&state=s", &[]).await;
    let other_state = call_with(
        &addr,
        "GET",
        "/auth/github/callback?code=c&state=forged",
        &[("Cookie", cookie.as_str())],
    )
    .await;

    for failed in [without_sign_in, other_state] {
        assert_eq!(failed.status, 303);
        assert_eq!(
            header_values(&failed.raw_head, "location"),
            ["/sign-in?error=failed"]
        );
        assert!(cookie_set(&failed.raw_head, "__Host-cihub_session").is_none());
    }
}

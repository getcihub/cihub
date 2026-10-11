//! The server of the browser tests of the dashboard (`mise run e2e`): the API,
//! the dashboard of `web/dist`, the sign-in, and the worker, the router and
//! the scheduler of the queues, on the simulator and on a fresh database
//! `cihub_e2e`, next to the one of `DATABASE_URL`. Its sign-in page signs
//! anyone in: never a server of production.
//!
//! The world: `acme` and `globex`, a queue each; `ada` is a member of `acme`,
//! `grace` of no organization. On `acme/web`, `ada` submits three PRs with
//! `/cihub merge`: two whose check passed, which the queue tests, and one
//! whose check waits.

use std::collections::HashMap;
use std::fmt::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::extract::{Query, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::get;
use cihub_domain::{
    CheckConclusion, CheckKind, CheckReport, ConnectionId, MembershipRole, OrgId, Permission,
    QueueId, RepoId,
};
use cihub_platform::{Delivery, Executors, find_route, run_router, run_worker, store_delivery};
use cihub_scm::sim::{PrChange, Sim};
use cihub_scm::{
    ConnectionRef, RepoRef, RequiredChecks, ScmAccount, ScmDelivery, ScmError, ScmUser,
    SignInRequest,
};
use cihub_store::{MIGRATOR, PgPool, begin_system, create_event_partitions, migrate};
use cihub_web::{Dashboard, GithubWebhooks};
use secrecy::SecretString;
use serde_json::Value;
use sqlx::AssertSqlSafe;
use time::OffsetDateTime;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tracing::warn;
use ulid::Ulid;

const ADDR: &str = "127.0.0.1:18090";
const PUBLIC_URL: &str = "http://127.0.0.1:18090";
const DATABASE: &str = "cihub_e2e";
const REQUIRED: &str = "ci";

/// The deliveries of the simulator, by the identifier that their stored
/// payload holds.
type Deliveries = Arc<Mutex<HashMap<String, ScmDelivery>>>;

fn now() -> OffsetDateTime {
    #[expect(
        clippy::disallowed_methods,
        reason = "the clock of a server for the browser tests"
    )]
    OffsetDateTime::now_utc()
}

fn person(id: &str, login: &str) -> ScmUser {
    ScmUser {
        external_id: id.into(),
        login: login.into(),
    }
}

fn people() -> [(ScmUser, &'static str); 2] {
    [
        (person("1", "ada"), "member of acme"),
        (person("2", "grace"), "no organization"),
    ]
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    cihub_telemetry::Logs::new("warn", cihub_telemetry::LogFormat::Pretty)?.init()?;
    let pool = fresh_database().await?;
    let sim = Arc::new(Sim::new());

    seed(&pool).await?;
    world(&sim)?;
    sim.serve_sign_in_at(&format!("{PUBLIC_URL}/sim/authorize"));

    let deliveries = Deliveries::default();
    let module = cihub_mergequeue::module(
        pool.clone(),
        sim.clone(),
        translate(&deliveries),
        "e2e".to_owned(),
        Arc::new(now),
    );
    let mut executors = Executors::default();
    (module.executors)(&mut executors);

    let cancel = CancellationToken::new();
    let idle = Duration::from_millis(100);
    let worker = run_worker(
        pool.clone(),
        "e2e".into(),
        Arc::new(executors),
        now,
        idle,
        cancel.clone(),
    );
    let router_deliveries = deliveries.clone();
    let router = run_router(
        pool.clone(),
        move |_: &str, payload: &Value| {
            stored_delivery(&router_deliveries, payload).ok_or(std::fmt::Error)
        },
        now,
        idle,
        cancel.clone(),
    );
    let scheduler = (module.scheduler)(cancel.clone());
    tokio::spawn(async move {
        if let Err(err) = worker.await {
            warn!(error = &err as &dyn std::error::Error, "worker stopped");
        }
    });
    tokio::spawn(async move {
        if let Err(err) = router.await {
            warn!(error = &err as &dyn std::error::Error, "router stopped");
        }
    });
    tokio::spawn(async move {
        if let Err(err) = scheduler.await {
            warn!(error = &*err as &dyn std::error::Error, "scheduler stopped");
        }
    });
    tokio::spawn(pass_deliveries(
        pool.clone(),
        sim.clone(),
        deliveries,
        cancel.clone(),
    ));
    submit_prs(&sim)?;

    let webhooks = GithubWebhooks {
        secret: SecretString::from("e2e"),
    };
    let dashboard = Dashboard {
        scm: sim.clone(),
        public_url: Some(PUBLIC_URL.to_owned()),
    };
    let sign_in_page = Router::new()
        .route("/sim/authorize", get(authorize_page))
        .route("/sim/sign-in", get(sign_in_as))
        .with_state(sim);
    let router =
        cihub_web::router(pool, now, webhooks, vec![module.api], dashboard).merge(sign_in_page);
    let listener = TcpListener::bind(ADDR).await?;

    cihub_web::serve(listener, router, cancel).await?;
    Ok(())
}

/// Drops and creates `cihub_e2e` on the server of `DATABASE_URL`, then applies
/// the migrations and creates the partitions, as `cihub server migrate` does.
async fn fresh_database() -> Result<PgPool, Box<dyn std::error::Error>> {
    let url = std::env::var("DATABASE_URL")?;
    let admin = PgPool::connect(&url).await?;

    sqlx::query(AssertSqlSafe(format!(
        "DROP DATABASE IF EXISTS {DATABASE} WITH (FORCE)"
    )))
    .execute(&admin)
    .await?;
    sqlx::query(AssertSqlSafe(format!("CREATE DATABASE {DATABASE}")))
        .execute(&admin)
        .await?;

    let (server, _) = url.rsplit_once('/').ok_or("DATABASE_URL has no database")?;
    let pool = PgPool::connect(&format!("{server}/{DATABASE}")).await?;
    migrate(&MIGRATOR, &pool).await?;
    create_event_partitions(&pool, now().date()).await?;
    Ok(pool)
}

/// `acme` and `globex`, with a connection, a repository and a queue each.
async fn seed(pool: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
    let mut tx = begin_system(pool).await?;
    for (n, slug, repo) in [(1u128, "acme", "web"), (2, "globex", "api")] {
        let org = OrgId::from_ulid(Ulid(n));
        let connection = ConnectionId::from_ulid(Ulid(n));
        let repo_id = RepoId::from_ulid(Ulid(n));
        let queue = QueueId::from_ulid(Ulid(n));
        sqlx::raw_sql(AssertSqlSafe(format!(
            "INSERT INTO orgs (id, slug) VALUES ('{org}', '{slug}');
             INSERT INTO scm_connections (id, org_id, account_external_id, kind, external_id,
                                          synced_at, permissions)
                 VALUES ('{connection}', '{org}', '{n}0', 'github_app', '{n}00', now(), '{{}}');
             INSERT INTO repos (id, org_id, connection_id, external_id, namespace, name,
                                default_branch)
                 VALUES ('{repo_id}', '{org}', '{connection}', '{n}000', '{slug}', '{repo}', 'main');
             INSERT INTO queues (id, org_id, repo_id, target_branch, name)
                 VALUES ('{queue}', '{org}', '{repo_id}', 'main', 'main');"
        )))
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// The repository `acme/web`, as the seed and the simulator name it.
fn acme_web() -> RepoRef {
    RepoRef {
        connection: ConnectionRef {
            id: ConnectionId::from_ulid(Ulid(1)),
            external_id: "100".into(),
        },
        external_id: "1000".into(),
        namespace: "acme".into(),
        name: "web".into(),
    }
}

/// The installations of the two organizations on the simulator, who is a
/// member of which, and `acme/web`, whose branch `main` requires `ci`.
fn world(sim: &Sim) -> Result<(), ScmError> {
    let [(ada, _), _] = people();
    for (installation, account, slug) in [("100", "10", "acme"), ("200", "20", "globex")] {
        let account = ScmAccount {
            external_id: account.into(),
            path: slug.into(),
        };
        sim.add_installation(installation, account, &[("contents", "write")]);
    }
    sim.set_member("100", &ada, MembershipRole::Member);

    let web = acme_web();
    sim.add_repo(&web, "main", &[("README.md", "web")]);
    let rules = RequiredChecks {
        names: [REQUIRED.to_owned()].into(),
        up_to_date: false,
    };
    sim.set_rules(&web, "main", rules)?;
    sim.set_permission(&web, &ada, Permission::Write)
}

/// Opens the PRs of `acme/web`, and `ada` submits each one with a comment.
fn submit_prs(sim: &Sim) -> Result<(), Box<dyn std::error::Error>> {
    let [(ada, _), _] = people();
    let web = acme_web();
    let prs = [
        (
            "export-csv",
            "Export invoices as CSV",
            person("11", "marie"),
            true,
        ),
        (
            "login-redirect",
            "Fix the login redirect",
            person("12", "lea"),
            true,
        ),
        (
            "install-guide",
            "Update the install guide",
            person("13", "alex"),
            false,
        ),
    ];

    for (branch, title, author, is_green) in prs {
        let main = sim.branch(&web, "main").ok_or("acme/web has no main")?;
        let file = format!("{branch}.txt");
        let head = sim.commit(&web, &main, &[(&file, Some(title))])?;
        sim.push(&web, branch, &head, &author)?;
        let number = sim.open_pr(&web, branch, "main", &author)?;
        sim.change_pr(&web, number, PrChange::Title(title))?;
        if is_green {
            let check = CheckReport {
                name: REQUIRED.into(),
                kind: CheckKind::Status,
                conclusion: Some(CheckConclusion::Success),
                details_url: None,
            };
            sim.report_check(&web, &head, check)?;
        }
        sim.comment(&web, number, &ada, "/cihub merge")?;
    }

    Ok(())
}

fn stored_delivery(deliveries: &Deliveries, payload: &Value) -> Option<ScmDelivery> {
    let id = payload["id"].as_str()?;
    deliveries.lock().ok()?.get(id).cloned()
}

/// The translation of a stored delivery of the simulator: its payload names
/// it.
fn translate(deliveries: &Deliveries) -> cihub_mergequeue::Translate {
    let deliveries = deliveries.clone();
    Arc::new(move |_, payload| {
        stored_delivery(&deliveries, payload).ok_or_else(|| "unknown delivery".into())
    })
}

/// Stores the deliveries of the simulator as the web role does, as they come,
/// until `cancel`.
async fn pass_deliveries(
    pool: PgPool,
    sim: Arc<Sim>,
    deliveries: Deliveries,
    cancel: CancellationToken,
) {
    let mut sent = 0;

    while !cancel.is_cancelled() {
        tokio::time::sleep(Duration::from_millis(100)).await;
        for delivery in sim.take_deliveries() {
            sent += 1;
            if let Err(err) = store(&pool, &deliveries, format!("delivery-{sent}"), delivery).await
            {
                warn!(
                    error = &*err as &dyn std::error::Error,
                    "delivery not stored"
                );
            }
        }
    }
}

async fn store(
    pool: &PgPool,
    deliveries: &Deliveries,
    id: String,
    delivery: ScmDelivery,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut tx = begin_system(pool).await?;
    let route = find_route(&mut tx, &delivery).await?;
    deliveries
        .lock()
        .map_err(|_| "deliveries poisoned")?
        .insert(id.clone(), delivery);
    let stored = Delivery {
        delivery_id: id.clone(),
        org_id: route.org_id,
        connection_id: route.connection_id,
        queue_id: route.queue_id,
        event: "sim".into(),
        payload: serde_json::json!({ "id": id }),
        received_at: now(),
    };

    store_delivery(&mut tx, &stored).await?;
    tx.commit().await?;

    Ok(())
}

/// The page of the simulator where a person signs in: one link per person,
/// and one that refuses.
async fn authorize_page(Query(query): Query<HashMap<String, String>>) -> Html<String> {
    let encoded = |name: &str| encode(query.get(name).map_or("", String::as_str));
    let request = format!(
        "redirect_uri={}&state={}&code_challenge={}",
        encoded("redirect_uri"),
        encoded("state"),
        encoded("code_challenge")
    );
    let links = people()
        .iter()
        .fold(String::new(), |mut links, (person, about)| {
            let _ = write!(
                links,
                "<li><a href=\"/sim/sign-in?login={}&amp;{}\">Sign in as {} ({about})</a></li>",
                person.login,
                request.replace('&', "&amp;"),
                person.login
            );
            links
        });
    let refuse = format!(
        "{}?error=access_denied&amp;state={}",
        query.get("redirect_uri").map_or("", String::as_str),
        encoded("state")
    );
    Html(format!(
        "<!doctype html><title>Simulator</title><h1>Sign in to the simulator</h1>\
         <ul>{links}<li><a href=\"{refuse}\">Refuse</a></li></ul>"
    ))
}

/// Signs the person in on the simulator and sends them back to CIHub with the
/// code, as the SCM does after the consent of the person.
async fn sign_in_as(
    State(sim): State<Arc<Sim>>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let get = |name: &str| query.get(name).cloned().unwrap_or_default();
    let login = get("login");
    let Some((person, _)) = people()
        .into_iter()
        .find(|(person, _)| person.login == login)
    else {
        return Html("no such person").into_response();
    };

    let request = SignInRequest {
        redirect_url: get("redirect_uri"),
        state: get("state"),
        code_challenge: get("code_challenge"),
        asks_for_account: false,
    };
    let code = sim.authorize(&person, &request);
    Redirect::to(&format!(
        "{}?code={}&state={}",
        request.redirect_url,
        encode(&code),
        encode(&request.state)
    ))
    .into_response()
}

fn encode(text: &str) -> String {
    text.bytes().fold(String::new(), |mut encoded, byte| {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            let _ = write!(encoded, "%{byte:02X}");
        }
        encoded
    })
}

//! The server of the browser tests of the dashboard (`mise run e2e`, lot 10a):
//! the API, the dashboard of `web/dist` and the sign-in, on the simulator and
//! on a fresh database `cihub_e2e`, next to the one of `DATABASE_URL`. Its
//! sign-in page signs anyone in: never a server of production.
//!
//! The world: `acme` and `globex`, a queue each; `ada` is a member of `acme`,
//! `grace` of no organization.

use std::collections::HashMap;
use std::fmt::Write;
use std::sync::Arc;

use axum::Router;
use axum::extract::{Query, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::get;
use cihub_domain::{ConnectionId, MembershipRole, OrgId, QueueId, RepoId};
use cihub_scm::sim::Sim;
use cihub_scm::{ScmAccount, ScmUser, SignInRequest};
use cihub_store::{MIGRATOR, PgPool, begin_system, migrate};
use cihub_web::{Dashboard, GithubWebhooks};
use secrecy::SecretString;
use sqlx::AssertSqlSafe;
use time::OffsetDateTime;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use ulid::Ulid;

const ADDR: &str = "127.0.0.1:18090";
const PUBLIC_URL: &str = "http://127.0.0.1:18090";
const DATABASE: &str = "cihub_e2e";

fn now() -> OffsetDateTime {
    #[expect(
        clippy::disallowed_methods,
        reason = "the clock of a server for the browser tests"
    )]
    OffsetDateTime::now_utc()
}

fn people() -> [(ScmUser, &'static str); 2] {
    let person = |id: &str, login: &str| ScmUser {
        external_id: id.into(),
        login: login.into(),
    };
    [
        (person("1", "ada"), "member of acme"),
        (person("2", "grace"), "no organization"),
    ]
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = fresh_database().await?;
    let sim = Arc::new(Sim::new());

    seed(&pool).await?;
    world(&sim);
    sim.serve_sign_in_at(&format!("{PUBLIC_URL}/sim/authorize"));

    let translate: cihub_mergequeue::Translate = Arc::new(|event, payload| {
        cihub_github::translate(event, payload)
            .map_err(|err| Box::new(err) as Box<dyn std::error::Error + Send + Sync>)
    });
    let module = cihub_mergequeue::module(
        pool.clone(),
        sim.clone(),
        translate,
        "e2e".to_owned(),
        Arc::new(now),
    );
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

    cihub_web::serve(listener, router, CancellationToken::new()).await?;
    Ok(())
}

/// Drops and creates `cihub_e2e` on the server of `DATABASE_URL`, then applies
/// the migrations.
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

/// The installations of the two organizations on the simulator, and who is a
/// member of which.
fn world(sim: &Sim) {
    let [(ada, _), _] = people();
    for (installation, account, slug) in [("100", "10", "acme"), ("200", "20", "globex")] {
        let account = ScmAccount {
            external_id: account.into(),
            path: slug.into(),
        };
        sim.add_installation(installation, account, &[]);
    }
    sim.set_member("100", &ada, MembershipRole::Member);
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

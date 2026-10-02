//! `cihub-github` against recorded answers of GitHub (`AGENTS.md` D74, D90):
//! the contract suite of the `Scm` trait, and the pause after a rate limit.
//! `test-only-key.pem` was made for these tests and opens nothing: the
//! replay does not check the JWT.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code: a panic is a failed test"
)]

mod support;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration as StdDuration;

use cihub_domain::{ConnectionId, JobClass, Permission};
use cihub_github::{GithubApp, GithubConfig};
use cihub_scm::contract::{self, Fixture};
use cihub_scm::{
    ConnectionRef, RecentDelivery, RepoRef, RequiredChecks, Scm, ScmAccount, ScmError, ScmRepo,
    ScmUser, SetupFinding,
};
use secrecy::SecretString;
use serde_json::json;
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};
use ulid::Ulid;

use support::cassette::{Cassette, Exchange, RECORDED_TOKEN};
use support::replay::Replay;

const TEST_KEY: &str = include_str!("test-only-key.pem");

fn app(url: &str, now: OffsetDateTime) -> GithubApp {
    let config = GithubConfig {
        url: url.to_owned(),
        app_id: "1".to_owned(),
        private_key: SecretString::from(TEST_KEY),
    };
    GithubApp::new(&config, Arc::new(move || now)).unwrap()
}

fn connection() -> ConnectionId {
    ConnectionId::from_ulid(Ulid(1))
}

fn repo(installation: &str, namespace: &str, name: &str) -> RepoRef {
    RepoRef {
        connection: ConnectionRef {
            id: connection(),
            external_id: installation.to_owned(),
        },
        external_id: String::new(),
        namespace: namespace.to_owned(),
        name: name.to_owned(),
    }
}

#[tokio::test]
async fn the_contract_passes_on_recorded_answers() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/recorded/contract.json");
    let cassette: Cassette = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let world = cassette.world;
    let replay = Replay::start(cassette.exchanges).await;
    let now = OffsetDateTime::from_unix_timestamp(cassette.recorded_at).unwrap();
    let fixture = Fixture {
        base_branch: world.base_branch,
        required: RequiredChecks {
            names: world.required_checks,
            up_to_date: world.up_to_date,
        },
        file: (world.file_path, world.file_content),
        mergeable_pr: world.mergeable_pr,
        conflicting_pr: world.conflicting_pr,
        draft_pr: world.draft_pr,
        missing_pr: world.missing_pr,
        admin: ScmUser {
            external_id: String::new(),
            login: world.admin,
        },
        work_branch: world.work_branch,
    };
    let repo = repo(&world.installation, &world.namespace, &world.name);
    contract::run(&app(&replay.url, now), &repo, &fixture).await;
    replay.finish();
}

fn token(installation: &str, now: OffsetDateTime) -> Exchange {
    let expires_at = (now + Duration::hours(1)).format(&Rfc3339).unwrap();
    Exchange {
        method: "POST".into(),
        path: format!("/app/installations/{installation}/access_tokens"),
        request: None,
        status: 201,
        headers: BTreeMap::new(),
        body: Some(json!({ "token": RECORDED_TOKEN, "expires_at": expires_at })),
        text: None,
    }
}

#[tokio::test]
async fn a_rate_limit_pauses_the_connection_until_its_reset() {
    let now = OffsetDateTime::from_unix_timestamp(1_789_000_000).unwrap();
    let reset = now + Duration::minutes(15);
    let limited = Exchange {
        method: "GET".into(),
        path: "/repos/acme/app/git/ref/heads/main".into(),
        request: None,
        status: 429,
        headers: BTreeMap::from([
            ("x-ratelimit-limit".into(), "5000".into()),
            ("x-ratelimit-remaining".into(), "0".into()),
            (
                "x-ratelimit-reset".into(),
                reset.unix_timestamp().to_string(),
            ),
        ]),
        body: Some(json!({ "message": "API rate limit exceeded" })),
        text: None,
    };
    let replay = Replay::start(vec![token("7", now), limited]).await;
    let app = app(&replay.url, now);
    let urgent = app.budget_urgent();
    let repo = repo("7", "acme", "app");

    let read = app.branch_head(&repo, "main", JobClass::Merge).await;
    assert_eq!(read, Err(ScmError::RateLimited { until: reset }));
    tokio::time::timeout(StdDuration::from_secs(1), urgent.notified())
        .await
        .expect("a pause is written at once");
    let changed = app.take_changed_budgets();
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].0, connection());
    assert_eq!(changed[0].1.paused_until, Some(reset));
    // Until the reset, no call goes out: the replay has no answer left.
    let again = app.branch_head(&repo, "main", JobClass::Interactive).await;
    assert_eq!(again, Err(ScmError::RateLimited { until: reset }));
    replay.finish();
}

fn page(path: &str, attempts: serde_json::Value, next: Option<&str>) -> Exchange {
    let headers = next
        .map(|next| {
            let link = format!("<https://api.github.com{next}>; rel=\"next\"");
            BTreeMap::from([("link".to_owned(), link)])
        })
        .unwrap_or_default();
    Exchange {
        method: "GET".into(),
        path: path.into(),
        request: None,
        status: 200,
        headers,
        body: Some(attempts),
        text: None,
    }
}

#[tokio::test]
async fn the_recent_deliveries_stop_at_the_first_older_attempt() {
    let now = OffsetDateTime::from_unix_timestamp(1_789_000_000).unwrap();
    let ago = |minutes| (now - Duration::minutes(minutes)).format(&Rfc3339).unwrap();
    let first = "/app/hook/deliveries?per_page=100";
    let second = "/app/hook/deliveries?per_page=100&cursor=c2";
    let exchanges = vec![
        page(
            first,
            json!([
                { "id": 3, "guid": "g2", "delivered_at": ago(1) },
                { "id": 2, "guid": "g1", "delivered_at": ago(2) },
            ]),
            Some(second),
        ),
        page(
            second,
            json!([{ "id": 1, "guid": "g0", "delivered_at": ago(20) }]),
            Some("/app/hook/deliveries?per_page=100&cursor=c3"),
        ),
        Exchange {
            method: "POST".into(),
            path: "/app/hook/deliveries/2/attempts".into(),
            request: None,
            status: 202,
            headers: BTreeMap::new(),
            body: Some(json!({})),
            text: None,
        },
    ];
    let replay = Replay::start(exchanges).await;
    let app = app(&replay.url, now);
    let since = now - Duration::minutes(10);
    let recent = app
        .recent_deliveries(since, JobClass::Repair)
        .await
        .unwrap();
    let attempt = |delivery_id: &str, attempt_id: &str| RecentDelivery {
        delivery_id: delivery_id.into(),
        attempt_id: attempt_id.into(),
    };
    assert_eq!(recent, [attempt("g2", "3"), attempt("g1", "2")]);
    app.redeliver("2", JobClass::Repair).await.unwrap();
    replay.finish();
}

fn answer(method: &str, path: &str, status: u16, body: serde_json::Value) -> Exchange {
    Exchange {
        method: method.into(),
        path: path.into(),
        request: None,
        status,
        headers: BTreeMap::new(),
        body: Some(body),
        text: None,
    }
}

#[tokio::test]
async fn an_installation_gives_its_account_and_every_page_of_its_repositories() {
    let now = OffsetDateTime::from_unix_timestamp(1_789_000_000).unwrap();
    let repository = |id: u64, name: &str, branch: &str| json!({ "id": id, "name": name, "owner": { "login": "acme" }, "default_branch": branch });
    let exchanges = vec![
        answer(
            "GET",
            "/app/installations/7",
            200,
            json!({ "id": 7, "account": { "id": 70, "login": "acme" } }),
        ),
        answer(
            "GET",
            "/app/installations/8",
            404,
            json!({ "message": "Not Found" }),
        ),
        token("7", now),
        answer(
            "GET",
            "/installation/repositories?per_page=100&page=1",
            200,
            json!({ "total_count": 3, "repositories": [
                repository(1, "app", "main"),
                repository(2, "lib", "trunk"),
            ]}),
        ),
        answer(
            "GET",
            "/installation/repositories?per_page=100&page=2",
            200,
            json!({ "total_count": 3, "repositories": [repository(3, "docs", "main")] }),
        ),
    ];
    let replay = Replay::start(exchanges).await;
    let app = app(&replay.url, now);

    let account = app.installation("7", JobClass::Interactive).await.unwrap();
    assert_eq!(
        account,
        ScmAccount {
            external_id: "70".into(),
            path: "acme".into(),
        }
    );
    let unknown = app.installation("8", JobClass::Interactive).await;
    assert!(matches!(unknown, Err(ScmError::NotFound(_))), "{unknown:?}");

    let connection = repo("7", "acme", "app").connection;
    let repos = app.list_repos(&connection, JobClass::Repair).await.unwrap();
    let listed = |id: &str, name: &str, branch: &str| ScmRepo {
        external_id: id.into(),
        namespace: "acme".into(),
        name: name.into(),
        default_branch: branch.into(),
    };
    assert_eq!(
        repos,
        [
            listed("1", "app", "main"),
            listed("2", "lib", "trunk"),
            listed("3", "docs", "main"),
        ]
    );
    replay.finish();
}

#[tokio::test]
async fn a_permission_is_kept_five_minutes() {
    let now = OffsetDateTime::from_unix_timestamp(1_789_000_000).unwrap();
    let path = "/repos/acme/app/collaborators/alice/permission";
    let exchanges = vec![
        token("7", now),
        answer("GET", path, 200, json!({ "permission": "write" })),
        answer("GET", path, 200, json!({ "permission": "read" })),
    ];
    let replay = Replay::start(exchanges).await;
    let clock = Arc::new(std::sync::Mutex::new(now));
    let at = Arc::clone(&clock);
    let config = GithubConfig {
        url: replay.url.clone(),
        app_id: "1".to_owned(),
        private_key: SecretString::from(TEST_KEY),
    };
    let app = GithubApp::new(&config, Arc::new(move || *at.lock().unwrap())).unwrap();
    let repo = repo("7", "acme", "app");
    let alice = ScmUser {
        external_id: "21".into(),
        login: "alice".into(),
    };
    let read = || app.permission(&repo, &alice, JobClass::Interactive);
    assert_eq!(read().await.unwrap(), Permission::Write);
    *clock.lock().unwrap() = now + Duration::minutes(4);
    assert_eq!(read().await.unwrap(), Permission::Write, "kept");
    *clock.lock().unwrap() = now + Duration::minutes(5);
    assert_eq!(read().await.unwrap(), Permission::Read, "read again");
    replay.finish();
}

#[tokio::test]
async fn the_setup_check_reads_the_rulesets_of_the_branch_and_of_the_test_branches() {
    let now = OffsetDateTime::from_unix_timestamp(1_789_000_000).unwrap();
    let rules = "/repos/acme/app/rules/branches";
    let exchanges = vec![
        token("7", now),
        answer(
            "GET",
            &format!("{rules}/main?per_page=100"),
            200,
            json!([{ "type": "update", "ruleset_id": 5 }, { "type": "pull_request", "ruleset_id": 6 }]),
        ),
        // The App of the tests has the identifier 1: the ruleset exempts it.
        answer(
            "GET",
            "/repos/acme/app/rulesets/5",
            200,
            json!({ "bypass_actors": [
                { "actor_id": 1, "actor_type": "Integration", "bypass_mode": "exempt" }
            ]}),
        ),
        answer(
            "GET",
            &format!("{rules}/cihub-temp/check/1?per_page=100"),
            200,
            json!([]),
        ),
        answer(
            "GET",
            &format!("{rules}/cihub-merge/check/1?per_page=100"),
            200,
            json!([{ "type": "pull_request", "ruleset_id": 6 }]),
        ),
        answer(
            "GET",
            &format!("{rules}/main"),
            200,
            json!([{ "type": "required_status_checks", "ruleset_id": 6, "parameters": {
                "strict_required_status_checks_policy": true,
                "required_status_checks": [{ "context": "ci" }]
            }}]),
        ),
        answer(
            "GET",
            "/repos/acme/app/branches/main/protection/required_status_checks",
            404,
            json!({ "message": "Not Found" }),
        ),
    ];
    let replay = Replay::start(exchanges).await;
    let app = app(&replay.url, now);
    let findings = app
        .check_setup(&repo("7", "acme", "app"), "main", JobClass::Interactive)
        .await
        .unwrap();
    assert_eq!(
        findings,
        [
            SetupFinding::TestBranchesNotExcluded,
            SetupFinding::UpToDateRequired
        ]
    );
    replay.finish();
}

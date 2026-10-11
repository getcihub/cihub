//! `cihub-github` against recorded answers of GitHub: the contract suite of the
//! `Scm` trait, and the pause after a rate limit.
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

use cihub_domain::{ConnectionId, JobClass, MembershipRole, MergeMethod, Permission};
use cihub_github::{GithubApp, GithubConfig, SignInClient};
use cihub_scm::contract;
use cihub_scm::{
    ConnectionRef, PrMerge, RecentDelivery, RepoRef, Scm, ScmAccount, ScmError, ScmInstallation,
    ScmMember, ScmMembership, ScmRepo, ScmUser, SetupFinding, SignInCode, SignInRequest,
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
        sign_in: None,
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
    let fixture = world.fixture();
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
                { "id": 3, "guid": "g2", "delivered_at": ago(1), "status_code": 202 },
                { "id": 2, "guid": "g1", "delivered_at": ago(2), "status_code": 503 },
            ]),
            Some(second),
        ),
        page(
            second,
            json!([{ "id": 1, "guid": "g0", "delivered_at": ago(20), "status_code": 202 }]),
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
    let attempt = |delivery_id: &str, attempt_id: &str, is_delivered| RecentDelivery {
        delivery_id: delivery_id.into(),
        attempt_id: attempt_id.into(),
        is_delivered,
    };

    assert_eq!(
        recent,
        [attempt("g2", "3", true), attempt("g1", "2", false)]
    );

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

/// Written by hand from the documentation of GitHub, not recorded: the
/// recording by the `github` example needs a test repository on GitHub.
#[tokio::test]
async fn acknowledging_a_comment_twice_is_one_reaction() {
    let now = OffsetDateTime::from_unix_timestamp(1_789_000_000).unwrap();
    let reaction = json!({ "id": 1, "content": "+1" });
    let path = "/repos/acme/app/issues/comments/40/reactions";
    let exchanges = vec![
        token("7", now),
        answer("POST", path, 201, reaction.clone()),
        answer("POST", path, 200, reaction),
    ];
    let replay = Replay::start(exchanges).await;
    let app = app(&replay.url, now);
    let repo = repo("7", "acme", "app");
    for _ in 0..2 {
        app.acknowledge_comment(&repo, "40", JobClass::Interactive)
            .await
            .unwrap();
    }

    replay.finish();
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
            json!({
                "id": 7,
                "account": { "id": 70, "login": "acme" },
                "permissions": { "contents": "write", "metadata": "read" },
                "suspended_at": null,
            }),
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

    let installation = app.installation("7", JobClass::Interactive).await.unwrap();

    assert_eq!(installation, Some(acme_installation()));
    let unknown = app.installation("8", JobClass::Interactive).await;

    assert_eq!(unknown, Ok(None));

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
        sign_in: None,
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

/// Written by hand from the documentation of GitHub, not recorded: GitHub
/// refuses a stacked PR on the merge endpoint and names its asynchronous one.
#[tokio::test]
async fn a_stacked_pr_merges_through_the_asynchronous_endpoint() {
    let now = OffsetDateTime::from_unix_timestamp(1_789_000_000).unwrap();
    let merge_path = "/repos/acme/app/pulls/7/merge";
    let async_path = "/repos/acme/app/pulls/7/merge-async";
    let stacked = || {
        answer(
            "PUT",
            merge_path,
            403,
            json!({ "message": "Merging stacked PRs via this endpoint is not supported. Use the asynchronous merge endpoint instead." }),
        )
    };
    let pending = json!({ "status": "pending", "details": {
        "message": "Merge request accepted", "uuid": "550e8400-e29b-41d4-a716-446655440000",
        "merge_method": "squash", "merge_action": "direct_merge", "expected_head_sha": "h1" } });
    let exchanges = vec![
        token("7", now),
        stacked(),
        answer("PUT", async_path, 202, pending.clone()),
        stacked(),
        answer("PUT", async_path, 409, pending),
        stacked(),
        answer(
            "PUT",
            async_path,
            200,
            json!({ "status": "merged", "details": { "message": "Pull request merged", "sha": "m1" } }),
        ),
        stacked(),
        answer(
            "PUT",
            async_path,
            202,
            json!({ "status": "failed", "details": { "message": "Merge conflict" } }),
        ),
        stacked(),
        answer(
            "PUT",
            async_path,
            400,
            json!({ "message": "Pull request is closed" }),
        ),
    ];
    let replay = Replay::start(exchanges).await;
    let app = app(&replay.url, now);
    let repo = repo("7", "acme", "app");
    let merge = PrMerge {
        number: 7,
        head_sha: "h1".into(),
        tested_sha: "t1".into(),
        method: MergeMethod::Squash,
        title: "feat: x (#7)".into(),
        message: String::new(),
    };
    let mut results = Vec::new();
    for _ in 0..5 {
        results.push(app.merge_pr(&repo, &merge, JobClass::Merge).await);
    }

    assert!(
        matches!(results[0], Err(ScmError::MergeInProgress)),
        "{results:?}"
    );
    assert!(
        matches!(results[1], Err(ScmError::MergeInProgress)),
        "{results:?}"
    );
    assert_eq!(results[2].as_deref().ok(), Some("m1"));
    assert!(
        matches!(&results[3], Err(ScmError::MergeRejected(m)) if m == "Merge conflict"),
        "{results:?}"
    );
    assert!(
        matches!(&results[4], Err(ScmError::MergeRejected(m)) if m.contains("closed")),
        "{results:?}"
    );

    replay.finish();
}

fn acme_installation() -> ScmInstallation {
    ScmInstallation {
        external_id: "7".into(),
        account: ScmAccount {
            external_id: "70".into(),
            path: "acme".into(),
        },
        permissions: BTreeMap::from([
            ("contents".to_owned(), "write".to_owned()),
            ("metadata".to_owned(), "read".to_owned()),
        ]),
        is_suspended: false,
    }
}

/// Written by hand from the documentation of GitHub, not recorded: a
/// recording by the `github` example has one page of installations only.
#[tokio::test]
async fn the_installations_of_the_app_come_from_every_page() {
    let now = OffsetDateTime::from_unix_timestamp(1_789_000_000).unwrap();
    let first = "/app/installations?per_page=100";
    let second = "/app/installations?per_page=100&page=2";
    let acme = json!({
        "id": 7,
        "account": { "id": 70, "login": "acme" },
        "permissions": { "contents": "write", "metadata": "read" },
        "suspended_at": null,
    });
    let globex = json!({
        "id": 9,
        "account": { "id": 90, "login": "globex" },
        "permissions": {},
        "suspended_at": "2026-09-01T10:00:00Z",
    });
    let exchanges = vec![
        page(first, json!([acme]), Some(second)),
        page(second, json!([globex]), None),
    ];
    let replay = Replay::start(exchanges).await;
    let app = app(&replay.url, now);

    let installations = app.installations(JobClass::Repair).await.unwrap();
    let globex = ScmInstallation {
        external_id: "9".into(),
        account: ScmAccount {
            external_id: "90".into(),
            path: "globex".into(),
        },
        permissions: BTreeMap::new(),
        is_suspended: true,
    };

    assert_eq!(installations, [acme_installation(), globex]);

    replay.finish();
}

fn app_with_sign_in(url: &str, now: OffsetDateTime) -> GithubApp {
    let config = GithubConfig {
        url: url.to_owned(),
        app_id: "1".to_owned(),
        private_key: SecretString::from(TEST_KEY),
        sign_in: Some(SignInClient {
            client_id: "Iv1.client".to_owned(),
            client_secret: SecretString::from("secret"),
        }),
    };
    GithubApp::new(&config, Arc::new(move || now)).unwrap()
}

fn sign_in_code() -> SignInCode {
    SignInCode {
        code: SecretString::from("code"),
        code_verifier: SecretString::from("verifier"),
        redirect_url: "https://ci.example.com/auth/github/callback".into(),
    }
}

/// Written by hand from the documentation of GitHub, not recorded: a
/// sign-in needs the consent of a person in a browser.
#[tokio::test]
async fn a_sign_in_gives_the_role_of_the_person_in_each_installation() {
    let now = OffsetDateTime::from_unix_timestamp(1_789_000_000).unwrap();
    let installation = |id: u64, account: u64, login: &str, kind: &str| json!({ "id": id, "account": { "id": account, "login": login, "type": kind } });
    let installations = json!({
        "total_count": 3,
        "installations": [
            installation(7, 70, "acme", "Organization"),
            installation(8, 5, "octocat", "User"),
            installation(9, 90, "globex", "Organization"),
        ],
    });
    let exchanges = vec![
        answer(
            "POST",
            "/login/oauth/access_token",
            200,
            json!({ "access_token": "ghu_person", "token_type": "bearer" }),
        ),
        answer("GET", "/user", 200, json!({ "id": 5, "login": "octocat" })),
        answer(
            "GET",
            "/user/installations?per_page=100",
            200,
            installations,
        ),
        answer(
            "GET",
            "/user/memberships/orgs/acme",
            200,
            json!({ "state": "active", "role": "admin" }),
        ),
        answer(
            "GET",
            "/user/memberships/orgs/globex",
            404,
            json!({ "message": "Not Found" }),
        ),
    ];
    let replay = Replay::start(exchanges).await;
    let app = app_with_sign_in(&replay.url, now);
    let membership = |installation: &str| ScmMembership {
        connection_external_id: installation.into(),
        role: MembershipRole::Admin,
    };

    let signed_in = app
        .sign_in(&sign_in_code(), JobClass::Interactive)
        .await
        .unwrap();

    replay.finish();
    let octocat = ScmUser {
        external_id: "5".into(),
        login: "octocat".into(),
    };
    assert_eq!(signed_in.user, octocat);
    assert_eq!(signed_in.memberships, [membership("7"), membership("8")]);
}

#[tokio::test]
async fn a_refused_sign_in_code_is_not_valid() {
    let now = OffsetDateTime::from_unix_timestamp(1_789_000_000).unwrap();
    let refused = json!({ "error": "bad_verification_code" });
    let replay = Replay::start(vec![answer(
        "POST",
        "/login/oauth/access_token",
        200,
        refused,
    )])
    .await;
    let app = app_with_sign_in(&replay.url, now);

    let signed_in = app.sign_in(&sign_in_code(), JobClass::Interactive).await;

    replay.finish();
    assert!(
        matches!(&signed_in, Err(ScmError::Validation(message)) if message.contains("bad_verification_code")),
        "{signed_in:?}"
    );
}

#[tokio::test]
async fn the_sign_in_address_carries_the_client_and_the_challenge() {
    let now = OffsetDateTime::from_unix_timestamp(1_789_000_000).unwrap();
    let request = SignInRequest {
        redirect_url: "https://ci.example.com/auth/github/callback".into(),
        state: "state".into(),
        code_challenge: "challenge".into(),
    };

    let address = app_with_sign_in("https://github.example.com", now).sign_in_url(&request);
    let without_client = app("https://github.example.com", now).sign_in_url(&request);

    assert_eq!(
        address.as_deref(),
        Some(
            "https://github.example.com/login/oauth/authorize?client_id=Iv1.client\
             &redirect_uri=https%3A%2F%2Fci.example.com%2Fauth%2Fgithub%2Fcallback\
             &state=state&code_challenge=challenge&code_challenge_method=S256"
        )
    );
    assert_eq!(without_client, None);
}

/// Written by hand from the documentation of GitHub, not recorded: the
/// recording by the `github` example does not read the members yet.
#[tokio::test]
async fn the_members_of_an_organization_come_with_their_role() {
    let now = OffsetDateTime::from_unix_timestamp(1_789_000_000).unwrap();
    let members = |role: &str| format!("/orgs/acme/members?role={role}&per_page=100&page=1");
    let acme = json!({ "id": 7, "account": { "id": 70, "login": "acme", "type": "Organization" } });
    let exchanges = vec![
        answer("GET", "/app/installations/7", 200, acme),
        token("7", now),
        answer(
            "GET",
            &members("admin"),
            200,
            json!([{ "id": 5, "login": "octocat" }]),
        ),
        answer(
            "GET",
            &members("member"),
            200,
            json!([{ "id": 6, "login": "hubot" }]),
        ),
    ];
    let replay = Replay::start(exchanges).await;
    let app = app(&replay.url, now);
    let member = |id: &str, login: &str, role| ScmMember {
        user: ScmUser {
            external_id: id.into(),
            login: login.into(),
        },
        role,
    };

    let listed = app
        .members(&repo("7", "acme", "app").connection, JobClass::Repair)
        .await
        .unwrap();

    replay.finish();
    assert_eq!(
        listed,
        [
            member("5", "octocat", MembershipRole::Admin),
            member("6", "hubot", MembershipRole::Member),
        ]
    );
}

#[tokio::test]
async fn the_owner_of_the_account_of_a_person_is_its_only_admin() {
    let now = OffsetDateTime::from_unix_timestamp(1_789_000_000).unwrap();
    let octocat = json!({ "id": 8, "account": { "id": 5, "login": "octocat", "type": "User" } });
    let exchanges = vec![answer("GET", "/app/installations/8", 200, octocat)];
    let replay = Replay::start(exchanges).await;
    let app = app(&replay.url, now);
    let owner = ScmMember {
        user: ScmUser {
            external_id: "5".into(),
            login: "octocat".into(),
        },
        role: MembershipRole::Admin,
    };

    let listed = app
        .members(&repo("8", "octocat", "app").connection, JobClass::Repair)
        .await
        .unwrap();

    replay.finish();
    assert_eq!(listed, [owner]);
}

#[tokio::test]
async fn an_account_neither_organization_nor_person_has_no_member() {
    let now = OffsetDateTime::from_unix_timestamp(1_789_000_000).unwrap();
    let enterprise =
        json!({ "id": 9, "account": { "id": 5, "login": "megacorp", "type": "Enterprise" } });
    let exchanges = vec![answer("GET", "/app/installations/9", 200, enterprise)];
    let replay = Replay::start(exchanges).await;
    let app = app(&replay.url, now);

    let listed = app
        .members(&repo("9", "megacorp", "app").connection, JobClass::Repair)
        .await
        .unwrap();

    replay.finish();
    assert!(listed.is_empty());
}

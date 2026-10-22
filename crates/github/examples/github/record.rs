//! Prepares the repository of the contract suite on a real GitHub, then plays
//! the suite through a recording proxy and writes each exchange in
//! `tests/recorded/contract.json`.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, PoisonError};

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::Response;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use cihub_domain::{CheckKind, JobClass, Mergeability, ReviewDecision};
use cihub_github::{AppKey, GithubApp, GithubConfig};
use cihub_scm::contract;
use cihub_scm::{CommitStatus, RepoRef, Scm, StatusUpdate};
use serde_json::{Value, json};
use tokio::net::TcpListener;

use crate::{Result, api_urls, now};

#[path = "../../tests/support/cassette.rs"]
mod cassette;

use cassette::{Cassette, Exchange, KEPT_HEADERS, RECORDED_TOKEN, World, api_path};

const CLASS: JobClass = JobClass::Interactive;
const BASE: &str = "main";
const REQUIRED: &str = "cihub-contract-ci";
const FILE: (&str, &str) = ("cihub-contract/file.txt", "contract\n");
const CONFLICTING_BRANCH: &str = "cihub-contract/conflicting";
const DRAFT_BRANCH: &str = "cihub-contract/draft";
const WORK_BRANCH: &str = "cihub-temp/contract";
const MISSING_PR: u64 = 999_999;
const RECORDING: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/recorded/contract.json");

struct Rest {
    http: reqwest::Client,
    repo_url: String,
    token: String,
}

impl Rest {
    async fn new(config: &GithubConfig, repo: &RepoRef) -> Result<Self> {
        let (api, _) = api_urls(&config.url);
        let jwt = AppKey::from_pem(&config.private_key)?.jwt(&config.app_id, now())?;
        let http = reqwest::Client::builder().user_agent("cihub").build()?;
        let installation = &repo.connection.external_id;
        let url = format!("{api}/app/installations/{installation}/access_tokens");
        let answer = send(http.post(url).bearer_auth(jwt), None).await?;
        let token = answer["token"].as_str().ok_or("no token")?.to_owned();
        let repo_url = format!("{api}/repos/{}/{}", repo.namespace, repo.name);
        Ok(Self {
            http,
            repo_url,
            token,
        })
    }

    async fn put_file(&self, branch: &str, path: &str, content: &str) -> Result<()> {
        let url = format!("{}/contents/{path}", self.repo_url);
        let body = json!({
            "message": format!("cihub contract: {path}"),
            "content": STANDARD.encode(content),
            "branch": branch,
        });
        send(self.http.put(url).bearer_auth(&self.token), Some(body)).await?;
        Ok(())
    }

    async fn open_pr(&self, head: &str, title: &str) -> Result<u64> {
        let url = format!("{}/pulls", self.repo_url);
        let body = json!({ "title": title, "head": head, "base": BASE });
        let answer = send(self.http.post(url).bearer_auth(&self.token), Some(body)).await?;
        Ok(answer["number"].as_u64().ok_or("no PR number")?)
    }
}

pub async fn send(request: reqwest::RequestBuilder, body: Option<Value>) -> Result<Value> {
    let mut request = request
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28");
    if let Some(body) = body {
        request = request
            .header("Content-Type", "application/json")
            .body(body.to_string());
    }

    let response = request.send().await?;
    let status = response.status();
    let body: Value = serde_json::from_slice(&response.bytes().await?).unwrap_or_default();
    if !status.is_success() {
        return Err(format!("GitHub answered {status}: {body}").into());
    }

    Ok(body)
}

pub async fn prepare_base(app: &GithubApp, config: &GithubConfig, repo: &RepoRef) -> Result<()> {
    let rest = Rest::new(config, repo).await?;
    let head = app
        .branch_head(repo, BASE, CLASS)
        .await?
        .ok_or("no branch main")?;
    app.set_branch(repo, CONFLICTING_BRANCH, &head, false, CLASS)
        .await?;

    let conflict = "cihub-contract/conflict.txt";
    rest.put_file(CONFLICTING_BRANCH, conflict, "conflicting\n")
        .await?;

    rest.put_file(BASE, conflict, "base\n").await?;
    rest.put_file(BASE, FILE.0, FILE.1).await?;
    let conflicting = rest
        .open_pr(CONFLICTING_BRANCH, "cihub contract: conflicting")
        .await?;
    let head = app
        .branch_head(repo, BASE, CLASS)
        .await?
        .ok_or("no branch main")?;
    app.set_branch(repo, DRAFT_BRANCH, &head, false, CLASS)
        .await?;

    rest.put_file(DRAFT_BRANCH, "cihub-contract/draft.txt", "draft\n")
        .await?;

    let draft = app
        .open_draft_pr(repo, DRAFT_BRANCH, BASE, "cihub contract: draft", "", CLASS)
        .await?;
    println!("conflicting PR: {conflicting}\ndraft PR: {draft}");
    println!(
        "now add a ruleset on `main`: require the status `{REQUIRED}` (branches up to date \
         not required) and one approval, then run `prepare-pr`"
    );

    Ok(())
}

pub async fn prepare_pr(app: &GithubApp, config: &GithubConfig, repo: &RepoRef) -> Result<()> {
    let rest = Rest::new(config, repo).await?;
    let stamp = now().unix_timestamp();
    let branch = format!("cihub-contract/pr-{stamp}");
    let head = app
        .branch_head(repo, BASE, CLASS)
        .await?
        .ok_or("no branch main")?;
    app.set_branch(repo, &branch, &head, false, CLASS).await?;
    rest.put_file(&branch, &format!("cihub-contract/pr-{stamp}.txt"), "pr\n")
        .await?;

    let number = rest.open_pr(&branch, "cihub contract: mergeable").await?;
    let pr = app.pull_request(repo, number, CLASS).await?;
    let status = StatusUpdate {
        sha: pr.head_sha,
        context: REQUIRED.to_owned(),
        status: CommitStatus::Success,
        description: "the required status of the contract suite".to_owned(),
        details_url: None,
    };
    app.set_status(repo, &status, CLASS).await?;
    println!("mergeable PR: {number}\napprove it on GitHub, then run `record`");

    Ok(())
}

pub struct Prs {
    pub mergeable: u64,
    pub conflicting: u64,
    pub draft: u64,
    pub admin: String,
}

pub async fn record(
    app: &GithubApp,
    config: &GithubConfig,
    repo: &RepoRef,
    prs: Prs,
) -> Result<()> {
    wait_for_mergeability(app, repo, &prs).await?;
    let proxy = Proxy::start(&config.url).await?;
    let recorded_at = now();
    let through_proxy = GithubConfig {
        url: proxy.url.clone(),
        app_id: config.app_id.clone(),
        private_key: config.private_key.clone(),
        sign_in: None,
    };
    let recording_app = GithubApp::new(&through_proxy, Arc::new(now))?;
    let world = World {
        installation: repo.connection.external_id.clone(),
        namespace: repo.namespace.clone(),
        name: repo.name.clone(),
        base_branch: BASE.to_owned(),
        required_checks: BTreeSet::from([REQUIRED.to_owned()]),
        up_to_date: false,
        file_path: FILE.0.to_owned(),
        file_content: FILE.1.to_owned(),
        mergeable_pr: prs.mergeable,
        conflicting_pr: prs.conflicting,
        draft_pr: prs.draft,
        missing_pr: MISSING_PR,
        admin: prs.admin,
        work_branch: WORK_BRANCH.to_owned(),
    };
    contract::run(&recording_app, repo, &world.fixture()).await;
    let mut exchanges = std::mem::take(
        &mut *proxy
            .exchanges
            .lock()
            .unwrap_or_else(PoisonError::into_inner),
    );
    for exchange in &mut exchanges {
        keep_only_repo(exchange, repo);
    }

    let cassette = Cassette {
        recorded_at: recorded_at.unix_timestamp(),
        world,
        exchanges,
    };
    std::fs::write(RECORDING, serde_json::to_string_pretty(&cassette)? + "\n")?;
    println!(
        "{} exchanges written in {RECORDING}",
        cassette.exchanges.len()
    );

    Ok(())
}

/// Keeps only the repository of the suite in a list of the repositories of
/// the installation: the others can be private, and the cassette is public.
fn keep_only_repo(exchange: &mut Exchange, repo: &RepoRef) {
    if !exchange.path.starts_with("/installation/repositories") {
        return;
    }

    let Some(Value::Object(page)) = exchange.body.as_mut() else {
        return;
    };

    let full_name = format!("{}/{}", repo.namespace, repo.name);
    if let Some(Value::Array(repositories)) = page.get_mut("repositories") {
        repositories.retain(|listed| listed["full_name"] == full_name.as_str());
        let kept = repositories.len();
        page.insert("total_count".to_owned(), Value::from(kept));
    }
}

/// GitHub computes the mergeability of a PR later: reads the PRs until it is
/// known, the approval is there and the checks of the mergeable PR concluded.
async fn wait_for_mergeability(app: &GithubApp, repo: &RepoRef, prs: &Prs) -> Result<()> {
    let numbers = [prs.mergeable, prs.conflicting, prs.draft];
    let mut last = Vec::new();
    for _ in 0..20 {
        let read = app.pull_requests(repo, &numbers, CLASS).await?;
        let known = read
            .iter()
            .all(|pr| pr.mergeability != Mergeability::Unknown);
        let approved = read.iter().any(|pr| {
            pr.number == prs.mergeable
                && pr.review_decision == ReviewDecision::Approved
                && pr.mergeability == Mergeability::Mergeable
                && pr.checks.iter().any(|check| check.kind == CheckKind::Check)
                && pr.checks.iter().all(|check| check.conclusion.is_some())
        });
        if read.len() == numbers.len() && known && approved {
            return Ok(());
        }

        last = read;
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }

    let states: Vec<String> = last
        .iter()
        .map(|pr| {
            format!(
                "#{}: {:?}, draft {}, {:?}, {:?}",
                pr.number, pr.state, pr.draft, pr.mergeability, pr.review_decision
            )
        })
        .collect();
    Err(format!(
        "the PRs are not ready: the mergeable PR must be open, approved and mergeable, \
         with its status passed and the check runs of the CI concluded \
         (arguments: <mergeable> <conflicting> <draft>)\n{}",
        states.join("\n")
    )
    .into())
}

struct Proxy {
    url: String,
    exchanges: Arc<Mutex<Vec<Exchange>>>,
}

#[derive(Clone)]
struct Upstream {
    http: reqwest::Client,
    rest: String,
    graphql: String,
    exchanges: Arc<Mutex<Vec<Exchange>>>,
}

impl Proxy {
    async fn start(github_url: &str) -> Result<Self> {
        let (rest, graphql) = api_urls(github_url);
        let exchanges = Arc::new(Mutex::new(Vec::new()));
        let upstream = Upstream {
            http: reqwest::Client::builder().user_agent("cihub").build()?,
            rest,
            graphql,
            exchanges: Arc::clone(&exchanges),
        };
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let url = format!("http://{}", listener.local_addr()?);
        let router = Router::new().fallback(forward).with_state(upstream);
        tokio::spawn(async move { axum::serve(listener, router).await });

        Ok(Self { url, exchanges })
    }
}

async fn forward(
    State(upstream): State<Upstream>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let full = uri.path_and_query().map_or(uri.path(), |p| p.as_str());
    let path = api_path(full).unwrap_or(full).to_owned();
    let url = if path == "/graphql" {
        upstream.graphql.clone()
    } else {
        format!("{}{path}", upstream.rest)
    };
    let Ok(forwarded) = reqwest::Method::from_bytes(method.as_str().as_bytes()) else {
        return failure(format!("method {method}"));
    };

    let mut request = upstream.http.request(forwarded, url).body(body.to_vec());
    for name in [
        "authorization",
        "accept",
        "content-type",
        "x-github-api-version",
    ] {
        if let Some(value) = headers.get(name).and_then(|v| v.to_str().ok()) {
            request = request.header(name, value);
        }
    }

    let response = match request.send().await {
        Ok(response) => response,
        Err(err) => return failure(err.to_string()),
    };
    let status = response.status().as_u16();
    let kept: Vec<(String, String)> = KEPT_HEADERS
        .iter()
        .filter_map(|name| {
            let value = response.headers().get(*name)?.to_str().ok()?;
            Some(((*name).to_owned(), value.to_owned()))
        })
        .collect();
    let answer = match response.bytes().await {
        Ok(answer) => answer,
        Err(err) => return failure(err.to_string()),
    };
    let mut recorded_body = serde_json::from_slice::<Value>(&answer).ok();
    if path.ends_with("/access_tokens")
        && let Some(token) = recorded_body.as_mut().and_then(|b| b.get_mut("token"))
    {
        *token = Value::from(RECORDED_TOKEN);
    }

    let text = match recorded_body {
        None if !answer.is_empty() => Some(String::from_utf8_lossy(&answer).into_owned()),
        _ => None,
    };
    let exchange = Exchange {
        method: method.to_string(),
        path,
        request: serde_json::from_slice(&body).ok(),
        status,
        headers: kept.iter().cloned().collect(),
        body: recorded_body,
        text,
    };
    upstream
        .exchanges
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(exchange);

    let mut builder = Response::builder().status(status);
    for (name, value) in kept {
        builder = builder.header(name, value);
    }

    builder
        .body(Body::from(answer))
        .unwrap_or_else(|err| failure(err.to_string()))
}

fn failure(message: String) -> Response {
    let mut response = Response::new(Body::from(message));
    *response.status_mut() = StatusCode::BAD_GATEWAY;
    response
}

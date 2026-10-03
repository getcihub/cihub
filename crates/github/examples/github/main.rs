//! Calls a real GitHub through `cihub-github` and prints what comes back:
//! to check by hand that the App authenticates and that each call of the
//! `Scm` trait returns the right data.
//!
//! The App comes from the environment, as for `cihub-server` (`DECISIONS.md`
//! D79): `CIHUB_GITHUB_APP_ID`, `CIHUB_GITHUB_PRIVATE_KEY` or
//! `CIHUB_GITHUB_PRIVATE_KEY_FILE`, and `CIHUB_GITHUB_URL` (`https://github.com`
//! by default). After each call, the example prints the budget of the
//! connection, as the rate limit headers give it.
//!
//! The reads change nothing. The writes act only on what they name: the
//! branch `cihub-temp/example`, a status context, or the PR given.
//!
//! ```sh
//! set -gx CIHUB_GITHUB_APP_ID 123456
//! set -gx CIHUB_GITHUB_PRIVATE_KEY_FILE ~/Downloads/app.private-key.pem
//! cargo run -p cihub-github --example github -- installations
//! cargo run -p cihub-github --example github -- 789 acme/widgets prs 12 13
//! cargo run -p cihub-github --example github -- 789 acme/widgets branch main
//! ```
//!
//! Commands, after `<installation> <owner>/<repo>`:
//!
//! - `prs <number>...`: the PRs, in one GraphQL query;
//! - `branch <name>`: the head of a branch;
//! - `checks <sha>`: the checks and statuses of a commit;
//! - `rules <branch>`: the checks that the rules of a branch require;
//! - `file <ref> <path>`: the content of a file;
//! - `permission <login>`: the permission of a user on the repository;
//! - `branch-roundtrip <branch>` (write): creates `cihub-temp/example` at the
//!   head of `<branch>`, reads it, moves it again, then deletes it twice;
//! - `status <sha> <pending|success|failure|error>` (write): publishes the
//!   status `cihub-example` on a commit;
//! - `comment <pr> <body>` (write): comments on a PR, then edits the comment;
//! - `label <pr> <label>` (write): adds the label twice, then removes it twice.
//!
//! The recording of the contract suite (`DECISIONS.md` D90), on a test
//! repository whose default branch is `main`:
//!
//! 1. `prepare-base` (write), once: commits the file of the suite on `main`,
//!    opens a PR in conflict with `main` and a draft PR. Then add a ruleset
//!    on `main` that requires the status `cihub-contract-ci` and one approval,
//!    without "branches up to date".
//! 2. `prepare-pr` (write), before each recording: opens a PR and passes its
//!    required status. Approve it on GitHub: the App is its author, so any
//!    person with the write permission can approve it.
//! 3. `record <admin> <mergeable> <conflicting> <draft>` (write): plays the
//!    suite through a recording proxy, merges the mergeable PR, and writes
//!    `tests/recorded/contract.json` without the token. `<admin>` is the login
//!    of an admin of the repository.

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a command-line example writes its result to the terminal"
)]

mod record;

use std::error::Error;
use std::fmt::Debug;
use std::sync::Arc;

use cihub_domain::{ConnectionId, JobClass};
use cihub_github::{AppKey, GithubApp, GithubConfig};
use cihub_scm::{CommitStatus, ConnectionRef, RepoRef, Scm, ScmUser, StatusUpdate};
use secrecy::SecretString;
use time::OffsetDateTime;
use ulid::Ulid;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Every call of the example is a person waiting at the terminal.
const CLASS: JobClass = JobClass::Interactive;

const USAGE: &str = "usage: github installations
       github <installation> <owner>/<repo> <command> [arguments]
commands: prs, branch, checks, rules, file, permission,
          branch-roundtrip, status, comment, label,
          prepare-base, prepare-pr, record (see the example documentation)";

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(err) = run(std::env::args().skip(1).collect()).await {
        eprintln!("error: {err}");
        let mut source = err.source();
        while let Some(err) = source {
            eprintln!("  caused by: {err}");
            source = err.source();
        }
        std::process::exit(1);
    }
}

fn now() -> OffsetDateTime {
    #[expect(
        clippy::disallowed_methods,
        reason = "the example talks to a real GitHub, on the real time"
    )]
    OffsetDateTime::now_utc()
}

fn config() -> Result<GithubConfig> {
    let env = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
    let private_key = match (
        env("CIHUB_GITHUB_PRIVATE_KEY"),
        env("CIHUB_GITHUB_PRIVATE_KEY_FILE"),
    ) {
        (Some(key), _) => key,
        (None, Some(path)) => std::fs::read_to_string(&path)
            .map_err(|err| format!("cannot read CIHUB_GITHUB_PRIVATE_KEY_FILE {path}: {err}"))?,
        (None, None) => {
            return Err("set CIHUB_GITHUB_PRIVATE_KEY or CIHUB_GITHUB_PRIVATE_KEY_FILE".into());
        }
    };
    Ok(GithubConfig {
        url: env("CIHUB_GITHUB_URL").unwrap_or_else(|| "https://github.com".into()),
        app_id: env("CIHUB_GITHUB_APP_ID").ok_or("set CIHUB_GITHUB_APP_ID")?,
        private_key: SecretString::from(private_key),
    })
}

async fn run(args: Vec<String>) -> Result<()> {
    let config = config()?;
    let app = GithubApp::new(&config, Arc::new(now))?;
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["installations"] => installations(&config).await,
        [installation, repo, "prepare-base"] => {
            record::prepare_base(&app, &config, &repo_ref(installation, repo)?).await
        }
        [installation, repo, "prepare-pr"] => {
            record::prepare_pr(&app, &config, &repo_ref(installation, repo)?).await
        }
        [
            installation,
            repo,
            "record",
            admin,
            mergeable,
            conflicting,
            draft,
        ] => {
            let prs = record::Prs {
                mergeable: mergeable.parse()?,
                conflicting: conflicting.parse()?,
                draft: draft.parse()?,
                admin: (*admin).to_owned(),
            };
            record::record(&app, &config, &repo_ref(installation, repo)?, prs).await
        }
        [installation, repo, command, rest @ ..] => {
            let repo = repo_ref(installation, repo)?;
            command_on(&app, &repo, command, rest).await?;
            for (_, budget) in app.take_changed_budgets() {
                println!("\nbudget: {budget:#?}");
            }
            Ok(())
        }
        _ => Err(USAGE.into()),
    }
}

/// A repository for the calls. The example knows no identifier of CIHub:
/// the adapter reads only the installation, the owner and the name.
fn repo_ref(installation: &str, repo: &str) -> Result<RepoRef> {
    let (namespace, name) = repo
        .split_once('/')
        .ok_or("the repository is <owner>/<repo>")?;
    Ok(RepoRef {
        connection: ConnectionRef {
            id: ConnectionId::from_ulid(Ulid(1)),
            external_id: (*installation).to_owned(),
        },
        external_id: String::new(),
        namespace: namespace.to_owned(),
        name: name.to_owned(),
    })
}

fn show<T: Debug>(call: &str, result: &T) {
    println!("{call}:\n{result:#?}");
}

async fn command_on(app: &GithubApp, repo: &RepoRef, command: &str, args: &[&str]) -> Result<()> {
    match (command, args) {
        ("prs", numbers) if !numbers.is_empty() => {
            let numbers = numbers
                .iter()
                .map(|n| n.parse::<u64>())
                .collect::<std::result::Result<Vec<_>, _>>()?;
            show(
                "pull_requests",
                &app.pull_requests(repo, &numbers, CLASS).await?,
            );
        }
        ("branch", [name]) => show("branch_head", &app.branch_head(repo, name, CLASS).await?),
        ("checks", [sha]) => show("checks", &app.checks(repo, sha, CLASS).await?),
        ("rules", [branch]) => {
            show(
                "required_checks",
                &app.required_checks(repo, branch, CLASS).await?,
            );
        }
        ("file", [git_ref, path]) => {
            show("file", &app.file(repo, git_ref, path, CLASS).await?);
        }
        ("permission", [login]) => {
            let user = ScmUser {
                external_id: String::new(),
                login: (*login).to_owned(),
            };
            show("permission", &app.permission(repo, &user, CLASS).await?);
        }
        ("branch-roundtrip", [from]) => branch_roundtrip(app, repo, from).await?,
        ("status", [sha, status]) => {
            let status = match *status {
                "pending" => CommitStatus::Pending,
                "success" => CommitStatus::Success,
                "failure" => CommitStatus::Failure,
                "error" => CommitStatus::Error,
                other => return Err(format!("unknown status {other}").into()),
            };
            let update = StatusUpdate {
                sha: (*sha).to_owned(),
                context: "cihub-example".into(),
                status,
                description: "published by the example of cihub-github".into(),
                details_url: None,
            };
            app.set_status(repo, &update, CLASS).await?;
            show(
                "checks after set_status",
                &app.checks(repo, sha, CLASS).await?,
            );
        }
        ("comment", [pr, body]) => {
            let pr = pr.parse()?;
            let id = app.create_comment(repo, pr, body, CLASS).await?;
            show("create_comment", &id);
            let edited = format!("{body}\n\n(edited by the example of cihub-github)");
            app.update_comment(repo, &id, &edited, CLASS).await?;
            println!("update_comment: done");
        }
        ("label", [pr, label]) => {
            let pr = pr.parse()?;
            for _ in 0..2 {
                app.add_label(repo, pr, label, CLASS).await?;
            }
            let labels = app.pull_request(repo, pr, CLASS).await?.labels;
            show("labels after add_label twice", &labels);
            for _ in 0..2 {
                app.remove_label(repo, pr, label, CLASS).await?;
            }
            let labels = app.pull_request(repo, pr, CLASS).await?.labels;
            show("labels after remove_label twice", &labels);
        }
        _ => return Err(USAGE.into()),
    }
    Ok(())
}

async fn branch_roundtrip(app: &GithubApp, repo: &RepoRef, from: &str) -> Result<()> {
    const BRANCH: &str = "cihub-temp/example";
    let head = app
        .branch_head(repo, from, CLASS)
        .await?
        .ok_or_else(|| format!("no branch {from}"))?;
    show("head of the source branch", &head);
    show("before", &app.branch_head(repo, BRANCH, CLASS).await?);
    for _ in 0..2 {
        app.set_branch(repo, BRANCH, &head, false, CLASS).await?;
    }
    show(
        "after set_branch twice",
        &app.branch_head(repo, BRANCH, CLASS).await?,
    );
    for _ in 0..2 {
        app.delete_branch(repo, BRANCH, CLASS).await?;
    }
    show(
        "after delete_branch twice",
        &app.branch_head(repo, BRANCH, CLASS).await?,
    );
    Ok(())
}

/// The installations of the App, to find the one to give to the other
/// commands. The only call outside the `Scm` trait: it authenticates as the
/// App, not as an installation.
async fn installations(config: &GithubConfig) -> Result<()> {
    let key = AppKey::from_pem(&config.private_key)?;
    let jwt = key.jwt(&config.app_id, now())?;
    let (api, _) = api_urls(&config.url);
    let http = reqwest::Client::builder().user_agent("cihub").build()?;
    let request = http
        .get(format!("{api}/app/installations"))
        .bearer_auth(jwt);
    let body = record::send(request, None).await?;
    for installation in body.as_array().into_iter().flatten() {
        println!(
            "installation {} on {} ({})",
            installation["id"],
            installation["account"]["login"],
            installation["repository_selection"]
        );
    }
    Ok(())
}

/// The addresses of the REST and GraphQL APIs of a GitHub, as `GithubApp`
/// reaches them.
fn api_urls(github_url: &str) -> (String, String) {
    let url = github_url.trim_end_matches('/');
    if url == "https://github.com" {
        (
            "https://api.github.com".to_owned(),
            "https://api.github.com/graphql".to_owned(),
        )
    } else {
        (format!("{url}/api/v3"), format!("{url}/api/graphql"))
    }
}

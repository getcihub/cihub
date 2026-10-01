//! The `Scm` trait on GitHub: reads in GraphQL, where one query reads 50 PRs,
//! and writes in REST. Each value of GitHub becomes a neutral value here
//! (`docs/GITHUB.md`, `AGENTS.md` D23, D78).

use std::fmt::Write;

use cihub_domain::{
    CheckConclusion, CheckKind, CheckReport, JobClass, MergeMethod, Mergeability, PrState,
    ReviewDecision,
};
use cihub_scm::{
    CommitStatus, ConnectionRef, MAX_PRS_PER_READ, MergeIntoRef, PrMerge, PrSnapshot,
    RecentDelivery, RepoPermission, RepoRef, RequiredChecks, Scm, ScmError, ScmFuture, ScmUser,
    StatusUpdate,
};
use reqwest::{Method, StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::GithubApp;
use crate::client::{Answer, JSON, Request};

/// The fields of a PR that CIHub reads, and the checks of its head.
const PR_FIELDS: &str = "
fragment checks on StatusCheckRollup {
  contexts(first: 100) {
    nodes {
      __typename
      ... on CheckRun { name status conclusion detailsUrl }
      ... on StatusContext { context state targetUrl }
    }
  }
}
fragment pr on PullRequest {
  number state isDraft headRefOid baseRefName mergeable mergeStateStatus reviewDecision
  additions deletions changedFiles
  mergeCommit { oid }
  author { login ... on User { databaseId } ... on Bot { databaseId } }
  labels(first: 100) { nodes { name } }
  statusCheckRollup { ...checks }
}";

const CHECKS_QUERY: &str = "
query($owner: String!, $name: String!, $sha: GitObjectID!) {
  repository(owner: $owner, name: $name) {
    object(oid: $sha) { ... on Commit { statusCheckRollup { ...checks } } }
  }
}
fragment checks on StatusCheckRollup {
  contexts(first: 100) {
    nodes {
      __typename
      ... on CheckRun { name status conclusion detailsUrl }
      ... on StatusContext { context state targetUrl }
    }
  }
}";

#[derive(Deserialize)]
struct GraphqlAnswer {
    data: Option<Value>,
    #[serde(default)]
    errors: Vec<GraphqlError>,
}

#[derive(Deserialize)]
struct GraphqlError {
    #[serde(default, rename = "type")]
    kind: Option<String>,
    message: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphqlPr {
    number: u64,
    state: String,
    is_draft: bool,
    head_ref_oid: String,
    base_ref_name: String,
    mergeable: String,
    merge_state_status: String,
    review_decision: Option<String>,
    additions: u64,
    deletions: u64,
    changed_files: u64,
    merge_commit: Option<Oid>,
    author: Option<GraphqlAuthor>,
    labels: Nodes<Label>,
    status_check_rollup: Option<Rollup>,
}

#[derive(Deserialize)]
struct Oid {
    oid: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphqlAuthor {
    login: String,
    database_id: Option<u64>,
}

#[derive(Deserialize)]
struct Nodes<T> {
    nodes: Vec<Option<T>>,
}

#[derive(Deserialize)]
struct Label {
    name: String,
}

#[derive(Deserialize)]
struct Rollup {
    contexts: Nodes<Context>,
}

#[derive(Deserialize)]
#[serde(tag = "__typename")]
enum Context {
    #[serde(rename_all = "camelCase")]
    CheckRun {
        name: String,
        status: String,
        conclusion: Option<String>,
        details_url: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    StatusContext {
        context: String,
        state: String,
        target_url: Option<String>,
    },
}

fn pr_state(state: &str) -> Result<PrState, ScmError> {
    match state {
        "OPEN" => Ok(PrState::Open),
        "CLOSED" => Ok(PrState::Closed),
        "MERGED" => Ok(PrState::Merged),
        other => Err(ScmError::Protocol(format!("PR state {other}"))),
    }
}

/// `docs/GITHUB.md`: `UNSTABLE` (a check that is not required failed) and
/// `HAS_HOOKS` admit the PR, as the branch protection of GitHub does.
fn mergeability(mergeable: &str, status: &str) -> Mergeability {
    if mergeable == "CONFLICTING" {
        return Mergeability::Conflict;
    }
    match status {
        "CLEAN" | "HAS_HOOKS" | "UNSTABLE" => Mergeability::Mergeable,
        "BEHIND" => Mergeability::Behind,
        "DIRTY" => Mergeability::Conflict,
        "BLOCKED" | "DRAFT" => Mergeability::Blocked,
        _ => Mergeability::Unknown,
    }
}

fn review_decision(decision: Option<&str>) -> ReviewDecision {
    match decision {
        Some("APPROVED") => ReviewDecision::Approved,
        Some("CHANGES_REQUESTED") => ReviewDecision::ChangesRequested,
        Some("REVIEW_REQUIRED") => ReviewDecision::ReviewRequired,
        _ => ReviewDecision::None,
    }
}

fn check_conclusion(conclusion: &str) -> CheckConclusion {
    match conclusion {
        "SUCCESS" => CheckConclusion::Success,
        "NEUTRAL" => CheckConclusion::Neutral,
        "SKIPPED" => CheckConclusion::Skipped,
        "CANCELLED" => CheckConclusion::Cancelled,
        "TIMED_OUT" => CheckConclusion::TimedOut,
        "ACTION_REQUIRED" => CheckConclusion::ActionRequired,
        "STALE" => CheckConclusion::Stale,
        // `FAILURE`, `STARTUP_FAILURE`, and a value that GitHub adds later.
        _ => CheckConclusion::Failure,
    }
}

fn check_reports(rollup: Option<Rollup>) -> Vec<CheckReport> {
    let Some(rollup) = rollup else {
        return Vec::new();
    };
    rollup
        .contexts
        .nodes
        .into_iter()
        .flatten()
        .map(|context| match context {
            Context::CheckRun {
                name,
                status,
                conclusion,
                details_url,
            } => CheckReport {
                name,
                kind: CheckKind::Check,
                conclusion: (status == "COMPLETED")
                    .then(|| conclusion.as_deref().map(check_conclusion))
                    .flatten(),
                details_url,
            },
            Context::StatusContext {
                context,
                state,
                target_url,
            } => CheckReport {
                name: context,
                kind: CheckKind::Status,
                conclusion: match state.as_str() {
                    "SUCCESS" => Some(CheckConclusion::Success),
                    "FAILURE" => Some(CheckConclusion::Failure),
                    "ERROR" => Some(CheckConclusion::Error),
                    _ => None,
                },
                details_url: target_url,
            },
        })
        .collect()
}

impl GraphqlPr {
    fn snapshot(self) -> Result<PrSnapshot, ScmError> {
        let author = self.author.map_or_else(
            || ScmUser {
                external_id: String::new(),
                login: "ghost".into(),
            },
            |author| ScmUser {
                external_id: author
                    .database_id
                    .map(|id| id.to_string())
                    .unwrap_or_default(),
                login: author.login,
            },
        );
        Ok(PrSnapshot {
            number: self.number,
            state: pr_state(&self.state)?,
            head_sha: self.head_ref_oid,
            base_branch: self.base_ref_name,
            draft: self.is_draft,
            labels: self
                .labels
                .nodes
                .into_iter()
                .flatten()
                .map(|label| label.name)
                .collect(),
            mergeability: mergeability(&self.mergeable, &self.merge_state_status),
            review_decision: review_decision(self.review_decision.as_deref()),
            checks: check_reports(self.status_check_rollup),
            author,
            additions: self.additions,
            deletions: self.deletions,
            changed_files: self.changed_files,
        })
    }
}

/// Encodes a part of a path or of a query, keeping `/`, which the branch
/// names of GitHub contain.
fn encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                encoded.push(char::from(byte));
            }
            _ => {
                let _ = write!(encoded, "%{byte:02X}");
            }
        }
    }
    encoded
}

/// The error of an answer whose status the caller does not expect.
fn unexpected(answer: &Answer) -> ScmError {
    let message = answer.message();
    match answer.status {
        StatusCode::NOT_FOUND => ScmError::NotFound(message),
        // GitHub says this when the App lacks the permission of the call,
        // or when the installation does not cover the repository.
        StatusCode::FORBIDDEN if message.contains("not accessible by integration") => {
            ScmError::Forbidden(format!(
                "the GitHub App lacks a permission for this call (docs/GITHUB.md), or its \
                 installation does not cover the repository: {message}"
            ))
        }
        StatusCode::FORBIDDEN => ScmError::Forbidden(message),
        StatusCode::UNPROCESSABLE_ENTITY | StatusCode::CONFLICT => ScmError::Validation(message),
        _ => ScmError::Protocol(message),
    }
}

/// GitHub Free offers no branch protection and no ruleset on a private
/// repository: it answers 403 with an offer to upgrade. Such a repository has
/// no rules, and the configuration checker reports it.
fn needs_paid_plan(answer: &Answer) -> bool {
    answer.status == StatusCode::FORBIDDEN && answer.message().contains("Upgrade to GitHub")
}

fn expect(answer: Answer, statuses: &[StatusCode]) -> Result<Answer, ScmError> {
    if statuses.contains(&answer.status) {
        Ok(answer)
    } else {
        Err(unexpected(&answer))
    }
}

impl GithubApp {
    fn repo_path(repo: &RepoRef, rest: &str) -> String {
        format!(
            "/repos/{}/{}{rest}",
            encode(&repo.namespace),
            encode(&repo.name)
        )
    }

    async fn rest(
        &self,
        repo: &RepoRef,
        class: JobClass,
        method: Method,
        rest: &str,
        body: Option<Value>,
    ) -> Result<Answer, ScmError> {
        self.send(Request {
            connection: &repo.connection,
            class,
            method,
            path: Self::repo_path(repo, rest),
            body,
            accept: JSON,
        })
        .await
    }

    /// The `data` of a GraphQL answer. An error of type `NOT_FOUND` leaves
    /// its field `null`, as for a PR that does not exist.
    async fn query(
        &self,
        connection: &ConnectionRef,
        class: JobClass,
        query: &str,
        variables: Value,
    ) -> Result<Value, ScmError> {
        let answer = self.graphql(connection, class, query, variables).await?;
        let answer = expect(answer, &[StatusCode::OK])?;
        let GraphqlAnswer { data, errors } = answer.json()?;
        for error in &errors {
            match error.kind.as_deref() {
                Some("NOT_FOUND") => {}
                Some("RATE_LIMITED") => return Err(self.graphql_limit_reached(connection.id)),
                _ => return Err(ScmError::Protocol(error.message.clone())),
            }
        }
        data.ok_or_else(|| ScmError::Protocol("GraphQL answer without data".into()))
    }

    async fn read_prs(
        &self,
        repo: &RepoRef,
        numbers: &[u64],
        class: JobClass,
    ) -> Result<Vec<(GraphqlPr, Option<String>)>, ScmError> {
        if numbers.len() > MAX_PRS_PER_READ {
            return Err(ScmError::Validation(format!(
                "{} PRs in one read, {MAX_PRS_PER_READ} at most",
                numbers.len()
            )));
        }
        if numbers.is_empty() {
            return Ok(Vec::new());
        }
        let mut fields = String::new();
        for (i, number) in numbers.iter().enumerate() {
            let _ = writeln!(fields, "p{i}: pullRequest(number: {number}) {{ ...pr }}");
        }
        let query = format!(
            "query($owner: String!, $name: String!) {{\n  repository(owner: $owner, name: $name) {{\n{fields}  }}\n}}{PR_FIELDS}"
        );
        let variables = json!({ "owner": repo.namespace, "name": repo.name });
        let data = self
            .query(&repo.connection, class, &query, variables)
            .await?;
        let repository = data
            .get("repository")
            .filter(|repository| !repository.is_null())
            .ok_or_else(|| ScmError::NotFound(format!("repository {}", repo.name)))?;
        let mut prs = Vec::new();
        for i in 0..numbers.len() {
            let Some(pr) = repository.get(format!("p{i}")).filter(|pr| !pr.is_null()) else {
                continue;
            };
            let pr: GraphqlPr = serde_json::from_value(pr.clone())
                .map_err(|err| ScmError::Protocol(err.to_string()))?;
            let merged = pr.merge_commit.as_ref().map(|commit| commit.oid.clone());
            prs.push((pr, merged));
        }
        Ok(prs)
    }

    async fn read_pr(
        &self,
        repo: &RepoRef,
        number: u64,
        class: JobClass,
    ) -> Result<(GraphqlPr, Option<String>), ScmError> {
        self.read_prs(repo, &[number], class)
            .await?
            .pop()
            .ok_or_else(|| ScmError::NotFound(format!("PR {number}")))
    }

    async fn head_of(
        &self,
        repo: &RepoRef,
        branch: &str,
        class: JobClass,
    ) -> Result<Option<String>, ScmError> {
        #[derive(Deserialize)]
        struct Ref {
            object: Object,
        }
        #[derive(Deserialize)]
        struct Object {
            sha: String,
        }
        let rest = format!("/git/ref/heads/{}", encode(branch));
        let answer = self.rest(repo, class, Method::GET, &rest, None).await?;
        if answer.status == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let answer = expect(answer, &[StatusCode::OK])?;
        Ok(Some(answer.json::<Ref>()?.object.sha))
    }

    async fn move_branch(
        &self,
        repo: &RepoRef,
        branch: &str,
        sha: &str,
        force: bool,
        class: JobClass,
    ) -> Result<(), ScmError> {
        let rest = format!("/git/refs/heads/{}", encode(branch));
        let body = json!({ "sha": sha, "force": force });
        let answer = self
            .rest(repo, class, Method::PATCH, &rest, Some(body))
            .await?;
        if answer.status == StatusCode::OK {
            return Ok(());
        }
        if answer.status == StatusCode::UNPROCESSABLE_ENTITY
            && answer.message().contains("does not exist")
        {
            let body = json!({ "ref": format!("refs/heads/{branch}"), "sha": sha });
            let created = self
                .rest(repo, class, Method::POST, "/git/refs", Some(body))
                .await?;
            expect(created, &[StatusCode::CREATED])?;
            return Ok(());
        }
        Err(unexpected(&answer))
    }
}

impl Scm for GithubApp {
    fn pull_requests<'a>(
        &'a self,
        repo: &'a RepoRef,
        numbers: &'a [u64],
        class: JobClass,
    ) -> ScmFuture<'a, Vec<PrSnapshot>> {
        Box::pin(async move {
            self.read_prs(repo, numbers, class)
                .await?
                .into_iter()
                .map(|(pr, _)| pr.snapshot())
                .collect()
        })
    }

    fn pull_request<'a>(
        &'a self,
        repo: &'a RepoRef,
        number: u64,
        class: JobClass,
    ) -> ScmFuture<'a, PrSnapshot> {
        Box::pin(async move { self.read_pr(repo, number, class).await?.0.snapshot() })
    }

    fn branch_head<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, Option<String>> {
        Box::pin(self.head_of(repo, branch, class))
    }

    fn checks<'a>(
        &'a self,
        repo: &'a RepoRef,
        sha: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<CheckReport>> {
        Box::pin(async move {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Commit {
                status_check_rollup: Option<Rollup>,
            }
            let variables = json!({ "owner": repo.namespace, "name": repo.name, "sha": sha });
            let data = self
                .query(&repo.connection, class, CHECKS_QUERY, variables)
                .await?;
            let object = data
                .pointer("/repository/object")
                .filter(|object| !object.is_null())
                .ok_or_else(|| ScmError::NotFound(format!("commit {sha}")))?;
            let commit: Commit = serde_json::from_value(object.clone())
                .map_err(|err| ScmError::Protocol(err.to_string()))?;
            Ok(check_reports(commit.status_check_rollup))
        })
    }

    fn required_checks<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, RequiredChecks> {
        Box::pin(async move {
            #[derive(Deserialize)]
            struct Rule {
                #[serde(rename = "type")]
                kind: String,
                #[serde(default)]
                parameters: Value,
            }
            #[derive(Deserialize)]
            struct Protection {
                #[serde(default)]
                strict: bool,
                #[serde(default)]
                contexts: Vec<String>,
            }
            let mut required = RequiredChecks::default();
            let rest = format!("/rules/branches/{}", encode(branch));
            let rules = self.rest(repo, class, Method::GET, &rest, None).await?;
            let rules: Vec<Rule> = if needs_paid_plan(&rules) {
                Vec::new()
            } else {
                expect(rules, &[StatusCode::OK])?.json()?
            };
            for rule in rules {
                if rule.kind != "required_status_checks" {
                    continue;
                }
                let checks = rule.parameters["required_status_checks"].as_array();
                for check in checks.into_iter().flatten() {
                    if let Some(context) = check["context"].as_str() {
                        required.names.insert(context.to_owned());
                    }
                }
                required.up_to_date |= rule.parameters["strict_required_status_checks_policy"]
                    .as_bool()
                    .unwrap_or(false);
            }
            let rest = format!(
                "/branches/{}/protection/required_status_checks",
                encode(branch)
            );
            let classic = self.rest(repo, class, Method::GET, &rest, None).await?;
            if classic.status != StatusCode::NOT_FOUND && !needs_paid_plan(&classic) {
                let protection: Protection = expect(classic, &[StatusCode::OK])?.json()?;
                required.names.extend(protection.contexts);
                required.up_to_date |= protection.strict;
            }
            Ok(required)
        })
    }

    fn file<'a>(
        &'a self,
        repo: &'a RepoRef,
        git_ref: &'a str,
        path: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, Option<String>> {
        Box::pin(async move {
            let rest = format!("/contents/{}?ref={}", encode(path), encode(git_ref));
            let answer = self
                .send(Request {
                    connection: &repo.connection,
                    class,
                    method: Method::GET,
                    path: Self::repo_path(repo, &rest),
                    body: None,
                    accept: "application/vnd.github.raw+json",
                })
                .await?;
            if answer.status == StatusCode::NOT_FOUND {
                return Ok(None);
            }
            let answer = expect(answer, &[StatusCode::OK])?;
            String::from_utf8(answer.body)
                .map(Some)
                .map_err(|err| ScmError::Protocol(err.to_string()))
        })
    }

    fn permission<'a>(
        &'a self,
        repo: &'a RepoRef,
        user: &'a ScmUser,
        class: JobClass,
    ) -> ScmFuture<'a, RepoPermission> {
        Box::pin(async move {
            #[derive(Deserialize)]
            struct Permission {
                permission: String,
            }
            let rest = format!("/collaborators/{}/permission", encode(&user.login));
            let answer = self.rest(repo, class, Method::GET, &rest, None).await?;
            if answer.status == StatusCode::NOT_FOUND {
                return Ok(RepoPermission::None);
            }
            let answer = expect(answer, &[StatusCode::OK])?;
            Ok(match answer.json::<Permission>()?.permission.as_str() {
                "admin" => RepoPermission::Admin,
                "write" | "maintain" => RepoPermission::Write,
                "read" | "triage" => RepoPermission::Read,
                _ => RepoPermission::None,
            })
        })
    }

    fn set_branch<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        sha: &'a str,
        force: bool,
        class: JobClass,
    ) -> ScmFuture<'a, ()> {
        Box::pin(self.move_branch(repo, branch, sha, force, class))
    }

    fn delete_branch<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, ()> {
        Box::pin(async move {
            let rest = format!("/git/refs/heads/{}", encode(branch));
            let answer = self.rest(repo, class, Method::DELETE, &rest, None).await?;
            match answer.status {
                StatusCode::NO_CONTENT | StatusCode::NOT_FOUND => Ok(()),
                StatusCode::UNPROCESSABLE_ENTITY if answer.message().contains("does not exist") => {
                    Ok(())
                }
                _ => Err(unexpected(&answer)),
            }
        })
    }

    fn merge_into_branch<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        sha: &'a str,
        message: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, MergeIntoRef> {
        Box::pin(async move {
            #[derive(Deserialize)]
            struct Merge {
                sha: String,
            }
            let body = json!({ "base": branch, "head": sha, "commit_message": message });
            let answer = self
                .rest(repo, class, Method::POST, "/merges", Some(body))
                .await?;
            match answer.status {
                StatusCode::CREATED => Ok(MergeIntoRef::Merged {
                    sha: answer.json::<Merge>()?.sha,
                }),
                StatusCode::NO_CONTENT => Ok(MergeIntoRef::AlreadyContained),
                StatusCode::CONFLICT => Ok(MergeIntoRef::Conflict),
                _ => Err(unexpected(&answer)),
            }
        })
    }

    fn merge_pr<'a>(
        &'a self,
        repo: &'a RepoRef,
        merge: &'a PrMerge,
        class: JobClass,
    ) -> ScmFuture<'a, String> {
        Box::pin(async move {
            #[derive(Deserialize)]
            struct Merged {
                sha: String,
            }
            let method = match merge.method {
                MergeMethod::Squash => "squash",
                MergeMethod::MergeCommit => "merge",
                MergeMethod::Rebase => "rebase",
                MergeMethod::FastForward => {
                    let (pr, merged) = self.read_pr(repo, merge.number, class).await?;
                    if let Some(merged) = merged.filter(|_| pr.state == "MERGED") {
                        return Ok(merged);
                    }
                    if pr.head_ref_oid != merge.head_sha {
                        return Err(ScmError::HeadChanged);
                    }
                    return match self
                        .move_branch(repo, &pr.base_ref_name, &merge.head_sha, false, class)
                        .await
                    {
                        Ok(()) => Ok(merge.head_sha.clone()),
                        Err(ScmError::Validation(message)) => Err(ScmError::MergeRejected(message)),
                        Err(err) => Err(err),
                    };
                }
            };
            let body = json!({
                "sha": merge.head_sha,
                "merge_method": method,
                "commit_title": merge.title,
                "commit_message": merge.message,
            });
            let rest = format!("/pulls/{}/merge", merge.number);
            let answer = self
                .rest(repo, class, Method::PUT, &rest, Some(body))
                .await?;
            match answer.status {
                StatusCode::OK => Ok(answer.json::<Merged>()?.sha),
                StatusCode::CONFLICT => Err(ScmError::HeadChanged),
                StatusCode::METHOD_NOT_ALLOWED => {
                    let (pr, merged) = self.read_pr(repo, merge.number, class).await?;
                    if let Some(merged) = merged.filter(|_| pr.state == "MERGED") {
                        return Ok(merged);
                    }
                    let message = answer.message();
                    if message.contains("in progress") {
                        Err(ScmError::MergeInProgress)
                    } else {
                        Err(ScmError::MergeRejected(message))
                    }
                }
                _ => Err(unexpected(&answer)),
            }
        })
    }

    fn set_status<'a>(
        &'a self,
        repo: &'a RepoRef,
        status: &'a StatusUpdate,
        class: JobClass,
    ) -> ScmFuture<'a, ()> {
        Box::pin(async move {
            let state = match status.status {
                CommitStatus::Pending => "pending",
                CommitStatus::Success => "success",
                CommitStatus::Failure => "failure",
                CommitStatus::Error => "error",
            };
            // GitHub refuses a description of more than 140 characters.
            let description: String = status.description.chars().take(140).collect();
            let body = json!({
                "state": state,
                "context": status.context,
                "description": description,
                "target_url": status.details_url,
            });
            let rest = format!("/statuses/{}", encode(&status.sha));
            let answer = self
                .rest(repo, class, Method::POST, &rest, Some(body))
                .await?;
            expect(answer, &[StatusCode::CREATED]).map(drop)
        })
    }

    fn create_comment<'a>(
        &'a self,
        repo: &'a RepoRef,
        number: u64,
        body: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, String> {
        Box::pin(async move {
            #[derive(Deserialize)]
            struct Comment {
                id: u64,
            }
            let rest = format!("/issues/{number}/comments");
            let answer = self
                .rest(
                    repo,
                    class,
                    Method::POST,
                    &rest,
                    Some(json!({ "body": body })),
                )
                .await?;
            let answer = expect(answer, &[StatusCode::CREATED])?;
            Ok(answer.json::<Comment>()?.id.to_string())
        })
    }

    fn update_comment<'a>(
        &'a self,
        repo: &'a RepoRef,
        comment_id: &'a str,
        body: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, ()> {
        Box::pin(async move {
            let rest = format!("/issues/comments/{}", encode(comment_id));
            let answer = self
                .rest(
                    repo,
                    class,
                    Method::PATCH,
                    &rest,
                    Some(json!({ "body": body })),
                )
                .await?;
            expect(answer, &[StatusCode::OK]).map(drop)
        })
    }

    fn add_label<'a>(
        &'a self,
        repo: &'a RepoRef,
        number: u64,
        label: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, ()> {
        Box::pin(async move {
            let rest = format!("/issues/{number}/labels");
            let body = json!({ "labels": [label] });
            let answer = self
                .rest(repo, class, Method::POST, &rest, Some(body))
                .await?;
            expect(answer, &[StatusCode::OK]).map(drop)
        })
    }

    fn remove_label<'a>(
        &'a self,
        repo: &'a RepoRef,
        number: u64,
        label: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, ()> {
        Box::pin(async move {
            // A label name can hold `/`, which is not a path separator here.
            let rest = format!(
                "/issues/{number}/labels/{}",
                encode(label).replace('/', "%2F")
            );
            let answer = self.rest(repo, class, Method::DELETE, &rest, None).await?;
            expect(answer, &[StatusCode::OK, StatusCode::NOT_FOUND]).map(drop)
        })
    }

    fn open_draft_pr<'a>(
        &'a self,
        repo: &'a RepoRef,
        head: &'a str,
        base: &'a str,
        title: &'a str,
        body: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, u64> {
        Box::pin(async move {
            #[derive(Deserialize)]
            struct Pull {
                number: u64,
            }
            let rest = format!(
                "/pulls?state=open&head={}:{}&base={}",
                encode(&repo.namespace),
                encode(head),
                encode(base)
            );
            let open = self.rest(repo, class, Method::GET, &rest, None).await?;
            let open: Vec<Pull> = expect(open, &[StatusCode::OK])?.json()?;
            if let Some(pull) = open.first() {
                return Ok(pull.number);
            }
            let body =
                json!({ "title": title, "head": head, "base": base, "body": body, "draft": true });
            let answer = self
                .rest(repo, class, Method::POST, "/pulls", Some(body))
                .await?;
            Ok(expect(answer, &[StatusCode::CREATED])?
                .json::<Pull>()?
                .number)
        })
    }

    fn close_pr<'a>(
        &'a self,
        repo: &'a RepoRef,
        number: u64,
        class: JobClass,
    ) -> ScmFuture<'a, ()> {
        Box::pin(async move {
            let rest = format!("/pulls/{number}");
            let body = json!({ "state": "closed" });
            let answer = self
                .rest(repo, class, Method::PATCH, &rest, Some(body))
                .await?;
            expect(answer, &[StatusCode::OK]).map(drop)
        })
    }

    /// The pages of `GET /app/hook/deliveries`, newest first, until an
    /// attempt older than `since`.
    fn recent_deliveries(
        &self,
        since: OffsetDateTime,
        _class: JobClass,
    ) -> ScmFuture<'_, Vec<RecentDelivery>> {
        Box::pin(async move {
            #[derive(Deserialize)]
            struct Attempt {
                id: u64,
                guid: String,
                delivered_at: String,
            }
            let mut path = Some("/app/hook/deliveries?per_page=100".to_owned());
            let mut recent = Vec::new();
            while let Some(page) = path.take() {
                let (answer, next) = self.send_as_app(Method::GET, &page).await?;
                let attempts: Vec<Attempt> = expect(answer, &[StatusCode::OK])?.json()?;
                let mut older = false;
                for attempt in attempts {
                    let at = OffsetDateTime::parse(&attempt.delivered_at, &Rfc3339)
                        .map_err(|err| ScmError::Protocol(err.to_string()))?;
                    if at < since {
                        older = true;
                        break;
                    }
                    recent.push(RecentDelivery {
                        delivery_id: attempt.guid,
                        attempt_id: attempt.id.to_string(),
                    });
                }
                if !older {
                    path = next;
                }
            }
            Ok(recent)
        })
    }

    fn redeliver<'a>(&'a self, attempt_id: &'a str, _class: JobClass) -> ScmFuture<'a, ()> {
        Box::pin(async move {
            let path = format!("/app/hook/deliveries/{}/attempts", encode(attempt_id));
            let (answer, _) = self.send_as_app(Method::POST, &path).await?;
            expect(answer, &[StatusCode::ACCEPTED]).map(drop)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_merge_state_of_github_becomes_a_mergeability() {
        let cases = [
            ("MERGEABLE", "CLEAN", Mergeability::Mergeable),
            ("MERGEABLE", "HAS_HOOKS", Mergeability::Mergeable),
            ("MERGEABLE", "UNSTABLE", Mergeability::Mergeable),
            ("MERGEABLE", "BEHIND", Mergeability::Behind),
            ("CONFLICTING", "DIRTY", Mergeability::Conflict),
            ("CONFLICTING", "BLOCKED", Mergeability::Conflict),
            ("MERGEABLE", "BLOCKED", Mergeability::Blocked),
            ("MERGEABLE", "DRAFT", Mergeability::Blocked),
            ("UNKNOWN", "UNKNOWN", Mergeability::Unknown),
        ];
        for (mergeable, status, expected) in cases {
            assert_eq!(
                mergeability(mergeable, status),
                expected,
                "{mergeable} {status}"
            );
        }
    }

    #[test]
    fn checks_and_statuses_become_check_reports() {
        let rollup: Rollup = serde_json::from_value(json!({ "contexts": { "nodes": [
            { "__typename": "CheckRun", "name": "build", "status": "COMPLETED",
              "conclusion": "STARTUP_FAILURE", "detailsUrl": "https://ci/1" },
            { "__typename": "CheckRun", "name": "lint", "status": "IN_PROGRESS",
              "conclusion": null, "detailsUrl": null },
            { "__typename": "StatusContext", "context": "deploy", "state": "EXPECTED",
              "targetUrl": null },
            { "__typename": "StatusContext", "context": "cla", "state": "SUCCESS",
              "targetUrl": null },
        ]}}))
        .unwrap();
        let reports = check_reports(Some(rollup));
        let summary: Vec<_> = reports
            .iter()
            .map(|r| (r.name.as_str(), r.kind, r.conclusion))
            .collect();
        assert_eq!(
            summary,
            [
                ("build", CheckKind::Check, Some(CheckConclusion::Failure)),
                ("lint", CheckKind::Check, None),
                ("deploy", CheckKind::Status, None),
                ("cla", CheckKind::Status, Some(CheckConclusion::Success)),
            ]
        );
    }

    #[test]
    fn a_missing_permission_of_the_app_says_so() {
        let answer = |message: &str| Answer {
            status: StatusCode::FORBIDDEN,
            body: json!({ "message": message }).to_string().into_bytes(),
        };
        let missing = unexpected(&answer("Resource not accessible by integration"));
        let ScmError::Forbidden(message) = missing else {
            panic!("{missing:?}");
        };
        assert!(
            message.starts_with("the GitHub App lacks a permission"),
            "{message}"
        );
        assert!(message.ends_with("HTTP 403 Forbidden: Resource not accessible by integration"));
        let rules = unexpected(&answer("Protected branch update failed"));
        assert_eq!(
            rules,
            ScmError::Forbidden("HTTP 403 Forbidden: Protected branch update failed".into())
        );
    }

    #[test]
    fn a_private_repository_on_github_free_has_no_rules() {
        let answer = |status, message: &str| Answer {
            status,
            body: json!({ "message": message }).to_string().into_bytes(),
        };
        let upgrade =
            "Upgrade to GitHub Pro or make this repository public to enable this feature.";
        assert!(needs_paid_plan(&answer(StatusCode::FORBIDDEN, upgrade)));
        assert!(!needs_paid_plan(&answer(StatusCode::OK, upgrade)));
        let other = answer(
            StatusCode::FORBIDDEN,
            "Resource not accessible by integration",
        );
        assert!(!needs_paid_plan(&other));
    }

    #[test]
    fn a_path_keeps_its_slashes_and_encodes_the_rest() {
        assert_eq!(encode("cihub-temp/main/12"), "cihub-temp/main/12");
        assert_eq!(encode("a b#?&é"), "a%20b%23%3F%26%C3%A9");
    }
}

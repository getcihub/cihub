//! The `Scm` trait on GitHub: reads in GraphQL, where one query reads 50 PRs,
//! and writes in REST. Each value of GitHub becomes a neutral value here
//! (`specs/GITHUB.md`).

use std::collections::BTreeSet;
use std::fmt::Write;

use cihub_domain::{
    CheckConclusion, CheckKind, CheckReport, CheckStatus, JobClass, MembershipRole, MergeMethod,
    Mergeability, Permission, PrState, ReviewDecision,
};
use cihub_scm::{
    CommitStatus, ConnectionRef, MAX_OPEN_PRS, MAX_PRS_PER_READ, MergeIntoRef, PrMerge, PrSnapshot,
    RecentDelivery, RepoRef, RequiredChecks, Scm, ScmAccount, ScmError, ScmFuture, ScmInstallation,
    ScmMember, ScmPersonReach, ScmPersonToken, ScmRepo, ScmUser, SetupFinding, SignInCode,
    SignInRequest, SignedIn, StatusUpdate,
};
use reqwest::{Method, StatusCode};
use secrecy::SecretString;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::GithubApp;
use crate::client::{Answer, JSON, Request};
use crate::sign_in::Installation;
use crate::translate::{check_run_status, status_report};

const CHECKS_FRAGMENT: &str = "
fragment checks on StatusCheckRollup {
  contexts(first: 100) {
    nodes {
      __typename
      ... on CheckRun { name status conclusion detailsUrl startedAt completedAt }
      ... on StatusContext { context state targetUrl createdAt }
    }
  }
}";

const PR_FIELDS: &str = "
fragment pr on PullRequest {
  number url state isDraft title body headRefOid baseRefName headRefName isCrossRepository createdAt
  mergeable mergeStateStatus reviewDecision
  latestOpinionatedReviews(first: 100) { nodes { state author { login } } }
  reviewRequests(first: 100) {
    nodes { requestedReviewer { ... on Actor { login } ... on Team { combinedSlug } } }
  }
  additions deletions changedFiles
  mergeCommit { oid }
  author { login ... on User { databaseId } ... on Bot { databaseId } }
  labels(first: 100) { nodes { name } }
  statusCheckRollup { ...checks }
}";

const THREADS_QUERY: &str = "
query($owner: String!, $name: String!, $number: Int!) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) { reviewThreads(first: 100) { nodes { isResolved path } } }
  }
}";

/// GitHub lists 3 000 files of a PR at most, 100 per page.
const FILE_PAGES: u32 = 30;

/// The pages of 100 open PRs that `open_pull_requests` reads at most.
const OPEN_PR_PAGES: usize = MAX_OPEN_PRS / 100;

const CHECKS_QUERY: &str = "
query($owner: String!, $name: String!, $sha: GitObjectID!) {
  repository(owner: $owner, name: $name) {
    object(oid: $sha) { ... on Commit { statusCheckRollup { ...checks } } }
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
    url: String,
    state: String,
    is_draft: bool,
    title: String,
    body: String,
    head_ref_oid: String,
    base_ref_name: String,
    head_ref_name: String,
    is_cross_repository: bool,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    mergeable: String,
    merge_state_status: String,
    review_decision: Option<String>,
    latest_opinionated_reviews: Nodes<Review>,
    review_requests: Nodes<ReviewRequest>,
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
struct Review {
    state: String,
    author: Option<Login>,
}

#[derive(Deserialize)]
struct Login {
    login: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewRequest {
    requested_reviewer: Option<Reviewer>,
}

/// A user, a bot or a team: a team is named `<org>/<team>`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Reviewer {
    login: Option<String>,
    combined_slug: Option<String>,
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
        #[serde(with = "time::serde::rfc3339::option")]
        started_at: Option<OffsetDateTime>,
        #[serde(with = "time::serde::rfc3339::option")]
        completed_at: Option<OffsetDateTime>,
    },
    #[serde(rename_all = "camelCase")]
    StatusContext {
        context: String,
        state: String,
        target_url: Option<String>,
        #[serde(with = "time::serde::rfc3339::option")]
        created_at: Option<OffsetDateTime>,
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

/// `specs/GITHUB.md`: `UNSTABLE` (a check that is not required failed) and
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
        _ => CheckConclusion::Failure,
    }
}

fn decode<T: DeserializeOwned>(value: &Value) -> Result<T, ScmError> {
    T::deserialize(value).map_err(|err| ScmError::Protocol(err.to_string()))
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
                started_at,
                completed_at,
            } => {
                let status = check_run_status(&status);
                CheckReport {
                    name,
                    kind: CheckKind::Check,
                    status,
                    conclusion: (status == CheckStatus::Completed)
                        .then(|| conclusion.as_deref().map(check_conclusion))
                        .flatten(),
                    details_url,
                    started_at,
                    completed_at,
                }
            }
            Context::StatusContext {
                context,
                state,
                target_url,
                created_at,
            } => status_report(context, &state, target_url, created_at),
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
        let reviewers = |state: &str| -> BTreeSet<String> {
            self.latest_opinionated_reviews
                .nodes
                .iter()
                .flatten()
                .filter(|review| review.state == state)
                .filter_map(|review| review.author.as_ref().map(|a| a.login.clone()))
                .collect()
        };
        let approved_by = reviewers("APPROVED");
        let changes_requested_by = reviewers("CHANGES_REQUESTED");
        let review_requested = self
            .review_requests
            .nodes
            .into_iter()
            .flatten()
            .filter_map(|request| request.requested_reviewer)
            .filter_map(|reviewer| reviewer.login.or(reviewer.combined_slug))
            .collect();
        Ok(PrSnapshot {
            number: self.number,
            url: self.url,
            state: pr_state(&self.state)?,
            head_sha: self.head_ref_oid,
            base_branch: self.base_ref_name,
            head_branch: self.head_ref_name,
            from_fork: self.is_cross_repository,
            created_at: self.created_at,
            draft: self.is_draft,
            title: self.title,
            body: self.body,
            labels: self
                .labels
                .nodes
                .into_iter()
                .flatten()
                .map(|label| label.name)
                .collect(),
            mergeability: mergeability(&self.mergeable, &self.merge_state_status),
            review_decision: review_decision(self.review_decision.as_deref()),
            approved_by,
            changes_requested_by,
            review_requested,
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
pub(crate) fn encode(text: &str) -> String {
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

fn unexpected(answer: &Answer) -> ScmError {
    let message = answer.message();
    match answer.status {
        StatusCode::NOT_FOUND => ScmError::NotFound(message),
        // GitHub says this when the App lacks the permission of the call,
        // or when the installation does not cover the repository.
        StatusCode::FORBIDDEN if message.contains("not accessible by integration") => {
            ScmError::Forbidden(format!(
                "the GitHub App lacks a permission for this call (specs/GITHUB.md), or its \
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

pub(crate) fn expect(answer: Answer, statuses: &[StatusCode]) -> Result<Answer, ScmError> {
    if statuses.contains(&answer.status) {
        Ok(answer)
    } else {
        Err(unexpected(&answer))
    }
}

#[derive(Deserialize)]
struct DeliveryAttempt {
    id: u64,
    guid: String,
    delivered_at: String,
    status_code: u16,
}

/// The attempts of a page delivered at `since` or later, and whether the page
/// reaches an older one: the pages after it hold only older attempts.
fn recent_attempts(
    attempts: Vec<DeliveryAttempt>,
    since: OffsetDateTime,
) -> Result<(Vec<RecentDelivery>, bool), ScmError> {
    let mut recent = Vec::new();
    for attempt in attempts {
        let at = OffsetDateTime::parse(&attempt.delivered_at, &Rfc3339)
            .map_err(|err| ScmError::Protocol(err.to_string()))?;
        if at < since {
            return Ok((recent, true));
        }

        recent.push(RecentDelivery {
            delivery_id: attempt.guid,
            attempt_id: attempt.id.to_string(),
            is_delivered: (200..300).contains(&attempt.status_code),
        });
    }

    Ok((recent, false))
}

#[derive(Deserialize)]
struct AsyncMerge {
    status: String,
    details: AsyncMergeDetails,
}

#[derive(Deserialize)]
struct AsyncMergeDetails {
    sha: Option<String>,
    message: Option<String>,
}

impl AsyncMerge {
    fn outcome(self) -> Result<String, ScmError> {
        let message = self.details.message.unwrap_or_default();
        match (self.status.as_str(), self.details.sha) {
            ("merged", Some(sha)) => Ok(sha),
            ("pending", _) => Err(ScmError::MergeInProgress),
            ("failed", _) => Err(ScmError::MergeRejected(message)),
            (status, _) => Err(ScmError::Protocol(format!(
                "asynchronous merge `{status}`: {message}"
            ))),
        }
    }
}

#[derive(Deserialize)]
struct BranchRule {
    #[serde(rename = "type")]
    kind: String,
    ruleset_id: Option<u64>,
    #[serde(default)]
    parameters: Value,
}

/// The classic branch protection of a branch, next to its rules.
#[derive(Deserialize)]
struct Protection {
    #[serde(default)]
    strict: bool,
    #[serde(default)]
    contexts: Vec<String>,
}

impl GithubApp {
    async fn branch_rules(
        &self,
        repo: &RepoRef,
        branch: &str,
        class: JobClass,
    ) -> Result<Vec<BranchRule>, ScmError> {
        let rest = format!("/rules/branches/{}?per_page=100", encode(branch));
        let answer = self.rest(repo, class, Method::GET, &rest, None).await?;
        if needs_paid_plan(&answer) {
            return Ok(Vec::new());
        }

        expect(answer, &[StatusCode::OK])?.json()
    }

    /// What `rules` and the classic protection of `branch` require.
    async fn required_checks_of(
        &self,
        repo: &RepoRef,
        branch: &str,
        class: JobClass,
        rules: &[BranchRule],
    ) -> Result<RequiredChecks, ScmError> {
        let mut required = RequiredChecks::default();
        for rule in rules {
            if rule.kind != "required_status_checks" {
                continue;
            }

            let checks = rule.parameters["required_status_checks"].as_array();
            let contexts = checks
                .into_iter()
                .flatten()
                .filter_map(|check| check["context"].as_str());
            required.names.extend(contexts.map(str::to_owned));

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
    }

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

    async fn rest_json<T: DeserializeOwned>(
        &self,
        repo: &RepoRef,
        class: JobClass,
        method: Method,
        rest: &str,
        body: Option<Value>,
        status: StatusCode,
    ) -> Result<T, ScmError> {
        let answer = self.rest(repo, class, method, rest, body).await?;
        expect(answer, &[status])?.json()
    }

    async fn rest_ok(
        &self,
        repo: &RepoRef,
        class: JobClass,
        method: Method,
        rest: &str,
        body: Option<Value>,
        status: StatusCode,
    ) -> Result<(), ScmError> {
        let answer = self.rest(repo, class, method, rest, body).await?;
        expect(answer, &[status]).map(drop)
    }

    async fn merge_pr_async(
        &self,
        repo: &RepoRef,
        number: u64,
        mut body: Value,
        class: JobClass,
    ) -> Result<String, ScmError> {
        body["merge_action"] = json!("direct_merge");
        let rest = format!("/pulls/{number}/merge-async");
        let answer = self
            .rest(repo, class, Method::PUT, &rest, Some(body))
            .await?;
        match answer.status {
            StatusCode::OK | StatusCode::ACCEPTED => answer.json::<AsyncMerge>()?.outcome(),
            // Another request of the PR is still pending.
            StatusCode::CONFLICT => Err(ScmError::MergeInProgress),
            // Closed, draft: the PR cannot merge.
            StatusCode::BAD_REQUEST => Err(ScmError::MergeRejected(answer.message())),
            _ => Err(unexpected(&answer)),
        }
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
    ) -> Result<Vec<GraphqlPr>, ScmError> {
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
            "query($owner: String!, $name: String!) {{\n  repository(owner: $owner, name: $name) {{\n{fields}  }}\n}}{CHECKS_FRAGMENT}{PR_FIELDS}"
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
            prs.push(decode(pr)?);
        }

        Ok(prs)
    }

    async fn read_pr(
        &self,
        repo: &RepoRef,
        number: u64,
        class: JobClass,
    ) -> Result<GraphqlPr, ScmError> {
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
        // GitHub answers 409 "Git Repository is empty" for a repository
        // without a commit: it has no branch.
        if matches!(answer.status, StatusCode::NOT_FOUND | StatusCode::CONFLICT) {
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
            && answer.message().contains("Reference does not exist")
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
                .map(GraphqlPr::snapshot)
                .collect()
        })
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
            let query = format!("{CHECKS_QUERY}{CHECKS_FRAGMENT}");
            let data = self
                .query(&repo.connection, class, &query, variables)
                .await?;
            let object = data
                .pointer("/repository/object")
                .filter(|object| !object.is_null())
                .ok_or_else(|| ScmError::NotFound(format!("commit {sha}")))?;
            let commit: Commit = decode(object)?;
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
            let rules = self.branch_rules(repo, branch, class).await?;
            self.required_checks_of(repo, branch, class, &rules).await
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
    ) -> ScmFuture<'a, Permission> {
        Box::pin(async move {
            #[derive(Deserialize)]
            struct Answer {
                permission: String,
            }

            let key = (
                repo.connection.id,
                format!("{}/{}", repo.namespace, repo.name),
                user.login.clone(),
            );
            if let Some(permission) = self.kept_permission(&key) {
                return Ok(permission);
            }

            let rest = format!("/collaborators/{}/permission", encode(&user.login));
            let answer = self.rest(repo, class, Method::GET, &rest, None).await?;
            let permission = if answer.status == StatusCode::NOT_FOUND {
                Permission::None
            } else {
                let answer = expect(answer, &[StatusCode::OK])?;
                match answer.json::<Answer>()?.permission.as_str() {
                    "admin" => Permission::Admin,
                    "write" | "maintain" => Permission::Write,
                    "read" | "triage" => Permission::Read,
                    _ => Permission::None,
                }
            };
            self.keep_permission(key, permission);

            Ok(permission)
        })
    }

    fn is_ancestor<'a>(
        &'a self,
        repo: &'a RepoRef,
        ancestor: &'a str,
        descendant: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, bool> {
        Box::pin(async move {
            #[derive(Deserialize)]
            struct Comparison {
                status: String,
            }

            let rest = format!(
                "/compare/{}...{}?per_page=1",
                encode(ancestor),
                encode(descendant)
            );
            let answer = self.rest(repo, class, Method::GET, &rest, None).await?;
            if answer.status == StatusCode::NOT_FOUND {
                return Err(ScmError::NotFound(format!(
                    "commit {ancestor} or {descendant}"
                )));
            }

            let status = expect(answer, &[StatusCode::OK])?
                .json::<Comparison>()?
                .status;
            Ok(matches!(status.as_str(), "ahead" | "identical"))
        })
    }

    fn open_pull_requests<'a>(
        &'a self,
        repo: &'a RepoRef,
        base_branch: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<u64>> {
        Box::pin(async move {
            #[derive(Deserialize)]
            struct Pull {
                number: u64,
            }
            let mut numbers = Vec::new();
            for page in 1..=OPEN_PR_PAGES {
                let rest = format!(
                    "/pulls?state=open&base={}&sort=created&direction=asc&per_page=100&page={page}",
                    encode(base_branch)
                );
                let pulls: Vec<Pull> = self
                    .rest_json(repo, class, Method::GET, &rest, None, StatusCode::OK)
                    .await?;
                let last = pulls.len() < 100;

                numbers.extend(pulls.into_iter().map(|pull| pull.number));
                if last {
                    break;
                }
            }

            Ok(numbers)
        })
    }

    fn pr_files<'a>(
        &'a self,
        repo: &'a RepoRef,
        number: u64,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<String>> {
        Box::pin(async move {
            #[derive(Deserialize)]
            struct File {
                filename: String,
            }
            let mut paths = Vec::new();
            for page in 1..=FILE_PAGES {
                let rest = format!("/pulls/{number}/files?per_page=100&page={page}");
                let files: Vec<File> = self
                    .rest_json(repo, class, Method::GET, &rest, None, StatusCode::OK)
                    .await?;
                let last = files.len() < 100;

                paths.extend(files.into_iter().map(|file| file.filename));
                if last {
                    break;
                }
            }

            Ok(paths)
        })
    }

    fn unresolved_threads<'a>(
        &'a self,
        repo: &'a RepoRef,
        number: u64,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<String>> {
        Box::pin(async move {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Thread {
                is_resolved: bool,
                path: String,
            }

            let variables = json!({ "owner": repo.namespace, "name": repo.name, "number": number });
            let data = self
                .query(&repo.connection, class, THREADS_QUERY, variables)
                .await?;
            let threads = data
                .pointer("/repository/pullRequest/reviewThreads")
                .filter(|threads| !threads.is_null())
                .ok_or_else(|| ScmError::NotFound(format!("PR {number}")))?;
            let threads: Nodes<Thread> = decode(threads)?;
            Ok(threads
                .nodes
                .into_iter()
                .flatten()
                .filter(|thread| !thread.is_resolved)
                .map(|thread| thread.path)
                .collect())
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
                    let pr = self.read_pr(repo, merge.number, class).await?;
                    if pr.state == "MERGED"
                        && let Some(merged) = pr.merge_commit
                    {
                        return Ok(merged.oid);
                    }

                    if pr.head_ref_oid != merge.head_sha {
                        return Err(ScmError::HeadChanged);
                    }

                    // GitHub marks the PR merged once its head is in the base.
                    return match self
                        .move_branch(repo, &pr.base_ref_name, &merge.tested_sha, false, class)
                        .await
                    {
                        Ok(()) => Ok(merge.tested_sha.clone()),
                        Err(ScmError::Validation(message))
                            if message.contains("not a fast forward") =>
                        {
                            Err(ScmError::NotFastForward)
                        }
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
                .rest(repo, class, Method::PUT, &rest, Some(body.clone()))
                .await?;
            match answer.status {
                StatusCode::OK => Ok(answer.json::<Merged>()?.sha),
                StatusCode::CONFLICT => Err(ScmError::HeadChanged),
                // GitHub merges a stacked PR only through its asynchronous
                // endpoint, which GitHub Enterprise Server does not have.
                StatusCode::FORBIDDEN
                    if answer.message().contains("asynchronous merge endpoint") =>
                {
                    self.merge_pr_async(repo, merge.number, body, class).await
                }
                StatusCode::METHOD_NOT_ALLOWED => {
                    let pr = self.read_pr(repo, merge.number, class).await?;
                    if pr.state == "MERGED"
                        && let Some(merged) = pr.merge_commit
                    {
                        return Ok(merged.oid);
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
            self.rest_ok(
                repo,
                class,
                Method::POST,
                &rest,
                Some(body),
                StatusCode::CREATED,
            )
            .await
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
            let comment: Comment = self
                .rest_json(
                    repo,
                    class,
                    Method::POST,
                    &rest,
                    Some(json!({ "body": body })),
                    StatusCode::CREATED,
                )
                .await?;
            Ok(comment.id.to_string())
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
            self.rest_ok(
                repo,
                class,
                Method::PATCH,
                &rest,
                Some(json!({ "body": body })),
                StatusCode::OK,
            )
            .await
        })
    }

    fn acknowledge_comment<'a>(
        &'a self,
        repo: &'a RepoRef,
        comment_id: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, ()> {
        Box::pin(async move {
            // `200` when the reaction is already there.
            let rest = format!("/issues/comments/{}/reactions", encode(comment_id));
            let body = json!({ "content": "+1" });
            let answer = self
                .rest(repo, class, Method::POST, &rest, Some(body))
                .await?;
            expect(answer, &[StatusCode::OK, StatusCode::CREATED]).map(drop)
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
            self.rest_ok(repo, class, Method::POST, &rest, Some(body), StatusCode::OK)
                .await
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
            let open: Vec<Pull> = self
                .rest_json(repo, class, Method::GET, &rest, None, StatusCode::OK)
                .await?;
            if let Some(pull) = open.first() {
                return Ok(pull.number);
            }

            let body =
                json!({ "title": title, "head": head, "base": base, "body": body, "draft": true });
            let opened: Pull = self
                .rest_json(
                    repo,
                    class,
                    Method::POST,
                    "/pulls",
                    Some(body),
                    StatusCode::CREATED,
                )
                .await?;
            Ok(opened.number)
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
            self.rest_ok(
                repo,
                class,
                Method::PATCH,
                &rest,
                Some(body),
                StatusCode::OK,
            )
            .await
        })
    }

    fn recent_deliveries(
        &self,
        since: OffsetDateTime,
        _class: JobClass,
    ) -> ScmFuture<'_, Vec<RecentDelivery>> {
        Box::pin(async move {
            let mut path = Some("/app/hook/deliveries?per_page=100".to_owned());
            let mut recent = Vec::new();
            while let Some(page) = path.take() {
                let (answer, next) = self.send_as_app(Method::GET, &page).await?;
                let attempts: Vec<DeliveryAttempt> = expect(answer, &[StatusCode::OK])?.json()?;
                let (page_recent, has_older) = recent_attempts(attempts, since)?;
                recent.extend(page_recent);

                if !has_older {
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

    fn installation<'a>(
        &'a self,
        external_id: &'a str,
        _class: JobClass,
    ) -> ScmFuture<'a, Option<ScmInstallation>> {
        Box::pin(async move {
            let path = format!("/app/installations/{}", encode(external_id));
            let (answer, _) = self.send_as_app(Method::GET, &path).await?;
            if answer.status == StatusCode::NOT_FOUND {
                return Ok(None);
            }

            let installation: Installation = expect(answer, &[StatusCode::OK])?.json()?;
            Ok(Some(installation.neutral()))
        })
    }

    fn installations(&self, _class: JobClass) -> ScmFuture<'_, Vec<ScmInstallation>> {
        Box::pin(async move {
            let mut path = Some("/app/installations?per_page=100".to_owned());
            let mut installations = Vec::new();
            while let Some(page) = path.take() {
                let (answer, next) = self.send_as_app(Method::GET, &page).await?;
                let listed: Vec<Installation> = expect(answer, &[StatusCode::OK])?.json()?;
                installations.extend(listed.iter().map(Installation::neutral));
                path = next;
            }

            Ok(installations)
        })
    }

    fn check_setup<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<SetupFinding>> {
        Box::pin(async move {
            #[derive(Deserialize)]
            struct Ruleset {
                #[serde(default)]
                bypass_actors: Vec<Exemption>,
            }

            #[derive(Deserialize)]
            struct Exemption {
                actor_id: Option<u64>,
                actor_type: String,
                bypass_mode: String,
            }

            let mut findings = Vec::new();
            let rules = self.branch_rules(repo, branch, class).await?;
            let updates: BTreeSet<u64> = rules
                .iter()
                .filter(|rule| rule.kind == "update")
                .filter_map(|rule| rule.ruleset_id)
                .collect();
            if updates.is_empty() {
                findings.push(SetupFinding::UpdateRestrictionMissing);
            }

            for id in updates {
                let rest = format!("/rulesets/{id}");
                let ruleset: Ruleset = self
                    .rest_json(repo, class, Method::GET, &rest, None, StatusCode::OK)
                    .await?;
                let exempt = ruleset.bypass_actors.iter().any(|actor| {
                    actor.actor_type == "Integration"
                        && actor.actor_id.map(|id| id.to_string()).as_deref() == Some(self.app_id())
                        && actor.bypass_mode == "exempt"
                });
                if !exempt {
                    findings.push(SetupFinding::AppNotExempt);
                    break;
                }
            }

            for test_branch in ["cihub-temp/check/1", "cihub-merge/check/1"] {
                if !self
                    .branch_rules(repo, test_branch, class)
                    .await?
                    .is_empty()
                {
                    findings.push(SetupFinding::TestBranchesNotExcluded);
                    break;
                }
            }

            let required = self.required_checks_of(repo, branch, class, &rules).await?;
            if required.up_to_date {
                findings.push(SetupFinding::UpToDateRequired);
            }

            Ok(findings)
        })
    }

    fn list_repos<'a>(
        &'a self,
        connection: &'a ConnectionRef,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<ScmRepo>> {
        Box::pin(async move {
            #[derive(Deserialize)]
            struct Page {
                total_count: usize,
                repositories: Vec<Repository>,
            }

            #[derive(Deserialize)]
            struct Repository {
                id: u64,
                name: String,
                owner: Owner,
                default_branch: String,
            }

            #[derive(Deserialize)]
            struct Owner {
                login: String,
            }

            let mut repos = Vec::new();
            for page in 1.. {
                let answer = self
                    .send(Request {
                        connection,
                        class,
                        method: Method::GET,
                        path: format!("/installation/repositories?per_page=100&page={page}"),
                        body: None,
                        accept: JSON,
                    })
                    .await?;
                let Page {
                    total_count,
                    repositories,
                } = expect(answer, &[StatusCode::OK])?.json()?;
                let last = repositories.is_empty();

                repos.extend(repositories.into_iter().map(|repo| ScmRepo {
                    external_id: repo.id.to_string(),
                    namespace: repo.owner.login,
                    name: repo.name,
                    default_branch: repo.default_branch,
                }));

                if last || repos.len() >= total_count {
                    break;
                }
            }

            Ok(repos)
        })
    }

    fn app_install_url(&self, _class: JobClass) -> ScmFuture<'_, String> {
        Box::pin(self.install_address())
    }

    fn branch_rules_url(&self, repo: &RepoRef) -> Option<String> {
        Some(format!(
            "{}/{}/{}/settings/rules",
            self.web_url, repo.namespace, repo.name
        ))
    }

    fn pr_url(&self, repo: &RepoRef, number: u64) -> String {
        format!(
            "{}/{}/{}/pull/{number}",
            self.web_url, repo.namespace, repo.name
        )
    }

    fn sign_in_url(&self, request: &SignInRequest) -> Option<String> {
        self.sign_in_address(request)
    }

    fn sign_in<'a>(
        &'a self,
        code: &'a SignInCode,
        _class: JobClass,
    ) -> ScmFuture<'a, (SignedIn, ScmPersonToken)> {
        Box::pin(self.sign_in_person(code))
    }

    fn refresh_person_token<'a>(
        &'a self,
        refresh: &'a SecretString,
        _class: JobClass,
    ) -> ScmFuture<'a, ScmPersonToken> {
        Box::pin(self.refresh_person(refresh))
    }

    fn person_reach<'a>(
        &'a self,
        token: &'a SecretString,
        _class: JobClass,
    ) -> ScmFuture<'a, ScmPersonReach> {
        Box::pin(self.reach_of_person(token))
    }

    fn person_permission<'a>(
        &'a self,
        token: &'a SecretString,
        repo: &'a RepoRef,
        _class: JobClass,
    ) -> ScmFuture<'a, Permission> {
        Box::pin(self.permission_of_person(token, repo))
    }

    fn person_role<'a>(
        &'a self,
        token: &'a SecretString,
        person: &'a ScmUser,
        account: &'a ScmAccount,
        _class: JobClass,
    ) -> ScmFuture<'a, Option<MembershipRole>> {
        Box::pin(self.role_in_account(token, person, account))
    }

    fn members<'a>(
        &'a self,
        connection: &'a ConnectionRef,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<ScmMember>> {
        Box::pin(self.account_members(connection, class))
    }
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

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
              "conclusion": "STARTUP_FAILURE", "detailsUrl": "https://ci/1",
              "startedAt": "2026-10-11T12:00:00Z", "completedAt": "2026-10-11T12:30:00Z" },
            { "__typename": "CheckRun", "name": "lint", "status": "IN_PROGRESS",
              "conclusion": null, "detailsUrl": null,
              "startedAt": "2026-10-11T12:00:00Z", "completedAt": null },
            { "__typename": "CheckRun", "name": "e2e", "status": "WAITING",
              "conclusion": null, "detailsUrl": null, "startedAt": null, "completedAt": null },
            { "__typename": "StatusContext", "context": "deploy", "state": "EXPECTED",
              "targetUrl": null, "createdAt": null },
            { "__typename": "StatusContext", "context": "cla", "state": "SUCCESS",
              "targetUrl": null, "createdAt": "2026-10-11T12:10:00Z" },
        ]}}))
        .unwrap();
        let reports = check_reports(Some(rollup));
        let summary: Vec<_> = reports
            .iter()
            .map(|r| (r.name.as_str(), r.kind, r.status, r.conclusion))
            .collect();
        let times: Vec<_> = reports
            .iter()
            .map(|r| (r.started_at, r.completed_at))
            .collect();

        assert_eq!(
            summary,
            [
                (
                    "build",
                    CheckKind::Check,
                    CheckStatus::Completed,
                    Some(CheckConclusion::Failure)
                ),
                ("lint", CheckKind::Check, CheckStatus::InProgress, None),
                ("e2e", CheckKind::Check, CheckStatus::Queued, None),
                ("deploy", CheckKind::Status, CheckStatus::Queued, None),
                (
                    "cla",
                    CheckKind::Status,
                    CheckStatus::Completed,
                    Some(CheckConclusion::Success)
                ),
            ]
        );
        assert_eq!(
            times,
            [
                (
                    Some(datetime!(2026-10-11 12:00 UTC)),
                    Some(datetime!(2026-10-11 12:30 UTC))
                ),
                (Some(datetime!(2026-10-11 12:00 UTC)), None),
                (None, None),
                (None, None),
                (None, Some(datetime!(2026-10-11 12:10 UTC))),
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

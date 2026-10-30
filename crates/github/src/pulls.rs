//! The pull requests: their read in GraphQL, where one query reads 50 PRs,
//! their files, threads and merge, and the draft PRs of CIHub.

use std::collections::BTreeSet;
use std::fmt::Write;

use cihub_domain::{JobClass, MergeMethod, Mergeability, PrState, ReviewDecision};
use cihub_scm::{
    BranchMove, DraftPr, MAX_OPEN_PRS, MAX_PRS_PER_READ, PrMerge, PrSnapshot, RepoRef, ScmError,
    ScmUser,
};
use reqwest::{Method, StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};
use time::OffsetDateTime;

use crate::GithubApp;
use crate::checks::{CHECKS_FRAGMENT, Rollup, check_reports};
use crate::client::{Answer, Auth, PER_PAGE, encode, repo_path};
use crate::git;
use crate::graphql::{Nodes, decode};

/// The GraphQL fragment of the fields of a PR that CIHub reads.
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

/// The GraphQL query of the review threads of a PR.
const THREADS_QUERY: &str = "
query($owner: String!, $name: String!, $number: Int!) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) { reviewThreads(first: 100) { nodes { isResolved path } } }
  }
}";

/// GitHub lists 3 000 files of a PR at most, 100 per page.
const FILE_PAGES: usize = 30;

/// The pages of open PRs that `open_pull_requests` reads at most.
const OPEN_PR_PAGES: usize = MAX_OPEN_PRS / PER_PAGE;

/// A PR as the GraphQL query reads it.
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

/// A commit, by its id.
#[derive(Deserialize)]
struct Oid {
    oid: String,
}

/// The author of a PR: a user or a bot.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphqlAuthor {
    login: String,
    database_id: Option<u64>,
}

/// A label of a PR.
#[derive(Deserialize)]
struct Label {
    name: String,
}

/// The latest review of a reviewer.
#[derive(Deserialize)]
struct Review {
    state: String,
    author: Option<Login>,
}

/// An actor, by its login.
#[derive(Deserialize)]
struct Login {
    login: String,
}

/// A request for a review.
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

/// The state of a PR.
fn pr_state(state: &str) -> Result<PrState, ScmError> {
    match state {
        "OPEN" => Ok(PrState::Open),
        "CLOSED" => Ok(PrState::Closed),
        "MERGED" => Ok(PrState::Merged),
        other => Err(ScmError::Protocol(format!("PR state {other}"))),
    }
}

/// `UNSTABLE` (a check that is not required failed) and `HAS_HOOKS` admit
/// the PR, as the branch protection of GitHub does.
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

/// The review decision of a PR.
fn review_decision(decision: Option<&str>) -> ReviewDecision {
    match decision {
        Some("APPROVED") => ReviewDecision::Approved,
        Some("CHANGES_REQUESTED") => ReviewDecision::ChangesRequested,
        Some("REVIEW_REQUIRED") => ReviewDecision::ReviewRequired,
        _ => ReviewDecision::None,
    }
}

impl GraphqlPr {
    /// The merge commit of the PR, if it is merged.
    fn merged_sha(&self) -> Option<String> {
        self.merge_commit
            .as_ref()
            .filter(|_| self.state == "MERGED")
            .map(|commit| commit.oid.clone())
    }

    /// The PR in the neutral terms of `cihub-scm`.
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
            is_from_fork: self.is_cross_repository,
            created_at: self.created_at,
            is_draft: self.is_draft,
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

/// The answer of the asynchronous merge endpoint.
#[derive(Deserialize)]
struct AsyncMerge {
    status: String,
    details: AsyncMergeDetails,
}

/// The commit or the reason of an asynchronous merge.
#[derive(Deserialize)]
struct AsyncMergeDetails {
    sha: Option<String>,
    message: Option<String>,
}

impl AsyncMerge {
    /// The merge commit, or the error of a merge that is pending or failed.
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

/// The PRs `numbers` of `repo`, in one GraphQL query; a PR that does not exist
/// is left out.
async fn read_prs(
    app: &GithubApp,
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
    let variables = json!({ "owner": repo.owner, "name": repo.name });
    let data = app
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

/// The PR `number` of `repo`.
async fn read_pr(
    app: &GithubApp,
    repo: &RepoRef,
    number: u64,
    class: JobClass,
) -> Result<GraphqlPr, ScmError> {
    read_prs(app, repo, &[number], class)
        .await?
        .pop()
        .ok_or_else(|| ScmError::NotFound(format!("PR {number}")))
}

/// The PRs `numbers` of `repo`.
pub(crate) async fn pull_requests(
    app: &GithubApp,
    repo: &RepoRef,
    numbers: &[u64],
    class: JobClass,
) -> Result<Vec<PrSnapshot>, ScmError> {
    read_prs(app, repo, numbers, class)
        .await?
        .into_iter()
        .map(GraphqlPr::snapshot)
        .collect()
}

/// The numbers of the open PRs on `base_branch`, the oldest first.
pub(crate) async fn open_pull_requests(
    app: &GithubApp,
    repo: &RepoRef,
    base_branch: &str,
    class: JobClass,
) -> Result<Vec<u64>, ScmError> {
    #[derive(Deserialize)]
    struct Pull {
        number: u64,
    }

    let rest = format!(
        "/pulls?state=open&base={}&sort=created&direction=asc",
        encode(base_branch)
    );
    let auth = Auth::Installation(&repo.connection, class);
    let pulls: Vec<Pull> = app
        .numbered_pages(auth, &repo_path(repo, &rest), OPEN_PR_PAGES)
        .await?;

    Ok(pulls.into_iter().map(|pull| pull.number).collect())
}

/// The paths of the files that PR `number` changes.
pub(crate) async fn pr_files(
    app: &GithubApp,
    repo: &RepoRef,
    number: u64,
    class: JobClass,
) -> Result<Vec<String>, ScmError> {
    #[derive(Deserialize)]
    struct File {
        filename: String,
    }

    let rest = format!("/pulls/{number}/files");
    let auth = Auth::Installation(&repo.connection, class);
    let files: Vec<File> = app
        .numbered_pages(auth, &repo_path(repo, &rest), FILE_PAGES)
        .await?;

    Ok(files.into_iter().map(|file| file.filename).collect())
}

/// The paths of the review threads of PR `number` that are not resolved.
pub(crate) async fn unresolved_threads(
    app: &GithubApp,
    repo: &RepoRef,
    number: u64,
    class: JobClass,
) -> Result<Vec<String>, ScmError> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Thread {
        is_resolved: bool,
        path: String,
    }

    let variables = json!({ "owner": repo.owner, "name": repo.name, "number": number });
    let data = app
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
}

/// Merges the PR with its method and gives the commit that lands on the
/// base.
pub(crate) async fn merge_pr(
    app: &GithubApp,
    repo: &RepoRef,
    merge: &PrMerge,
    class: JobClass,
) -> Result<String, ScmError> {
    #[derive(Deserialize)]
    struct Merged {
        sha: String,
    }

    let method = match merge.method {
        MergeMethod::Squash => "squash",
        MergeMethod::MergeCommit => "merge",
        MergeMethod::Rebase => "rebase",
        MergeMethod::FastForward => return fast_forward(app, repo, merge, class).await,
    };
    let body = json!({
        "sha": merge.head_sha,
        "merge_method": method,
        "commit_title": merge.title,
        "commit_message": merge.message,
    });
    let rest = format!("/pulls/{}/merge", merge.number);
    let answer = app
        .rest(repo, class, Method::PUT, &rest, Some(body.clone()))
        .await?;
    match answer.status {
        StatusCode::OK => Ok(answer.json::<Merged>()?.sha),
        StatusCode::CONFLICT => Err(ScmError::HeadChanged),
        // GitHub merges a stacked PR only through its asynchronous
        // endpoint, which GitHub Enterprise Server does not have.
        StatusCode::FORBIDDEN if answer.message().contains("asynchronous merge endpoint") => {
            merge_pr_async(app, repo, merge.number, body, class).await
        }
        StatusCode::METHOD_NOT_ALLOWED => {
            merge_refused(app, repo, merge.number, &answer, class).await
        }
        _ => Err(answer.error()),
    }
}

/// Moves the base of the PR to the tested commit. GitHub has no
/// fast-forward merge, but marks the PR merged once its head is in the base.
async fn fast_forward(
    app: &GithubApp,
    repo: &RepoRef,
    merge: &PrMerge,
    class: JobClass,
) -> Result<String, ScmError> {
    let pr = read_pr(app, repo, merge.number, class).await?;
    if let Some(sha) = pr.merged_sha() {
        return Ok(sha);
    }

    if pr.head_ref_oid != merge.head_sha {
        return Err(ScmError::HeadChanged);
    }

    let base = &pr.base_ref_name;
    let moved = git::set_branch(
        app,
        repo,
        base,
        &merge.tested_sha,
        BranchMove::FastForwardOnly,
        class,
    )
    .await;
    match moved {
        Ok(()) => Ok(merge.tested_sha.clone()),
        Err(ScmError::Validation(message)) => Err(ScmError::MergeRejected(message)),
        Err(err) => Err(err),
    }
}

/// The outcome of a merge that GitHub refuses with `405`: the commit of the
/// PR if it merged meanwhile, else the reason of the refusal.
async fn merge_refused(
    app: &GithubApp,
    repo: &RepoRef,
    number: u64,
    answer: &Answer,
    class: JobClass,
) -> Result<String, ScmError> {
    let pr = read_pr(app, repo, number, class).await?;
    if let Some(sha) = pr.merged_sha() {
        return Ok(sha);
    }

    let message = answer.message();
    if message.contains("in progress") {
        Err(ScmError::MergeInProgress)
    } else {
        Err(ScmError::MergeRejected(message))
    }
}

/// Merges the PR through the asynchronous merge endpoint of GitHub.
async fn merge_pr_async(
    app: &GithubApp,
    repo: &RepoRef,
    number: u64,
    mut body: Value,
    class: JobClass,
) -> Result<String, ScmError> {
    body["merge_action"] = json!("direct_merge");
    let rest = format!("/pulls/{number}/merge-async");
    let answer = app
        .rest(repo, class, Method::PUT, &rest, Some(body))
        .await?;
    match answer.status {
        StatusCode::OK | StatusCode::ACCEPTED => answer.json::<AsyncMerge>()?.outcome(),
        // Another request of the PR is still pending.
        StatusCode::CONFLICT => Err(ScmError::MergeInProgress),
        // Closed, draft: the PR cannot merge.
        StatusCode::BAD_REQUEST => Err(ScmError::MergeRejected(answer.message())),
        _ => Err(answer.error()),
    }
}

/// The PR `draft`, opened when none is open from its head to its base.
pub(crate) async fn open_draft_pr(
    app: &GithubApp,
    repo: &RepoRef,
    draft: &DraftPr,
    class: JobClass,
) -> Result<u64, ScmError> {
    #[derive(Deserialize)]
    struct Pull {
        number: u64,
    }

    let rest = format!(
        "/pulls?state=open&head={}:{}&base={}",
        encode(&repo.owner),
        encode(&draft.head),
        encode(&draft.base)
    );
    let open: Vec<Pull> = app.get_json(repo, class, &rest).await?;
    if let Some(pull) = open.first() {
        return Ok(pull.number);
    }

    let body = json!({
        "title": draft.title,
        "head": draft.head,
        "base": draft.base,
        "body": draft.body,
        "draft": true,
    });
    let answer = app
        .rest(repo, class, Method::POST, "/pulls", Some(body))
        .await?;
    let opened: Pull = answer
        .expect_status(&[StatusCode::CREATED])
        .map_err(draft_pr_refusal)?
        .json()?;

    Ok(opened.number)
}

/// The refusals of a draft PR that the queue acts on, which GitHub only
/// words in the message of a 422.
fn draft_pr_refusal(error: ScmError) -> ScmError {
    let ScmError::Validation(message) = &error else {
        return error;
    };
    let message = message.to_lowercase();

    if message.contains("no commits between") {
        ScmError::NothingToCompare
    } else if message.contains("draft") {
        ScmError::DraftPrsNotAllowed
    } else {
        error
    }
}

/// Closes PR `number`.
pub(crate) async fn close_pr(
    app: &GithubApp,
    repo: &RepoRef,
    number: u64,
    class: JobClass,
) -> Result<(), ScmError> {
    let rest = format!("/pulls/{number}");
    let body = json!({ "state": "closed" });
    let answer = app
        .rest(repo, class, Method::PATCH, &rest, Some(body))
        .await?;
    answer.expect_status(&[StatusCode::OK]).map(drop)
}

/// The page of PR `number`.
pub(crate) fn pr_url(app: &GithubApp, repo: &RepoRef, number: u64) -> String {
    format!("{}/{}/{}/pull/{number}", app.web_url, repo.owner, repo.name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_refusals_of_a_draft_pr_become_typed_errors() {
        let refusal = |message: &str| draft_pr_refusal(ScmError::Validation(message.into()));

        assert_eq!(
            refusal("Draft pull requests are not supported in this repository."),
            ScmError::DraftPrsNotAllowed
        );
        assert_eq!(
            refusal("No commits between main and cihub-merge/pr-1/run"),
            ScmError::NothingToCompare
        );
        assert_eq!(
            refusal("Validation Failed"),
            ScmError::Validation("Validation Failed".into())
        );
        assert_eq!(
            draft_pr_refusal(ScmError::NotFound("repo".into())),
            ScmError::NotFound("repo".into())
        );
    }

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
}

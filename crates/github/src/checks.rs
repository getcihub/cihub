//! The checks of a commit and the commit statuses that CIHub sets: the
//! check runs and statuses of GitHub as neutral check reports.

use cihub_domain::{CheckKind, CheckReport, CheckStatus, JobClass};
use cihub_scm::{CommitStatus, RepoRef, ScmError, StatusUpdate};
use reqwest::{Method, StatusCode};
use serde::Deserialize;
use serde_json::json;
use time::OffsetDateTime;

use crate::GithubApp;
use crate::client::encode;
use crate::graphql::{Nodes, decode};
use crate::translate::{check_conclusion, check_run_status, status_report};

/// The GraphQL fragment that reads the checks and the statuses of a commit.
pub(crate) const CHECKS_FRAGMENT: &str = "
fragment checks on StatusCheckRollup {
  contexts(first: 100) {
    nodes {
      __typename
      ... on CheckRun { name status conclusion detailsUrl startedAt completedAt }
      ... on StatusContext { context state targetUrl createdAt }
    }
  }
}";

/// The GraphQL query of the checks of one commit.
const CHECKS_QUERY: &str = "
query($owner: String!, $name: String!, $sha: GitObjectID!) {
  repository(owner: $owner, name: $name) {
    object(oid: $sha) { ... on Commit { statusCheckRollup { ...checks } } }
  }
}";

/// The checks and the statuses of a commit, in GraphQL.
#[derive(Deserialize)]
pub(crate) struct Rollup {
    contexts: Nodes<Context>,
}

/// A check run or a commit status of a rollup.
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

/// The check reports of a rollup; none without a rollup.
pub(crate) fn check_reports(rollup: Option<Rollup>) -> Vec<CheckReport> {
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

/// The checks and the statuses of the commit `sha`.
pub(crate) async fn checks(
    app: &GithubApp,
    repo: &RepoRef,
    sha: &str,
    class: JobClass,
) -> Result<Vec<CheckReport>, ScmError> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Commit {
        status_check_rollup: Option<Rollup>,
    }

    let variables = json!({ "owner": repo.owner, "name": repo.name, "sha": sha });
    let query = format!("{CHECKS_QUERY}{CHECKS_FRAGMENT}");
    let data = app
        .query(&repo.connection, class, &query, variables)
        .await?;
    let object = data
        .pointer("/repository/object")
        .filter(|object| !object.is_null())
        .ok_or_else(|| ScmError::NotFound(format!("commit {sha}")))?;
    let commit: Commit = decode(object)?;
    Ok(check_reports(commit.status_check_rollup))
}

/// Sets a commit status of CIHub on a commit.
pub(crate) async fn set_status(
    app: &GithubApp,
    repo: &RepoRef,
    status: &StatusUpdate,
    class: JobClass,
) -> Result<(), ScmError> {
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
    let answer = app
        .rest(repo, class, Method::POST, &rest, Some(body))
        .await?;
    answer.expect_status(&[StatusCode::CREATED]).map(drop)
}

#[cfg(test)]
mod tests {
    use cihub_domain::CheckConclusion;
    use time::macros::datetime;

    use super::*;

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
}

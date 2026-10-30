//! The comments and the labels of a PR, which GitHub keeps on its issue.

use cihub_domain::JobClass;
use cihub_scm::{RepoRef, ScmError};
use reqwest::{Method, StatusCode};
use serde::Deserialize;
use serde_json::json;

use crate::GithubApp;
use crate::client::encode;

/// Adds a comment to PR `number`, and gives its id.
pub(crate) async fn create_comment(
    app: &GithubApp,
    repo: &RepoRef,
    number: u64,
    body: &str,
    class: JobClass,
) -> Result<String, ScmError> {
    #[derive(Deserialize)]
    struct Comment {
        id: u64,
    }

    let rest = format!("/issues/{number}/comments");
    let body = json!({ "body": body });
    let answer = app
        .rest(repo, class, Method::POST, &rest, Some(body))
        .await?;
    let comment: Comment = answer.expect_status(&[StatusCode::CREATED])?.json()?;

    Ok(comment.id.to_string())
}

/// Replaces the text of the comment `comment_id`.
pub(crate) async fn update_comment(
    app: &GithubApp,
    repo: &RepoRef,
    comment_id: &str,
    body: &str,
    class: JobClass,
) -> Result<(), ScmError> {
    let rest = format!("/issues/comments/{}", encode(comment_id));
    let body = json!({ "body": body });
    let answer = app
        .rest(repo, class, Method::PATCH, &rest, Some(body))
        .await?;
    answer.expect_status(&[StatusCode::OK]).map(drop)
}

/// Adds the reaction `+1` to the comment `comment_id`.
pub(crate) async fn acknowledge_comment(
    app: &GithubApp,
    repo: &RepoRef,
    comment_id: &str,
    class: JobClass,
) -> Result<(), ScmError> {
    // `200` when the reaction is already there.
    let rest = format!("/issues/comments/{}/reactions", encode(comment_id));
    let body = json!({ "content": "+1" });
    let answer = app
        .rest(repo, class, Method::POST, &rest, Some(body))
        .await?;
    answer
        .expect_status(&[StatusCode::OK, StatusCode::CREATED])
        .map(drop)
}

/// Adds `label` to PR `number`.
pub(crate) async fn add_label(
    app: &GithubApp,
    repo: &RepoRef,
    number: u64,
    label: &str,
    class: JobClass,
) -> Result<(), ScmError> {
    let rest = format!("/issues/{number}/labels");
    let body = json!({ "labels": [label] });
    let answer = app
        .rest(repo, class, Method::POST, &rest, Some(body))
        .await?;
    answer.expect_status(&[StatusCode::OK]).map(drop)
}

/// Removes `label` from PR `number`; a label that is not there is not an error.
pub(crate) async fn remove_label(
    app: &GithubApp,
    repo: &RepoRef,
    number: u64,
    label: &str,
    class: JobClass,
) -> Result<(), ScmError> {
    let rest = format!(
        "/issues/{number}/labels/{}",
        encode(label).replace('/', "%2F")
    );
    let answer = app.rest(repo, class, Method::DELETE, &rest, None).await?;
    answer
        .expect_status(&[StatusCode::OK, StatusCode::NOT_FOUND])
        .map(drop)
}

//! The branches and the commits of a repository, and the content of its
//! files.

use cihub_domain::JobClass;
use cihub_scm::{BranchMove, MergeIntoRef, RepoRef, ScmError};
use reqwest::{Method, StatusCode};
use serde::Deserialize;
use serde_json::json;

use crate::GithubApp;
use crate::client::{Auth, RAW, encode, repo_path};

/// The commit at the head of `branch`, or `None` when the branch does not
/// exist.
pub(crate) async fn branch_head(
    app: &GithubApp,
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
    let answer = app.rest(repo, class, Method::GET, &rest, None).await?;
    // GitHub answers 409 "Git Repository is empty" for a repository
    // without a commit: it has no branch.
    if matches!(answer.status, StatusCode::NOT_FOUND | StatusCode::CONFLICT) {
        return Ok(None);
    }

    let answer = answer.expect_status(&[StatusCode::OK])?;
    Ok(Some(answer.json::<Ref>()?.object.sha))
}

/// Moves `branch` to `sha` as `how` says, and creates the branch when it
/// does not exist.
pub(crate) async fn set_branch(
    app: &GithubApp,
    repo: &RepoRef,
    branch: &str,
    sha: &str,
    how: BranchMove,
    class: JobClass,
) -> Result<(), ScmError> {
    let rest = format!("/git/refs/heads/{}", encode(branch));
    let body = json!({ "sha": sha, "force": how == BranchMove::Force });
    let answer = app
        .rest(repo, class, Method::PATCH, &rest, Some(body))
        .await?;
    if answer.status == StatusCode::OK {
        return Ok(());
    }

    if answer.status == StatusCode::UNPROCESSABLE_ENTITY
        && answer.message().contains("not a fast forward")
    {
        return Err(ScmError::NotFastForward);
    }

    if answer.status == StatusCode::UNPROCESSABLE_ENTITY
        && answer.message().contains("Reference does not exist")
    {
        let body = json!({ "ref": format!("refs/heads/{branch}"), "sha": sha });
        let created = app
            .rest(repo, class, Method::POST, "/git/refs", Some(body))
            .await?;
        created.expect_status(&[StatusCode::CREATED])?;
        return Ok(());
    }

    Err(answer.error())
}

/// Deletes `branch`; a branch that does not exist is not an error.
pub(crate) async fn delete_branch(
    app: &GithubApp,
    repo: &RepoRef,
    branch: &str,
    class: JobClass,
) -> Result<(), ScmError> {
    let rest = format!("/git/refs/heads/{}", encode(branch));
    let answer = app.rest(repo, class, Method::DELETE, &rest, None).await?;
    match answer.status {
        StatusCode::NO_CONTENT | StatusCode::NOT_FOUND => Ok(()),
        StatusCode::UNPROCESSABLE_ENTITY if answer.message().contains("does not exist") => Ok(()),
        _ => Err(answer.error()),
    }
}

/// Merges the commit `sha` into `branch`.
pub(crate) async fn merge_into_branch(
    app: &GithubApp,
    repo: &RepoRef,
    branch: &str,
    sha: &str,
    message: &str,
    class: JobClass,
) -> Result<MergeIntoRef, ScmError> {
    #[derive(Deserialize)]
    struct Merge {
        sha: String,
    }

    let body = json!({ "base": branch, "head": sha, "commit_message": message });
    let answer = app
        .rest(repo, class, Method::POST, "/merges", Some(body))
        .await?;
    match answer.status {
        StatusCode::CREATED => Ok(MergeIntoRef::Merged {
            sha: answer.json::<Merge>()?.sha,
        }),
        StatusCode::NO_CONTENT => Ok(MergeIntoRef::AlreadyContained),
        StatusCode::CONFLICT => Ok(MergeIntoRef::Conflict),
        _ => Err(answer.error()),
    }
}

/// Whether `ancestor` is in the history of `descendant`.
pub(crate) async fn is_ancestor(
    app: &GithubApp,
    repo: &RepoRef,
    ancestor: &str,
    descendant: &str,
    class: JobClass,
) -> Result<bool, ScmError> {
    #[derive(Deserialize)]
    struct Comparison {
        status: String,
    }

    let rest = format!(
        "/compare/{}...{}?per_page=1",
        encode(ancestor),
        encode(descendant)
    );
    let answer = app.rest(repo, class, Method::GET, &rest, None).await?;
    if answer.status == StatusCode::NOT_FOUND {
        return Err(ScmError::NotFound(format!(
            "commit {ancestor} or {descendant}"
        )));
    }

    let status = answer
        .expect_status(&[StatusCode::OK])?
        .json::<Comparison>()?
        .status;
    Ok(matches!(status.as_str(), "ahead" | "identical"))
}

/// The content of the file `path` at `git_ref`, or `None` when it does not
/// exist.
pub(crate) async fn file(
    app: &GithubApp,
    repo: &RepoRef,
    git_ref: &str,
    path: &str,
    class: JobClass,
) -> Result<Option<String>, ScmError> {
    let rest = format!("/contents/{}?ref={}", encode(path), encode(git_ref));
    let auth = Auth::Installation(&repo.connection, class);
    let path = repo_path(repo, &rest);
    let answer = app
        .send_accepting(auth, Method::GET, &path, None, RAW)
        .await?;
    if answer.status == StatusCode::NOT_FOUND {
        return Ok(None);
    }

    let answer = answer.expect_status(&[StatusCode::OK])?;
    String::from_utf8(answer.body)
        .map(Some)
        .map_err(|err| ScmError::Protocol(err.to_string()))
}

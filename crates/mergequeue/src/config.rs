//! The job `load_config` (`AGENTS.md` D136): it reads the configuration file
//! on the head of the target branch, and keeps it on the queue when it is
//! valid. The file is the only source of the settings (D134).

use std::error::Error;
use std::sync::Arc;

use cihub_config::{ConfigError, FILE_NAMES, RepoConfig, find_config_file};
use cihub_domain::JobClass;
use cihub_platform::{Job, Outcome, mark_dirty};
use cihub_scm::{RepoRef, Scm, ScmError};
use cihub_store::{PgPool, begin_system};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{info, warn};

use crate::MergeQueueError;
use crate::jobs::{Failure, job, outcome, read_payload, target_of};
use crate::target::QueueTarget;

pub const LOAD_CONFIG: &str = "load_config";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct LoadConfig {
    pub sha: String,
}

/// The read of the configuration file of `target` at the head `sha`.
pub(crate) fn load_config(target: &QueueTarget, sha: &str) -> Job {
    let key = format!("{LOAD_CONFIG}:{}-{sha}:1", target.queue_id);
    let request = LoadConfig {
        sha: sha.to_owned(),
    };
    job(target, key, LOAD_CONFIG, JobClass::Repair, json!(request))
}

/// The configuration file of `repo` at `sha`, or `None` without a file.
///
/// # Errors
///
/// The SCM fails (outer), or both names of the file exist (inner).
pub(crate) async fn read_config_file(
    scm: &dyn Scm,
    repo: &RepoRef,
    sha: &str,
    class: JobClass,
) -> Result<Result<Option<String>, ConfigError>, ScmError> {
    let mut found = Vec::new();
    let mut text = None;
    for name in FILE_NAMES {
        if let Some(read) = scm.file(repo, sha, name, class).await? {
            found.push(name);
            text = Some(read);
        }
    }
    Ok(find_config_file(found).map(|_| text))
}

pub(crate) async fn execute_load_config(pool: PgPool, scm: Arc<dyn Scm>, job: Job) -> Outcome {
    outcome(load(&pool, scm.as_ref(), &job).await)
}

/// Reads the file at the SHA of the job, while it is still the head of the
/// target, and keeps it with the SHA; a file that is not valid keeps the
/// last valid one, with the error. Then marks the queue dirty, so that its
/// next reconciliation uses the settings.
async fn load(pool: &PgPool, scm: &dyn Scm, job: &Job) -> Result<(), Failure> {
    let request: LoadConfig = read_payload(job)?;
    let target = target_of(pool, job).await?;
    let head = scm
        .branch_head(&target.repo, &target.target_branch, job.class)
        .await?;
    if head.as_deref() != Some(request.sha.as_str()) {
        info!(sha = request.sha, "target moved before its file was read");
        return Ok(());
    }
    let read = read_config_file(scm, &target.repo, &request.sha, job.class).await?;
    let checked = read.and_then(|text| match text {
        Some(text) => RepoConfig::parse(&text)
            .and_then(|config| config.settings(&target.target_branch, &target.name))
            .map(|_| Some(text)),
        None => Ok(None),
    });
    let mut tx = begin_system(pool)
        .await
        .map_err(|err| Failure::from(MergeQueueError::Store(err)))?;
    let query = match &checked {
        Ok(text) => sqlx::query(
            "UPDATE queues SET config_text = $2, config_sha = $3, config_error = NULL
             WHERE id = $1",
        )
        .bind(target.queue_id.to_string())
        .bind(text),
        Err(err) => {
            warn!(
                sha = request.sha,
                error = err as &dyn Error,
                "configuration file not valid"
            );
            sqlx::query("UPDATE queues SET config_error = $2, config_sha = $3 WHERE id = $1")
                .bind(target.queue_id.to_string())
                .bind(err.to_string())
        }
    };
    query
        .bind(&request.sha)
        .execute(&mut *tx)
        .await
        .map_err(|err| Failure::from(MergeQueueError::Query(err)))?;
    mark_dirty(&mut tx, target.queue_id)
        .await
        .map_err(|err| Failure::from(MergeQueueError::Platform(err)))?;
    tx.commit()
        .await
        .map_err(|err| Failure::from(MergeQueueError::Query(err)))?;
    Ok(())
}

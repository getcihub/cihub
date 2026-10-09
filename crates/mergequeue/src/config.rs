//! The job `load_config` (`DECISIONS.md` D136): it reads the configuration file
//! on the head of the target branch, and keeps it on the queue when it is
//! valid. The file is the only source of the settings (D134).

use std::error::Error;

use cihub_config::{ConfigError, FILE_NAMES, RepoConfig, TargetsSource, find_config_file};
use cihub_domain::{JobClass, QueueId};
use cihub_mq_core::Head;
use cihub_platform::{Job, enqueue_job, mark_dirty};
use cihub_scm::{RepoRef, Scm, ScmError};
use cihub_store::{PgPool, begin_system};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgConnection;
use time::OffsetDateTime;
use tracing::{info, warn};

use crate::jobs::{Failure, job, read_payload, read_targets, target_of};
use crate::target::QueueTarget;

pub(crate) const LOAD_CONFIG: &str = "load_config";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct LoadConfig {
    pub sha: String,
}

/// The `affected_targets` block that the kept file of `target` gives.
fn affected_targets(target: &QueueTarget) -> Result<Option<TargetsSource>, Failure> {
    let config = target.config()?;
    Ok(target.settings_in(config.as_ref())?.affected_targets.value)
}

/// The head of each live item of `queue`.
async fn live_heads(conn: &mut PgConnection, queue: QueueId) -> Result<Vec<Head>, Failure> {
    let rows: Vec<(i64, String)> = sqlx::query_as(
        "SELECT pr_number, pr_head_sha FROM queue_items
         WHERE queue_id = $1 AND is_current AND state NOT IN ('merged', 'failed', 'cancelled')
         ORDER BY pr_number",
    )
    .bind(queue.to_string())
    .fetch_all(conn)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(pr_number, sha)| {
            Some(Head {
                pr_number: u64::try_from(pr_number).ok()?,
                sha,
            })
        })
        .collect())
}

/// The read of the configuration file of `target` at the head `sha`. The
/// first read holds the admissions of the queue, so it is `interactive`
/// (`DECISIONS.md` D237).
pub(crate) fn load_config(target: &QueueTarget, sha: &str) -> Job {
    let key = format!("{LOAD_CONFIG}:{}-{sha}:1", target.queue_id);
    let request = LoadConfig {
        sha: sha.to_owned(),
    };
    let class = if target.config_sha.is_some() {
        JobClass::Repair
    } else {
        JobClass::Interactive
    };
    job(target, key, LOAD_CONFIG, class, json!(request))
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

/// Reads the file at the SHA of the job, while it is still the head of the
/// target, and keeps it with the SHA; a file that is not valid keeps the
/// last valid one, with the error. A kept file whose `affected_targets`
/// block changes asks the targets of each live item again (`DECISIONS.md`
/// D230, D235). Then marks the queue dirty, so that its next reconciliation
/// uses the settings.
pub(crate) async fn load(
    pool: &PgPool,
    scm: &dyn Scm,
    job: &Job,
    now: OffsetDateTime,
) -> Result<(), Failure> {
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
    let mut tx = begin_system(pool).await?;
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

    query.bind(&request.sha).execute(&mut *tx).await?;
    if let Ok(text) = &checked {
        let mut kept = target.clone();
        kept.config_text.clone_from(text);
        kept.config_sha = Some(request.sha.clone());
        if affected_targets(&kept)? != affected_targets(&target)? {
            for head in live_heads(&mut tx, target.queue_id).await? {
                enqueue_job(&mut tx, &read_targets(&kept, &head), now, now).await?;
            }
        }
    }
    mark_dirty(&mut tx, target.queue_id).await?;
    tx.commit().await?;

    Ok(())
}

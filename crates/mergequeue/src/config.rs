//! The job `load_config`: it reads the configuration file on the head of the
//! target branch, and keeps it on the queue when it is valid. The file is the
//! only source of the settings. The job `read_setup` reads what the rules of
//! the SCM miss for the queue, its setup, and keeps it for the pages.

use std::error::Error;

use cihub_api_types::Finding;
use cihub_config::{ConfigError, FILE_NAMES, RepoConfig, find_config_file};
use cihub_domain::{JobClass, QueueId};
use cihub_platform::{Job, enqueue_job, mark_dirty, notify_queue_changed};
use cihub_scm::{RepoRef, Scm, ScmError, SetupFinding};
use cihub_store::{PgPool, begin_system};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::types::Json;
use time::{Duration, OffsetDateTime};
use tracing::{info, warn};

use crate::jobs::{
    Failure, conditions_status_context, job, queue_status_context, read_payload, target_of, window,
};
use crate::target::QueueTarget;
use crate::{MergeQueueError, parse_id};

pub(crate) const LOAD_CONFIG: &str = "load_config";
pub(crate) const READ_SETUP: &str = "read_setup";

/// How old the setup read of a queue gets before the scheduler asks for
/// another one: the SCM sends no event when a rule changes.
pub(crate) const READ_SETUP_EVERY: Duration = Duration::hours(1);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct LoadConfig {
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
/// The SCM fails (outer), or several paths of the file exist (inner).
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
/// last valid one, with the error. Then marks the queue dirty, so that its
/// next reconciliation uses the settings.
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
        info!(
            sha = request.sha,
            reason = "target_moved",
            "config.not_read"
        );
        return Ok(());
    }

    let read = read_config_file(scm, &target.repo, &request.sha, job.class).await?;
    let checked = read.and_then(|text| match text {
        Some(text) => RepoConfig::parse(&text)
            .and_then(|config| config.settings(&target.target_branch))
            .map(|_| Some(text)),
        None => Ok(None),
    });
    let mut tx = begin_system(pool).await?;
    let query = match &checked {
        Ok(text) => sqlx::query(
            "UPDATE queues SET config_text = $2, config_sha = $3, config_error = NULL,
                               updated_at = CASE
                                   WHEN (config_text, config_error) IS DISTINCT FROM ($2, NULL)
                                   THEN $4 ELSE updated_at
                               END
             WHERE id = $1",
        )
        .bind(target.queue_id.to_string())
        .bind(text),
        Err(err) => {
            warn!(
                sha = request.sha,
                error = err as &dyn Error,
                "config.invalid"
            );
            sqlx::query(
                "UPDATE queues SET config_error = $2, config_sha = $3,
                                   updated_at = CASE
                                       WHEN config_error IS DISTINCT FROM $2 THEN $4
                                       ELSE updated_at
                                   END
                 WHERE id = $1",
            )
            .bind(target.queue_id.to_string())
            .bind(err.to_string())
        }
    };

    query.bind(&request.sha).bind(now).execute(&mut *tx).await?;
    mark_dirty(&mut tx, target.queue_id).await?;
    // The page shows the settings and the error of the file.
    notify_queue_changed(&mut tx, target.org_id, target.queue_id, None).await?;
    tx.commit().await?;

    Ok(())
}

/// The setup read of `target`, once for `subject`.
pub(crate) fn read_setup_job(target: &QueueTarget, subject: &str, class: JobClass) -> Job {
    let key = format!("{READ_SETUP}:{}-{subject}:1", target.queue_id);
    job(target, key, READ_SETUP, class, json!({}))
}

/// Asks for a setup read of each queue whose last read is older than
/// [`READ_SETUP_EVERY`], or that has none, while CIHub reaches its repository
/// and no read of it waits. Each read runs at a moment of the hour that its
/// queue keeps, so that the reads of many queues do not come together.
/// Returns their number.
///
/// Like the other sweeps of the scheduler, it reads the queues of every
/// organization, then acts on each queue alone.
///
/// # Errors
///
/// Postgres fails.
pub async fn request_queue_setups(
    pool: &PgPool,
    now: OffsetDateTime,
) -> Result<usize, MergeQueueError> {
    let mut tx = begin_system(pool).await?;
    let queues: Vec<(String, i64)> = sqlx::query_as(
        "SELECT q.id, abs(hashtextextended(q.id, 0) % $3) FROM queues q
         JOIN repos r ON r.id = q.repo_id
         JOIN scm_connections c ON c.id = r.connection_id
         LEFT JOIN queue_setups k ON k.queue_id = q.id
         WHERE c.suspended_at IS NULL AND (k.checked_at IS NULL OR k.checked_at <= $1)
           AND NOT EXISTS (SELECT 1 FROM jobs j
                           WHERE j.queue_id = q.id AND j.type = $2
                             AND j.state IN ('queued', 'running'))
         ORDER BY q.id",
    )
    .bind(now - READ_SETUP_EVERY)
    .bind(READ_SETUP)
    .bind(READ_SETUP_EVERY.whole_seconds())
    .fetch_all(&mut *tx)
    .await?;
    let subject = window(now);
    let mut requested = 0;

    for (queue, delay_seconds) in &queues {
        let queue: QueueId = parse_id(queue, queue)?;
        let target = QueueTarget::load(&mut tx, queue).await?;
        let read = read_setup_job(&target, &subject, JobClass::Repair);
        let run_after = now + Duration::seconds(*delay_seconds);
        if enqueue_job(&mut tx, &read, run_after, now).await? {
            requested += 1;
        }
    }

    tx.commit().await?;

    Ok(requested)
}

/// Reads what the rules of the SCM miss for the queue, and keeps it with the
/// time of the read: a page reads it without a call to the SCM.
pub(crate) async fn read_setup(
    pool: &PgPool,
    scm: &dyn Scm,
    job: &Job,
    now: OffsetDateTime,
) -> Result<(), Failure> {
    let target = target_of(pool, job).await?;
    let branch = &target.target_branch;
    // The rules of a branch that no longer exists still answer: its head
    // says whether it does.
    let head = scm.branch_head(&target.repo, branch, job.class).await?;
    let found: Vec<CheckFinding> = if head.is_some() {
        let mut found: Vec<CheckFinding> = scm
            .check_setup(&target.repo, branch, job.class)
            .await?
            .into_iter()
            .map(CheckFinding::Setup)
            .collect();
        let required = scm.required_checks(&target.repo, branch, job.class).await?;
        if required.names.contains(&conditions_status_context(branch)) {
            found.push(CheckFinding::ConditionsStatusRequired);
        }
        found
    } else {
        info!("target_branch.gone");
        vec![CheckFinding::TargetBranchMissing]
    };

    let findings: Vec<Finding> = found
        .into_iter()
        .map(|finding| Finding {
            code: finding.code().to_owned(),
            message: finding.text(branch),
        })
        .collect();
    let mut tx = begin_system(pool).await?;

    sqlx::query(
        "INSERT INTO queue_setups (org_id, queue_id, findings, checked_at)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (queue_id) DO UPDATE SET findings = $3, checked_at = $4
         WHERE queue_setups.checked_at <= $4",
    )
    .bind(target.org_id.to_string())
    .bind(target.queue_id.to_string())
    .bind(Json(findings))
    .bind(now)
    .execute(&mut *tx)
    .await?;
    notify_queue_changed(&mut tx, target.org_id, target.queue_id, None).await?;
    tx.commit().await?;

    Ok(())
}

/// What a setup read finds: a point of the rules of the SCM, or a target
/// branch that no longer exists. Its finding comes back in an hour, as the
/// others.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CheckFinding {
    Setup(SetupFinding),
    TargetBranchMissing,
    /// A rule requires the conditions status: a PR whose conditions pass
    /// would merge outside the queue.
    ConditionsStatusRequired,
}

impl CheckFinding {
    fn code(self) -> &'static str {
        match self {
            Self::Setup(finding) => finding.code(),
            Self::TargetBranchMissing => "target_branch_missing",
            Self::ConditionsStatusRequired => "conditions_status_required",
        }
    }

    pub(crate) fn text(self, branch: &str) -> String {
        match self {
            Self::Setup(SetupFinding::UpdateRestrictionMissing) => {
                format!(
                    "no rule restricts the updates of `{branch}`: a PR can merge without the queue"
                )
            }
            Self::Setup(SetupFinding::AppNotExempt) => format!(
                "a rule restricts the updates of `{branch}` without an exemption for the App of CIHub: \
                 the queue cannot merge"
            ),
            Self::Setup(SetupFinding::TestBranchesNotExcluded) => {
                "a rule covers `cihub-temp/**` or `cihub-merge/**`: exclude the test branches"
                    .to_owned()
            }
            Self::Setup(SetupFinding::UpToDateRequired) => format!(
                "the rules of `{branch}` ask for branches up to date: turn it off, unless the queue \
                 merges directly"
            ),
            Self::TargetBranchMissing => {
                format!("the branch `{branch}` no longer exists: the queue cannot merge")
            }
            Self::ConditionsStatusRequired => format!(
                "the rules of `{branch}` require `{}`: a PR whose conditions pass can merge \
                 without the queue; require `{}` instead",
                conditions_status_context(branch),
                queue_status_context(branch)
            ),
        }
    }
}

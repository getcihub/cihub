//! The setup of a repository: the rules of its default branch and its
//! configuration file at the head of that branch, read by `read_repo_setup`
//! and kept in `repo_setups`, outside the log. The job reads at the discovery
//! of a repository, then once a day, spread over the day, and at the sync that
//! an admin asks for, at most once a minute.

use cihub_config::{FILE_NAMES, RepoConfig, find_config_file};
use cihub_domain::{JobClass, RepoConfigState, RepoId};
use cihub_platform::{Job, enqueue_job};
use cihub_scm::{RepoRef, Scm, SetupFinding};
use cihub_store::{PgPool, Postgres, Transaction, begin_org, begin_system};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::types::Json;
use time::{Duration, OffsetDateTime};
use tracing::info;

use crate::MergeQueueError;
use crate::config::CheckFinding;
use crate::jobs::{Failure, read_payload};
use crate::target::RepoTarget;

pub const READ_REPO_SETUP: &str = "read_repo_setup";

/// How often the setup of a repository is read again without a request.
pub(crate) const READ_REPO_SETUP_EVERY: Duration = Duration::days(1);

#[derive(Deserialize, Serialize)]
struct ReadRepoSetup {
    repo_id: RepoId,
}

/// The read of the setup of `repo`, once for `subject`.
pub(crate) fn read_repo_setup_job(repo: &RepoTarget, subject: &str, class: JobClass) -> Job {
    Job {
        idempotency_key: format!("{READ_REPO_SETUP}:{}-{subject}:1", repo.repo_id),
        job_type: READ_REPO_SETUP.to_owned(),
        class,
        org_id: Some(repo.org_id),
        connection: Some(repo.repo.connection.id),
        queue_id: None,
        payload: json!(ReadRepoSetup {
            repo_id: repo.repo_id
        }),
        attempts: 0,
    }
}

/// Which repositories a sweep of the setups looks at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepoSetupSweep {
    /// The repositories never read: a sweep often, and cheap, as they are few.
    New,
    /// The repositories never read, and those read more than a day ago.
    All,
}

/// Asks, in the class `repair`, for the read of the setup of each repository
/// of `sweep` that the App reaches whose connection is not suspended, without
/// a read waiting: at once for a repository never read, and at a moment of
/// the day that it keeps for one read more than a day ago. Returns the number
/// of new requests.
///
/// # Errors
///
/// Postgres fails.
pub async fn request_repo_setups(
    pool: &PgPool,
    sweep: RepoSetupSweep,
    now: OffsetDateTime,
) -> Result<usize, MergeQueueError> {
    let mut tx = begin_system(pool).await?;
    let due: Vec<(String, bool, i64)> = sqlx::query_as(
        "SELECT r.id, s.repo_id IS NULL, abs(hashtextextended(r.id, 0) % $2)
         FROM repos r
         JOIN scm_connections c ON c.id = r.connection_id
         LEFT JOIN repo_setups s ON s.repo_id = r.id
         WHERE r.removed_at IS NULL AND c.suspended_at IS NULL
           AND (s.checked_at IS NULL OR ($4 AND s.checked_at <= $1))
           AND NOT EXISTS (
               SELECT 1 FROM jobs j
               WHERE j.type = $3 AND j.state IN ('queued', 'running')
                 AND j.payload ->> 'repo_id' = r.id
           )
         ORDER BY r.id",
    )
    .bind(now - READ_REPO_SETUP_EVERY)
    .bind(READ_REPO_SETUP_EVERY.whole_seconds())
    .bind(READ_REPO_SETUP)
    .bind(sweep == RepoSetupSweep::All)
    .fetch_all(&mut *tx)
    .await?;

    let day = now.date().to_string();
    let mut requested = 0;
    for (repo, is_new, delay) in due {
        let repo: RepoId = repo
            .parse()
            .map_err(|source| MergeQueueError::ReadQueue { id: repo, source })?;
        let target = RepoTarget::load(&mut tx, repo).await?;
        let run_after = if is_new {
            now
        } else {
            now + Duration::seconds(delay)
        };
        let job = read_repo_setup_job(&target, &day, JobClass::Repair);
        if enqueue_job(&mut tx, &job, run_after, now).await? {
            requested += 1;
        }
    }

    tx.commit().await?;
    Ok(requested)
}

/// Asks, in the class `interactive`, for the read of the setup of `repo` now,
/// at most once a minute; returns whether the request is new.
///
/// # Errors
///
/// Postgres fails, or the repository does not exist.
pub(crate) async fn sync_repo_setup(
    tx: &mut Transaction<'static, Postgres>,
    repo: RepoId,
    now: OffsetDateTime,
) -> Result<bool, MergeQueueError> {
    let target = RepoTarget::load(tx, repo).await?;
    let subject = format!("sync-{}", crate::jobs::window(now));
    let job = read_repo_setup_job(&target, &subject, JobClass::Interactive);

    Ok(enqueue_job(tx, &job, now, now).await?)
}

/// What a read of the setup found: the head of the default branch, the
/// findings of its rules, and its configuration file.
struct SetupRead {
    branch: String,
    head_sha: Option<String>,
    findings: Vec<SetupFinding>,
    file: FileRead,
}

/// The configuration file at the head of the default branch.
struct FileRead {
    state: RepoConfigState,
    path: Option<&'static str>,
    text: Option<String>,
    error: Option<cihub_config::ConfigError>,
    entries: Vec<String>,
}

impl FileRead {
    const NONE: Self = Self {
        state: RepoConfigState::None,
        path: None,
        text: None,
        error: None,
        entries: Vec::new(),
    };
}

/// Reads the setup of the repository of the job and keeps it, unless a later
/// read already did. A default branch that is gone keeps no file and no
/// finding: its head is `null`.
pub(crate) async fn read_repo_setup(
    pool: &PgPool,
    scm: &dyn Scm,
    job: &Job,
    now: OffsetDateTime,
) -> Result<(), Failure> {
    let ReadRepoSetup { repo_id } = read_payload(job)?;
    let org = job.org_id.ok_or_else(|| {
        Failure::never(format!("job {} has no organization", job.idempotency_key))
    })?;
    let mut tx = begin_org(pool, org).await?;
    let target = RepoTarget::load(&mut tx, repo_id).await?;
    drop(tx);

    let read = read(scm, &target.repo, target.default_branch.clone(), job.class).await?;

    let mut tx = begin_org(pool, org).await?;
    keep(&mut tx, &target, &read, now).await?;
    tx.commit().await?;
    info!(
        findings = read.findings.len(),
        config = read.file.state.as_str(),
        "repo_setup.read"
    );
    Ok(())
}

async fn read(
    scm: &dyn Scm,
    repo: &RepoRef,
    branch: String,
    class: JobClass,
) -> Result<SetupRead, Failure> {
    let Some(head_sha) = scm.branch_head(repo, &branch, class).await? else {
        return Ok(SetupRead {
            branch,
            head_sha: None,
            findings: Vec::new(),
            file: FileRead::NONE,
        });
    };

    let findings = scm.check_setup(repo, &branch, class).await?;
    let file = read_file(scm, repo, &head_sha, &branch, class).await?;
    Ok(SetupRead {
        branch,
        head_sha: Some(head_sha),
        findings,
        file,
    })
}

/// The configuration file of `repo` at `sha`, checked as the queue of
/// `branch` would load it.
async fn read_file(
    scm: &dyn Scm,
    repo: &RepoRef,
    sha: &str,
    branch: &str,
    class: JobClass,
) -> Result<FileRead, Failure> {
    let mut found = Vec::new();
    for name in FILE_NAMES {
        if let Some(text) = scm.file(repo, sha, name, class).await? {
            found.push((name, text));
        }
    }

    let path = find_config_file(found.iter().map(|(name, _)| *name));
    let (path, text) = match (path, found.pop()) {
        (Err(error), _) => {
            return Ok(FileRead {
                state: RepoConfigState::Invalid,
                path: None,
                text: None,
                error: Some(error),
                entries: Vec::new(),
            });
        }
        (Ok(Some(path)), Some((_, text))) => (path, text),
        (Ok(_), _) => return Ok(FileRead::NONE),
    };
    let parsed = RepoConfig::parse(&text);
    let entries = parsed
        .as_ref()
        .map(|config| {
            config
                .queues
                .iter()
                .map(|queue| queue.branch.clone())
                .collect()
        })
        .unwrap_or_default();
    let error = parsed.and_then(|config| config.settings(branch)).err();

    Ok(FileRead {
        state: if error.is_some() {
            RepoConfigState::Invalid
        } else {
            RepoConfigState::Valid
        },
        path: Some(path),
        text: Some(text),
        error,
        entries,
    })
}

#[derive(Serialize)]
struct StoredFinding {
    code: &'static str,
    message: String,
}

async fn keep(
    tx: &mut Transaction<'static, Postgres>,
    target: &RepoTarget,
    read: &SetupRead,
    now: OffsetDateTime,
) -> Result<(), MergeQueueError> {
    let findings: Vec<StoredFinding> = read
        .findings
        .iter()
        .map(|finding| StoredFinding {
            code: finding.code(),
            message: CheckFinding::Setup(*finding).text(&read.branch),
        })
        .collect();
    let error = read.file.error.as_ref();
    let position = error.and_then(|error| error.position);
    let line = position.map(|position| i32::try_from(position.line).unwrap_or(i32::MAX));
    let column = position.map(|position| i32::try_from(position.column).unwrap_or(i32::MAX));

    sqlx::query(
        "INSERT INTO repo_setups (org_id, repo_id, branch, head_sha, findings, config_state,
                                  config_path, config_error, config_line, config_column,
                                  config_text, entries, checked_at)
         VALUES ($1, $2, $3, $4, $5, $6::repo_config_state, $7, $8, $9, $10, $11, $12, $13)
         ON CONFLICT (repo_id) DO UPDATE
         SET branch = $3, head_sha = $4, findings = $5, config_state = $6::repo_config_state,
             config_path = $7, config_error = $8, config_line = $9, config_column = $10,
             config_text = $11, entries = $12, checked_at = $13
         WHERE repo_setups.checked_at <= $13",
    )
    .bind(target.org_id.to_string())
    .bind(target.repo_id.to_string())
    .bind(&read.branch)
    .bind(&read.head_sha)
    .bind(Json(findings))
    .bind(read.file.state.as_str())
    .bind(read.file.path)
    .bind(error.map(|error| error.message.clone()))
    .bind(line)
    .bind(column)
    .bind(&read.file.text)
    .bind(&read.file.entries)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

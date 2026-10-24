//! The setup of a repository: the rules of its default branch and its
//! configuration file at the head of that branch, read by `read_repo_setup`
//! and kept in `repo_setups` and `config_files`, outside the log. The job
//! reads at the discovery of a repository, then once a day, spread over the
//! day, and at the sync that an admin asks for, at most once a minute.

use cihub_domain::{JobClass, RepoId};
use cihub_platform::{Job, enqueue_job};
use cihub_scm::{RepoRef, Scm, SetupFinding};
use cihub_store::{PgPool, Postgres, Transaction, begin_org, begin_system};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::types::Json;
use time::{Duration, OffsetDateTime};
use tracing::info;

use crate::MergeQueueError;
use crate::config::{CheckFinding, ConfigBranch, ConfigRead, keep_config_file};
use crate::jobs::{Failure, read_payload};
use crate::proposal::{end_proposal, open_proposals, read_proposal_prs};
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
/// findings of its rules, the checks that they require, and its
/// configuration file.
struct SetupRead {
    branch: String,
    head_sha: Option<String>,
    findings: Vec<SetupFinding>,
    required_checks: Option<Vec<String>>,
    file: ConfigRead,
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
        Failure::Never(format!("job {} has no organization", job.idempotency_key))
    })?;
    let mut tx = begin_org(pool, org).await?;
    let target = RepoTarget::load(&mut tx, repo_id).await?;
    let proposals = open_proposals(&mut tx, repo_id).await?;
    drop(tx);

    let read = read(scm, &target.repo, target.default_branch.clone(), job.class).await?;
    // A delivery of the PR of a proposal can be lost: the read catches it up.
    let proposal_prs = read_proposal_prs(scm, &target.repo, &proposals, job.class).await?;

    let mut tx = begin_org(pool, org).await?;
    keep(&mut tx, &target, &read, now).await?;
    for (number, state) in proposal_prs {
        end_proposal(&mut tx, repo_id, number, state, now).await?;
    }
    tx.commit().await?;
    info!(
        repo_id = %repo_id,
        findings = read.findings.len(),
        config = read.file.state().as_str(),
        "repository setup read"
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
            required_checks: None,
            file: ConfigRead::NONE,
        });
    };

    let findings = scm.check_setup(repo, &branch, class).await?;
    let required = scm.required_checks(repo, &branch, class).await?;
    let file = ConfigRead::read(scm, repo, &head_sha, &branch, class).await?;
    Ok(SetupRead {
        branch,
        head_sha: Some(head_sha),
        findings,
        required_checks: Some(required.names.into_iter().collect()),
        file,
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

    sqlx::query(
        "INSERT INTO repo_setups (org_id, repo_id, branch, head_sha, findings, required_checks,
                                  checked_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (repo_id) DO UPDATE
         SET branch = $3, head_sha = $4, findings = $5, required_checks = $6, checked_at = $7
         WHERE repo_setups.checked_at <= $7",
    )
    .bind(target.org_id.to_string())
    .bind(target.repo_id.to_string())
    .bind(&read.branch)
    .bind(&read.head_sha)
    .bind(Json(findings))
    .bind(&read.required_checks)
    .bind(now)
    .execute(&mut **tx)
    .await?;

    let branch = ConfigBranch {
        org_id: target.org_id,
        repo_id: target.repo_id,
        branch: &read.branch,
        head_sha: read.head_sha.as_deref(),
    };
    keep_config_file(tx, &branch, &read.file, now).await
}

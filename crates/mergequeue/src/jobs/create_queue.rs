//! Creates the queue of a branch for a submission that finds none.

use cihub_config::RepoConfig;
use cihub_domain::{JobClass, QueueId};
use cihub_platform::{mark_dirty, notify_queue_changed};
use cihub_scm::Scm;
use cihub_store::{PgPool, begin_system};
use tracing::info;

use super::Failure;
use crate::config::read_config_file;
use crate::target::RepoTarget;

/// Creates the queue of `branch` when the configuration file on the head of
/// the branch names it, or when it is the default branch. The queue keeps
/// the file that it comes from, so that the submission reads its conditions.
pub(super) async fn create_queue(
    pool: &PgPool,
    scm: &dyn Scm,
    repo: &RepoTarget,
    branch: &str,
    class: JobClass,
) -> Result<(), Failure> {
    let Some(head) = scm.branch_head(&repo.repo, branch, class).await? else {
        return Ok(());
    };

    let read = read_config_file(scm, &repo.repo, &head, class).await?;
    let checked = read.and_then(|text| {
        text.map(|text| RepoConfig::parse(&text).map(|config| (text, config)))
            .transpose()
    });
    let is_named = matches!(&checked, Ok(Some((_, config))) if config.queue(branch).is_some());
    if !is_named && branch != repo.default_branch {
        return Ok(());
    }

    let (text, error) = match checked {
        Ok(found) => (found.map(|(text, _)| text), None),
        Err(err) => (None, Some(err.to_string())),
    };
    let mut tx = begin_system(pool).await?;
    #[expect(
        clippy::disallowed_methods,
        reason = "a new queue gets a new identifier"
    )]
    let id = QueueId::new();
    let inserted = sqlx::query(
        "INSERT INTO queues (id, org_id, repo_id, target_branch, dirty,
                             config_text, config_sha, config_error)
         VALUES ($1, $2, $3, $4, true, $5, $6, $7)
         ON CONFLICT (repo_id, target_branch) DO NOTHING",
    )
    .bind(id.to_string())
    .bind(repo.org_id.to_string())
    .bind(repo.repo_id.to_string())
    .bind(branch)
    .bind(&text)
    .bind(&head)
    .bind(&error)
    .execute(&mut *tx)
    .await?;

    if inserted.rows_affected() == 1 {
        info!(target_branch = branch, "queue created");
        mark_dirty(&mut tx, id).await?;
        notify_queue_changed(&mut tx, repo.org_id, id, Some(0)).await?;
    }

    tx.commit().await?;

    Ok(())
}

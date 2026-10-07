//! The affected targets that the CI of a PR uploads (`DECISIONS.md` D234):
//! kept by head in `affected_targets`, given at once to the queues in
//! `source: upload` whose live item has that head, and read by the
//! `read_targets` job of a head submitted after its upload.

use cihub_config::TargetsSource;
use cihub_domain::{Actor, AffectedTargets, EventKind, OrgId, QueueId, RepoId};
use sqlx::PgConnection;
use time::OffsetDateTime;

use crate::MergeQueueError;
use crate::jobs::give_back;
use crate::target::QueueTarget;

/// Stores the upload of `head_sha`; a new upload of the same head replaces
/// it.
pub(crate) async fn store(
    conn: &mut PgConnection,
    org_id: OrgId,
    repo_id: RepoId,
    pr_number: u64,
    head_sha: &str,
    targets: &AffectedTargets,
    now: OffsetDateTime,
) -> Result<(), MergeQueueError> {
    let (list, is_all) = match targets {
        AffectedTargets::All => (Vec::new(), true),
        AffectedTargets::Targets(set) => (set.iter().cloned().collect(), false),
    };
    sqlx::query(
        "INSERT INTO affected_targets (org_id, repo_id, pr_number, head_sha, targets, is_all,
                                       uploaded_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (repo_id, pr_number, head_sha)
         DO UPDATE SET targets = $5, is_all = $6, uploaded_at = $7",
    )
    .bind(org_id.to_string())
    .bind(repo_id.to_string())
    .bind(i64::try_from(pr_number).unwrap_or(i64::MAX))
    .bind(head_sha)
    .bind(list)
    .bind(is_all)
    .bind(now)
    .execute(conn)
    .await
    .map_err(MergeQueueError::Query)?;
    Ok(())
}

/// The upload of `head_sha`, if any.
pub(crate) async fn stored(
    conn: &mut PgConnection,
    org_id: OrgId,
    repo_id: RepoId,
    pr_number: u64,
    head_sha: &str,
) -> Result<Option<AffectedTargets>, MergeQueueError> {
    let row: Option<(Vec<String>, bool)> = sqlx::query_as(
        "SELECT targets, is_all FROM affected_targets
         WHERE org_id = $1 AND repo_id = $2 AND pr_number = $3 AND head_sha = $4",
    )
    .bind(org_id.to_string())
    .bind(repo_id.to_string())
    .bind(i64::try_from(pr_number).unwrap_or(i64::MAX))
    .bind(head_sha)
    .fetch_optional(conn)
    .await
    .map_err(MergeQueueError::Query)?;
    Ok(row.map(|(targets, is_all)| {
        if is_all {
            AffectedTargets::All
        } else {
            AffectedTargets::Targets(targets.into_iter().collect())
        }
    }))
}

/// Gives the upload to each queue in `source: upload` whose live item of
/// the PR has `head_sha`; a queue in `source: files` ignores it. Returns the
/// number of queues.
pub(crate) async fn deliver(
    conn: &mut PgConnection,
    org_id: OrgId,
    repo_id: RepoId,
    pr_number: u64,
    head_sha: &str,
    targets: &AffectedTargets,
    now: OffsetDateTime,
) -> Result<usize, MergeQueueError> {
    let queues: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT i.queue_id FROM queue_items i JOIN queues q ON q.id = i.queue_id
         WHERE q.org_id = $1 AND q.repo_id = $2 AND i.pr_number = $3 AND i.pr_head_sha = $4
           AND i.is_current AND i.concluded_at IS NULL
         ORDER BY i.queue_id",
    )
    .bind(org_id.to_string())
    .bind(repo_id.to_string())
    .bind(i64::try_from(pr_number).unwrap_or(i64::MAX))
    .bind(head_sha)
    .fetch_all(&mut *conn)
    .await
    .map_err(MergeQueueError::Query)?;
    let mut delivered = 0;
    for queue in queues {
        let queue: QueueId = queue
            .parse()
            .map_err(|source| MergeQueueError::ReadQueue { id: queue, source })?;
        let target = QueueTarget::load(conn, queue).await?;
        let config = target.config()?;
        let settings = target.settings_in(config.as_ref())?;
        if settings.affected_targets.value != Some(TargetsSource::Upload) {
            continue;
        }
        let kind = EventKind::TargetsReceived {
            pr_number,
            head_sha: head_sha.to_owned(),
            targets: targets.clone(),
        };
        give_back(conn, org_id, queue, vec![(Actor::System, kind)], now).await?;
        delivered += 1;
    }
    Ok(delivered)
}

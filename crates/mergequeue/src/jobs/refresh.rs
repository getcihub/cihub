//! The `refresh_prs` job: reads PRs again and gives back one `pr.observed`
//! per PR.

use std::collections::HashMap;

use cihub_config::QueueSettings;
use cihub_domain::{
    Actor, EventKind, ItemState, JobClass, PrState, Priority, QueueId, SubmittedVia, UserId,
};
use cihub_platform::Job;
use cihub_scm::{MAX_PRS_PER_READ, PrSnapshot, Scm};
use cihub_store::{PgPool, begin_system};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgConnection;
use time::OffsetDateTime;

use super::{
    Failure, give_back_with_titles, job, observed, read_payload, subject, target_of, window,
};
use crate::MergeQueueError;
use crate::admission::{Reading, Sender, unmet_conditions};
use crate::target::QueueTarget;

pub const REFRESH_PRS: &str = "refresh_prs";

/// The PRs that `refresh_prs` reads, at most
/// [`MAX_PRS_PER_READ`](cihub_scm::MAX_PRS_PER_READ).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct RefreshPrs {
    pub prs: Vec<u64>,
}

/// A read of `prs` after the delivery `delivery_id`.
pub(crate) fn refresh_after(target: &QueueTarget, delivery_id: &str, prs: Vec<u64>) -> Job {
    let key = format!(
        "{REFRESH_PRS}:{}-{}:1",
        target.queue_id,
        subject(delivery_id)
    );
    let payload = json!(RefreshPrs { prs });
    job(target, key, REFRESH_PRS, JobClass::Interactive, payload)
}

/// The reads of `prs`, due at the read point `point`.
pub(crate) fn refresh_at(target: &QueueTarget, point: OffsetDateTime, prs: &[u64]) -> Vec<Job> {
    refresh_reads(target, &window(point), JobClass::Repair, prs)
}

/// The reads of `prs` under the subject `subject`, by chunks of
/// [`MAX_PRS_PER_READ`]; the chunks after the first one add their rank to
/// the key.
pub(crate) fn refresh_reads(
    target: &QueueTarget,
    subject: &str,
    class: JobClass,
    prs: &[u64],
) -> Vec<Job> {
    prs.chunks(MAX_PRS_PER_READ)
        .enumerate()
        .map(|(chunk, prs)| {
            let rank = if chunk == 0 {
                String::new()
            } else {
                format!("-{}", chunk + 1)
            };
            let key = format!("{REFRESH_PRS}:{}-{subject}{rank}:1", target.queue_id);
            let payload = json!(RefreshPrs { prs: prs.to_vec() });
            job(target, key, REFRESH_PRS, class, payload)
        })
        .collect()
}

/// Reads the PRs of the job and gives back one `pr.observed` per PR. A PR
/// that no longer exists gives nothing.
pub(crate) async fn refresh(
    pool: &PgPool,
    scm: &dyn Scm,
    job: &Job,
    now: OffsetDateTime,
) -> Result<(), Failure> {
    let request: RefreshPrs = read_payload(job)?;
    let target = target_of(pool, job).await?;
    let required = scm
        .required_checks(&target.repo, &target.target_branch, job.class)
        .await?;
    let prs = scm
        .pull_requests(&target.repo, &request.prs, job.class)
        .await?;
    let mut tx = begin_system(pool).await?;
    let submissions = submissions(&mut tx, target.queue_id, &request.prs).await?;

    tx.commit().await?;

    let config = target.config()?;
    let settings = target.settings_in(config.as_ref())?;
    let mut facts = Vec::with_capacity(prs.len());

    for pr in &prs {
        let submission = submissions.get(&pr.number);
        // The new submission comes before the observation, which cancels the
        // old item: the queue keeps the observation for the new one.
        let resubmitted =
            submission.and_then(|submission| resubmission(&settings, &target, pr, submission));
        // An admitted item no longer depends on its conditions; a new
        // submission meets them on its new head, or waits.
        let admitted = resubmitted.is_none() && submission.is_some_and(|s| s.admitted);
        facts.extend(resubmitted);
        let unmet = if admitted {
            Vec::new()
        } else {
            let sender = submission.map(|s| s.sender.clone()).unwrap_or_default();
            let reading = Reading {
                sender: &sender,
                rules: &required,
                injection: settings.branch_protection_injection.value,
                class: job.class,
                now,
            };
            unmet_conditions(scm, &target, config.as_ref(), pr, &reading).await?
        };
        let kind = observed(pr, &required, unmet);
        facts.push((Actor::System, kind));
    }

    give_back_with_titles(pool, &target, &prs, facts, now).await
}

/// The live item of a PR, as `refresh_prs` needs it.
struct Submission {
    /// The item left `not_ready`.
    admitted: bool,
    sender: Sender,
    /// What a new submission of the PR keeps.
    head_sha: String,
    priority: Priority,
    no_batch: bool,
    profile: Option<String>,
    via: SubmittedVia,
    submitted_by: Option<UserId>,
    author_id: UserId,
}

#[derive(sqlx::FromRow)]
struct SubmissionRow {
    pr_number: i64,
    state: String,
    login: Option<String>,
    permission: Option<String>,
    head_sha: String,
    priority: i16,
    no_batch: bool,
    profile: Option<String>,
    via: String,
    submitted_by: Option<String>,
    author_id: String,
}

impl SubmissionRow {
    fn decode(self) -> Option<(u64, Submission)> {
        let submission = Submission {
            admitted: self.state != ItemState::NotReady.as_str(),
            sender: Sender {
                login: self.login,
                permission: self.permission.and_then(|p| p.parse().ok()),
            },
            head_sha: self.head_sha,
            priority: Priority(u8::try_from(self.priority).ok()?),
            no_batch: self.no_batch,
            profile: self.profile,
            via: self.via.parse().ok()?,
            submitted_by: self.submitted_by.and_then(|id| id.parse().ok()),
            author_id: self.author_id.parse().ok()?,
        };
        Some((u64::try_from(self.pr_number).ok()?, submission))
    }
}

/// The live items of `prs` in `queue`, with who submitted them.
async fn submissions(
    conn: &mut PgConnection,
    queue: QueueId,
    prs: &[u64],
) -> Result<HashMap<u64, Submission>, MergeQueueError> {
    let numbers: Vec<i64> = prs.iter().filter_map(|n| i64::try_from(*n).ok()).collect();
    let rows: Vec<SubmissionRow> = sqlx::query_as(
        "SELECT i.pr_number, i.state::text AS state, u.login,
                i.sender_permission::text AS permission, i.pr_head_sha AS head_sha,
                i.priority_value AS priority, i.no_batch, i.profile,
                i.submitted_via::text AS via, i.submitted_by, i.pr_author_id AS author_id
         FROM queue_items i LEFT JOIN users u ON u.id = i.submitted_by
         WHERE i.queue_id = $1 AND i.is_current
           AND i.state NOT IN ('merged', 'failed', 'cancelled')
           AND i.pr_number = ANY($2)",
    )
    .bind(queue.to_string())
    .bind(numbers)
    .fetch_all(conn)
    .await?;
    Ok(rows.into_iter().filter_map(SubmissionRow::decode).collect())
}

/// The new submission of a PR in the queue that has a new head, when the
/// queue resubmits on a push: same priority, source and options. A PR that
/// is closed, a draft or on another base leaves the queue instead, and a
/// submission by label needs the label still.
fn resubmission(
    settings: &QueueSettings,
    target: &QueueTarget,
    pr: &PrSnapshot,
    submission: &Submission,
) -> Option<(Actor, EventKind)> {
    let pushed = pr.state == PrState::Open
        && !pr.draft
        && pr.base_branch == target.target_branch
        && pr.head_sha != submission.head_sha;
    let labelled =
        submission.via != SubmittedVia::Label || pr.labels.contains(&settings.enqueue_label.value);

    if !(settings.resubmit_on_push.value && pushed && labelled) {
        return None;
    }

    let actor = submission
        .submitted_by
        .map_or(Actor::System, |user_id| Actor::User { user_id });
    let kind = EventKind::ItemSubmitted {
        pr_number: pr.number,
        head_sha: pr.head_sha.clone(),
        priority: submission.priority,
        no_batch: submission.no_batch,
        submitted_via: submission.via,
        author_id: submission.author_id,
        additions: pr.additions,
        deletions: pr.deletions,
        changed_files: pr.changed_files,
        sender_permission: submission.sender.permission,
        profile: submission.profile.clone(),
    };

    Some((actor, kind))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::target::test_target;

    #[test]
    fn keys_name_their_cause() {
        let target = test_target();
        let queue = target.queue_id;
        let after = refresh_after(&target, "a:b", vec![1]);

        assert_eq!(after.idempotency_key, format!("refresh_prs:{queue}-a-b:1"));
        assert_eq!(after.class, JobClass::Interactive);

        let point = OffsetDateTime::from_unix_timestamp(1_790_000_000).unwrap();
        let prs: Vec<u64> = (1..=u64::try_from(MAX_PRS_PER_READ + 1).unwrap()).collect();
        let reads = refresh_at(&target, point, &prs);

        assert_eq!(reads.len(), 2);
        assert_eq!(
            reads[0].idempotency_key,
            format!("refresh_prs:{queue}-20260921T1413:1")
        );
        assert_eq!(reads[0].class, JobClass::Repair);
        assert_eq!(
            reads[1].idempotency_key,
            format!("refresh_prs:{queue}-20260921T1413-2:1")
        );

        let second: RefreshPrs = serde_json::from_value(reads[1].payload.clone()).unwrap();
        assert_eq!(second.prs, vec![prs[MAX_PRS_PER_READ]]);
    }
}

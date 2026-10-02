//! The `queue_conditions` of a queue (`AGENTS.md` D142): the jobs that read
//! a PR evaluate them, and give the conditions that the PR does not meet in
//! its `pr.observed`. The core sees the result, not the conditions: they
//! need the logins, which the log does not keep (D42).

use std::time::Duration;

use cihub_conditions::{Attribute, Check, Conditions, Context};
use cihub_config::RepoConfig;
use cihub_domain::{JobClass, Mergeability, Permission};
use cihub_scm::{PrSnapshot, Scm};
use time::OffsetDateTime;

use crate::MergeQueueError;
use crate::jobs::Failure;
use crate::target::QueueTarget;

/// Who submitted the PR, for `sender` and `sender-permission`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Sender {
    pub login: Option<String>,
    pub permission: Option<Permission>,
}

/// The `queue_conditions` of the entry of the queue in its last valid file;
/// none without a file or an entry (D138).
fn queue_conditions(target: &QueueTarget) -> Result<Conditions, MergeQueueError> {
    let Some(text) = &target.config_text else {
        return Ok(Conditions::default());
    };
    let config = RepoConfig::parse(text).map_err(MergeQueueError::Settings)?;
    Ok(config
        .queue(&target.target_branch, &target.name)
        .map(|queue| queue.queue_conditions.clone())
        .unwrap_or_default())
}

/// The context of the conditions for `pr`, before its admission: the PR is
/// not in the queue yet.
fn context(target: &QueueTarget, pr: &PrSnapshot, sender: &Sender, now: OffsetDateTime) -> Context {
    let list = |set: &std::collections::BTreeSet<String>| set.iter().cloned().collect();
    Context {
        draft: pr.draft,
        conflict: pr.mergeability == Mergeability::Conflict,
        from_fork: pr.from_fork,
        behind_base: pr.mergeability == Mergeability::Behind,
        number: pr.number,
        title: pr.title.clone(),
        body: pr.body.clone(),
        review_decision: pr.review_decision,
        approved_reviews_by: list(&pr.approved_by),
        changes_requested_reviews_by: list(&pr.changes_requested_by),
        review_requested: list(&pr.review_requested),
        review_threads_unresolved: None,
        checks: pr
            .checks
            .iter()
            .map(|check| Check {
                name: check.name.clone(),
                conclusion: check.conclusion,
            })
            .collect(),
        labels: pr.labels.iter().cloned().collect(),
        author: pr.author.login.clone(),
        sender: sender.login.clone(),
        sender_permission: sender.permission,
        files: None,
        additions: pr.additions,
        deletions: pr.deletions,
        base: pr.base_branch.clone(),
        head: pr.head_branch.clone(),
        queue_name: Some(target.name.clone()),
        queue_position: None,
        queue_age: None,
        pr_age: Duration::try_from(now - pr.created_at).unwrap_or_default(),
    }
}

/// The `queue_conditions` of the queue of `target` that `pr` does not meet,
/// as the file writes them. The files and the review threads are read only
/// when a condition uses them (D143).
pub(crate) async fn unmet_conditions(
    scm: &dyn Scm,
    target: &QueueTarget,
    pr: &PrSnapshot,
    sender: &Sender,
    class: JobClass,
    now: OffsetDateTime,
) -> Result<Vec<String>, Failure> {
    let conditions = queue_conditions(target)?;
    if conditions.0.is_empty() {
        return Ok(Vec::new());
    }
    let mut ctx = context(target, pr, sender, now);
    if conditions.uses(Attribute::Files) {
        ctx.files = Some(scm.pr_files(&target.repo, pr.number, class).await?);
    }
    if conditions.uses(Attribute::ReviewThreadsUnresolved) {
        ctx.review_threads_unresolved = Some(
            scm.unresolved_threads(&target.repo, pr.number, class)
                .await?,
        );
    }
    conditions
        .unmet(&ctx)
        .map_err(|err| Failure::Never(err.to_string()))
}

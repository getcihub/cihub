//! The `submit_pr` job: checks a submission after a command or a label, reads
//! the PR, and gives back its `item.submitted` and its `pr.observed`.

use cihub_config::{QueueSettings, RepoConfig};
use cihub_domain::{
    Actor, ConnectionId, EventKind, JobClass, OrgId, Permission, PrState, Priority, RepoId,
    SubmittedVia, UserId,
};
use cihub_platform::Job;
use cihub_scm::{PrSnapshot, RepoRef, Scm, ScmError, ScmUser};
use cihub_store::{PgPool, begin_system};
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::OffsetDateTime;
use tracing::info;

use super::create_queue::create_queue;
use super::{
    Account, Failure, acknowledge, connection_job, give_back_with_titles, load_repo, observed,
    read_payload, reply, user_of,
};
use crate::admission::{Reading, Sender, submission_choices};
use crate::target::{QueueTarget, RepoTarget};

pub(crate) const SUBMIT_PR: &str = "submit_pr";

/// A submission that waits for its checks: the permission of the author of
/// a command, the label of a label submission.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SubmitPr {
    /// The repository of the PR: the job finds the queue by the base branch
    /// that it reads, since a comment does not give it.
    pub repo_id: RepoId,
    pub pr_number: u64,
    pub via: SubmittedVia,
    /// The author of the command; `None` for a label, whose sender the
    /// delivery does not give.
    pub requested_by: Option<Account>,
    /// `None` leaves the priority to the `priority_rules`.
    pub priority: Option<Priority>,
    pub no_batch: bool,
    pub direct: bool,
    /// The profile that the command names; `None` leaves it to the
    /// `conditions` of the profiles.
    pub profile: Option<String>,
    /// The label that the action put, for a submission by label.
    pub label: Option<String>,
    /// The comment of the command, which the job acknowledges once the
    /// queue has the submission.
    pub comment_id: Option<String>,
}

impl SubmitPr {
    /// The submission of the PR `pr_number` of `repo_id` by the label
    /// `label`: no author, and the priority of the rules.
    pub(crate) fn by_label(repo_id: RepoId, pr_number: u64, label: String) -> Self {
        Self {
            repo_id,
            pr_number,
            via: SubmittedVia::Label,
            requested_by: None,
            priority: None,
            no_batch: false,
            direct: false,
            profile: None,
            label: Some(label),
            comment_id: None,
        }
    }
}

/// The submission of `request` after the delivery `delivery_id`, on the
/// connection `connection`. It has no queue yet.
pub(crate) fn submit(
    org_id: OrgId,
    connection: ConnectionId,
    delivery_id: &str,
    request: &SubmitPr,
) -> Job {
    connection_job(org_id, connection, delivery_id, SUBMIT_PR, json!(request))
}

/// The author of the command of `request` with their permission, or `None`
/// when the permission is too low (`write`, `admin` for `--direct`). A label
/// gives no sender.
async fn sender_of(
    scm: &dyn Scm,
    repo: &RepoTarget,
    request: &SubmitPr,
    class: JobClass,
) -> Result<Option<Sender>, Failure> {
    let Some(author) = &request.requested_by else {
        return Ok(Some(Sender::default()));
    };

    let need = if request.direct {
        Permission::Admin
    } else {
        Permission::Write
    };
    let has = scm
        .permission(&repo.repo, &ScmUser::from(author), class)
        .await?;

    Ok((has >= need).then(|| Sender {
        login: Some(author.login.clone()),
        permission: Some(has),
    }))
}

/// The queue of `branch`, if it takes submissions.
async fn open_queue(
    pool: &PgPool,
    repo: &RepoTarget,
    branch: &str,
) -> Result<Option<QueueTarget>, Failure> {
    let mut tx = begin_system(pool).await?;
    let queue = QueueTarget::of_branch(&mut tx, repo.repo_id, branch).await?;

    tx.commit().await?;

    match queue {
        Some(queue) if queue.takes_submissions(&repo.default_branch)? => Ok(Some(queue)),
        _ => Ok(None),
    }
}

/// Why the queue refuses the submission, if it does, as the reply says it: a
/// command while the commands are off, a label that is not the label of the
/// queue, or that the PR no longer has.
fn refusal(
    settings: &QueueSettings,
    target: &QueueTarget,
    request: &SubmitPr,
    pr: &PrSnapshot,
) -> Option<String> {
    let label = &settings.enqueue_label.value;
    let branch = &target.target_branch;
    match request.via {
        SubmittedVia::Comment if !settings.commands_enabled.value => {
            Some(format!("the commands are off for the queue of `{branch}`."))
        }
        SubmittedVia::Label
            if !settings.label_commands_enabled.value || request.label.as_ref() != Some(label) =>
        {
            Some(format!(
                "the label is not the label of the queue of `{branch}`."
            ))
        }
        SubmittedVia::Label if !pr.labels.contains(label) => {
            Some(format!("the PR no longer has the label `{label}`."))
        }
        _ => None,
    }
}

/// What the PR is, when it cannot enter a queue: only an open PR that is not a
/// draft can.
fn not_open(pr: &PrSnapshot) -> Option<&'static str> {
    match pr.state {
        PrState::Merged => Some("merged"),
        PrState::Closed => Some("closed"),
        PrState::Open if pr.draft => Some("a draft"),
        PrState::Open => None,
    }
}

async fn find_pull_request(
    scm: &dyn Scm,
    repo: &RepoTarget,
    pr_number: u64,
    class: JobClass,
) -> Result<Option<PrSnapshot>, Failure> {
    match scm.pull_request(&repo.repo, pr_number, class).await {
        Ok(pr) => Ok(Some(pr)),
        Err(ScmError::NotFound(_)) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

async fn queue_of_pr(
    pool: &PgPool,
    scm: &dyn Scm,
    repo: &RepoTarget,
    pr: &PrSnapshot,
    request: &SubmitPr,
    class: JobClass,
) -> Result<Option<QueueTarget>, Failure> {
    let queue = open_queue(pool, repo, &pr.base_branch).await?;

    // A label is a setting of a queue: only a command creates one.
    if queue.is_some() || request.via != SubmittedVia::Comment {
        return Ok(queue);
    }

    create_queue(pool, scm, repo, &pr.base_branch, class).await?;

    open_queue(pool, repo, &pr.base_branch).await
}

async fn submission_accounts(
    pool: &PgPool,
    pr: &PrSnapshot,
    request: &SubmitPr,
) -> Result<(UserId, Actor), Failure> {
    // The accounts come in their own transaction: the one that gives the
    // facts back waits for the reconciliation of the queue, which may need
    // the same rows of `users`.
    let mut tx = begin_system(pool).await?;
    let author_id = user_of(&mut tx, &pr.author).await?;
    let actor = match &request.requested_by {
        Some(account) => Actor::User {
            user_id: user_of(&mut tx, &account.into()).await?,
        },
        None => Actor::System,
    };

    tx.commit().await?;

    Ok((author_id, actor))
}

/// The profile that `request` names, `None` when it names none. The error is
/// the refusal of a profile that the queue of `branch` in `config` does not
/// have.
fn named_profile(
    request: &SubmitPr,
    config: Option<&RepoConfig>,
    branch: &str,
) -> Result<Option<String>, String> {
    let Some(name) = &request.profile else {
        return Ok(None);
    };

    config
        .and_then(|config| config.profile(branch, name))
        .map(|profile| Some(profile.name.clone()))
        .ok_or_else(|| format!("the queue of `{branch}` has no profile `{name}`."))
}

/// Answers the refused submission `request`: a command or a checkbox gets the
/// cause, a label nothing, since its delivery does not give its author.
async fn refuse_submission(
    scm: &dyn Scm,
    repo: &RepoRef,
    request: &SubmitPr,
    text: String,
    class: JobClass,
) -> Result<(), Failure> {
    let Some(author) = &request.requested_by else {
        return Ok(());
    };

    reply(
        scm,
        repo,
        request.pr_number,
        author,
        "/cihub merge",
        text,
        class,
    )
    .await
}

/// Checks the submission of the job, reads the PR, and gives back its
/// `item.submitted` and its `pr.observed`. A refused command gets a comment
/// that says why; a refused label gets none.
pub(crate) async fn submit_pr(
    pool: &PgPool,
    scm: &dyn Scm,
    job: &Job,
    now: OffsetDateTime,
) -> Result<(), Failure> {
    let request: SubmitPr = read_payload(job)?;
    let repo = load_repo(pool, request.repo_id).await?;
    let pr_number = request.pr_number;
    let repo_ref = &repo.repo;
    let refuse = |text: String| refuse_submission(scm, repo_ref, &request, text, job.class);

    let Some(sender) = sender_of(scm, &repo, &request, job.class).await? else {
        info!(pr_number, "submission refused: permission");
        return refuse(
            "the command needs the write permission on the repository (admin with `--direct`)."
                .to_owned(),
        )
        .await;
    };

    let Some(pr) = find_pull_request(scm, &repo, pr_number, job.class).await? else {
        return Ok(());
    };

    if let Some(state) = not_open(&pr) {
        info!(pr_number, state, "submission refused: PR not open");
        return refuse(format!("this PR is {state}.")).await;
    }

    let Some(target) = queue_of_pr(pool, scm, &repo, &pr, &request, job.class).await? else {
        info!(
            pr_number,
            base_branch = pr.base_branch,
            "submission dropped: no queue"
        );
        return refuse(format!(
            "no queue serves the branch `{}`; a queue of `.cihub.yaml` on this branch would.",
            pr.base_branch
        ))
        .await;
    };

    let required = scm
        .required_checks(&target.repo, &target.target_branch, job.class)
        .await?;
    let config = target.config()?;
    let settings = target.settings_in(config.as_ref())?;
    if let Some(text) = refusal(&settings, &target, &request, &pr) {
        info!(pr_number, reason = text, "submission dropped");
        return refuse(text).await;
    }

    let reading = Reading {
        sender: &sender,
        rules: &required,
        injection: settings.branch_protection_injection.value,
        class: job.class,
        now,
    };
    let choices = submission_choices(scm, &target, config.as_ref(), &pr, &reading).await?;
    let profile = match named_profile(&request, config.as_ref(), &target.target_branch) {
        Ok(named) => named.or(choices.profile),
        Err(text) => {
            info!(pr_number, "submission refused: unknown profile");
            return refuse(text).await;
        }
    };
    let priority = request
        .priority
        .or(choices.priority)
        .unwrap_or(Priority::MEDIUM);
    let (author_id, actor) = submission_accounts(pool, &pr, &request).await?;

    let submitted = EventKind::ItemSubmitted {
        pr_number,
        head_sha: pr.head_sha.clone(),
        priority,
        no_batch: request.no_batch,
        direct: request.direct,
        submitted_via: request.via,
        author_id,
        additions: pr.additions,
        deletions: pr.deletions,
        changed_files: pr.changed_files,
        sender_permission: sender.permission,
        profile,
    };
    let facts = vec![
        (actor, submitted),
        (Actor::System, observed(&pr, &required, choices.unmet)),
    ];

    give_back_with_titles(pool, &target, std::slice::from_ref(&pr), facts, now).await?;
    acknowledge(scm, repo_ref, request.comment_id.as_deref(), job.class).await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::target::test_target;

    #[test]
    fn the_key_of_a_submission_names_its_delivery() {
        let target = test_target();
        let request = SubmitPr {
            repo_id: target.repo_id,
            pr_number: 1,
            via: SubmittedVia::Comment,
            requested_by: None,
            priority: None,
            no_batch: false,
            direct: false,
            profile: None,
            label: None,
            comment_id: None,
        };

        assert_eq!(
            submit(target.org_id, target.repo.connection.id, "d", &request).idempotency_key,
            "submit_pr:d:1"
        );
    }
}

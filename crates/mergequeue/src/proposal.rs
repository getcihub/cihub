//! A proposal of a change of the configuration file of a branch: the job
//! `propose_config` writes the file on the branch `cihub-config/<branch>` and
//! opens a PR that a person reviews and merges. The deliveries of the PR end
//! the proposal; `read_repo_setup` reads again the PRs of the open ones, in
//! case a delivery is lost. Kept in `config_proposals`, outside the log.

use cihub_domain::{
    ConfigProposalId, ConfigProposalReason, ConfigProposalState, JobClass, PrState, RepoId,
};
use cihub_platform::Job;
use cihub_scm::{RepoRef, Scm, ScmError};
use cihub_store::{PgPool, begin_org};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgConnection;
use time::OffsetDateTime;
use tracing::info;

use crate::config::read_config_path;
use crate::jobs::{Failure, read_payload};
use crate::target::RepoTarget;
use crate::{MergeQueueError, parse_id};

pub(crate) const PROPOSE_CONFIG: &str = "propose_config";

#[derive(Deserialize, Serialize)]
struct ProposeConfig {
    proposal_id: ConfigProposalId,
}

/// The job that opens the PR of `proposal`, once.
pub(crate) fn propose_config_job(repo: &RepoTarget, proposal: ConfigProposalId) -> Job {
    Job {
        idempotency_key: format!("{PROPOSE_CONFIG}:{proposal}:1"),
        job_type: PROPOSE_CONFIG.to_owned(),
        class: JobClass::Interactive,
        org_id: Some(repo.org_id),
        connection: Some(repo.repo.connection.id),
        queue_id: None,
        payload: json!(ProposeConfig {
            proposal_id: proposal
        }),
        attempts: 0,
    }
}

/// The branch where CIHub writes the proposals of `branch`.
pub(crate) fn proposal_branch(branch: &str) -> String {
    format!("cihub-config/{branch}")
}

/// What the job needs of a proposal.
#[derive(sqlx::FromRow)]
struct Proposal {
    repo_id: String,
    branch: String,
    base_sha: String,
    path: String,
    text: String,
    message: String,
    state: String,
    login: Option<String>,
}

/// Opens the PR of the proposal of the job: the file at the head of its
/// branch must still be the one at `base_sha`. Each step converges, so the job
/// runs again after a failure: the branch of the proposal moves to the head,
/// the commit holds the text, and an open PR keeps its number.
pub(crate) async fn propose(
    pool: &PgPool,
    scm: &dyn Scm,
    job: &Job,
    now: OffsetDateTime,
) -> Result<(), Failure> {
    let ProposeConfig { proposal_id } = read_payload(job)?;
    let org = job.org_id.ok_or_else(|| {
        Failure::Never(format!("job {} has no organization", job.idempotency_key))
    })?;
    let mut tx = begin_org(pool, org).await?;
    let proposal: Proposal = sqlx::query_as(
        "SELECT p.repo_id, p.branch, p.base_sha, p.path, p.text, p.message, p.state::text,
                u.login
         FROM config_proposals p LEFT JOIN users u ON u.id = p.requested_by
         WHERE p.id = $1",
    )
    .bind(proposal_id.to_string())
    .fetch_one(&mut *tx)
    .await?;
    let repo_id: RepoId = parse_id(&proposal.repo_id, proposal_id)?;
    let target = RepoTarget::load(&mut tx, repo_id).await?;
    drop(tx);

    if proposal.state != ConfigProposalState::Requested.as_str() {
        return Ok(());
    }

    // A refusal of the SCM ends the proposal: retrying would not change it.
    let opened = match open_pr(scm, &target.repo, &proposal, job.class).await {
        Ok(opened) => opened,
        Err(Failure::Never(text)) => {
            info!(proposal_id = %proposal_id, error = text, "the SCM refused the proposal");
            Err(ConfigProposalReason::PrNotOpened)
        }
        Err(retry) => return Err(retry),
    };

    let (state, reason, number) = match opened {
        Ok(number) => (ConfigProposalState::Opened, None, Some(number)),
        Err(reason) => (ConfigProposalState::Failed, Some(reason), None),
    };
    let mut tx = begin_org(pool, org).await?;
    sqlx::query(
        "UPDATE config_proposals
         SET state = $2::config_proposal_state, state_reason = $3::config_proposal_reason,
             pr_number = $4, updated_at = $5
         WHERE id = $1 AND state = 'requested'",
    )
    .bind(proposal_id.to_string())
    .bind(state.as_str())
    .bind(reason.map(ConfigProposalReason::as_str))
    .bind(
        number
            .map(i64::try_from)
            .transpose()
            .map_err(MergeQueueError::from)?,
    )
    .bind(now)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    info!(
        proposal_id = %proposal_id,
        state = state.as_str(),
        reason = reason.map(ConfigProposalReason::as_str),
        "configuration proposal ended its job"
    );
    Ok(())
}

/// The number of the PR of `proposal`, or why CIHub cannot open it.
async fn open_pr(
    scm: &dyn Scm,
    repo: &RepoRef,
    proposal: &Proposal,
    class: JobClass,
) -> Result<Result<u64, ConfigProposalReason>, Failure> {
    let Some(head) = scm.branch_head(repo, &proposal.branch, class).await? else {
        return Ok(Err(ConfigProposalReason::ConfigChanged));
    };

    let at_base = read_config_path(scm, repo, &proposal.base_sha, class).await?;
    let at_head = read_config_path(scm, repo, &head, class).await?;
    if at_base != at_head {
        return Ok(Err(ConfigProposalReason::ConfigChanged));
    }

    // A run that stopped after its commit finds it: the PR keeps it, and its CI does not run again.
    let branch = proposal_branch(&proposal.branch);
    let is_written = match scm.branch_head(repo, &branch, class).await? {
        Some(written) => {
            let file = scm.file(repo, &written, &proposal.path, class).await?;
            file.as_deref() == Some(proposal.text.as_str())
        }
        None => false,
    };
    if !is_written {
        match scm.set_branch(repo, &branch, &head, true, class).await {
            Err(ScmError::Forbidden(_) | ScmError::Validation(_)) => {
                return Ok(Err(ConfigProposalReason::ConfigBranchRefused));
            }
            moved => moved?,
        }
        let commit = scm
            .commit_file(
                repo,
                &branch,
                &proposal.path,
                &proposal.text,
                &proposal.message,
                class,
            )
            .await;
        match commit {
            Err(ScmError::Forbidden(_)) => {
                return Ok(Err(ConfigProposalReason::ConfigBranchRefused));
            }
            committed => committed?,
        }
    }

    let (title, body) = pr_text(&proposal.message, proposal.login.as_deref());
    let number = scm
        .open_pr(repo, &branch, &proposal.branch, &title, &body, false, class)
        .await?;
    Ok(Ok(number))
}

/// The title of the PR, the first line of the message, and its body: the rest
/// of the message and who proposed the change.
fn pr_text(message: &str, login: Option<&str>) -> (String, String) {
    let (title, rest) = message.split_once('\n').unwrap_or((message, ""));
    let by = match login {
        Some(login) => format!("Proposed in CIHub by {login}."),
        None => "Proposed in CIHub with an API token.".to_owned(),
    };
    let body = match rest.trim() {
        "" => by,
        rest => format!("{rest}\n\n{by}"),
    };
    (title.trim().to_owned(), body)
}

/// Ends the open proposal of `repo` whose PR is `number`, once the PR merged
/// or closed; a PR that is still open, or that is no proposal, changes
/// nothing.
///
/// # Errors
///
/// Postgres fails.
pub(crate) async fn end_proposal(
    conn: &mut PgConnection,
    repo: RepoId,
    number: u64,
    state: PrState,
    now: OffsetDateTime,
) -> Result<(), MergeQueueError> {
    let ended = match state {
        PrState::Open => return Ok(()),
        PrState::Merged => ConfigProposalState::Merged,
        PrState::Closed => ConfigProposalState::Closed,
    };

    sqlx::query(
        "UPDATE config_proposals SET state = $3::config_proposal_state, updated_at = $4
         WHERE repo_id = $1 AND pr_number = $2 AND state = 'opened'",
    )
    .bind(repo.to_string())
    .bind(i64::try_from(number)?)
    .bind(ended.as_str())
    .bind(now)
    .execute(conn)
    .await?;
    Ok(())
}

/// The PR numbers of the open proposals of `repo`.
///
/// # Errors
///
/// Postgres fails.
pub(crate) async fn open_proposals(
    conn: &mut PgConnection,
    repo: RepoId,
) -> Result<Vec<u64>, MergeQueueError> {
    let numbers: Vec<i64> = sqlx::query_scalar(
        "SELECT pr_number FROM config_proposals
         WHERE repo_id = $1 AND state = 'opened' AND pr_number IS NOT NULL",
    )
    .bind(repo.to_string())
    .fetch_all(conn)
    .await?;

    Ok(numbers
        .into_iter()
        .map(u64::try_from)
        .collect::<Result<_, _>>()?)
}

/// The state of the PR of each open proposal: a PR that the SCM no longer
/// finds is closed.
///
/// # Errors
///
/// The SCM fails.
pub(crate) async fn read_proposal_prs(
    scm: &dyn Scm,
    repo: &RepoRef,
    numbers: &[u64],
    class: JobClass,
) -> Result<Vec<(u64, PrState)>, ScmError> {
    let mut states = Vec::with_capacity(numbers.len());
    for &number in numbers {
        let state = match scm.pull_request(repo, number, class).await {
            Ok(pr) => pr.state,
            Err(ScmError::NotFound(_)) => PrState::Closed,
            Err(err) => return Err(err),
        };
        states.push((number, state));
    }
    Ok(states)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_line_of_the_message_is_the_title_of_the_pr() {
        assert_eq!(
            pr_text("Update the queue\n\nBatches of 6.", Some("ada")),
            (
                "Update the queue".to_owned(),
                "Batches of 6.\n\nProposed in CIHub by ada.".to_owned()
            )
        );
        assert_eq!(
            pr_text("Update the queue", None),
            (
                "Update the queue".to_owned(),
                "Proposed in CIHub with an API token.".to_owned()
            )
        );
    }
}

//! The jobs of the test cycle (lot 6, `AGENTS.md` D117). `assemble_test_branch`
//! resolves the required statuses of a run, builds its test branch and gives
//! back `test_branch.assembled` or `test_branch.assembly_failed`;
//! `cleanup_test_branch` closes the draft PR and deletes the branches of a run
//! that ended. Each one reads the real state first and changes nothing that
//! is already done (principle III).

use std::fmt::Write;
use std::sync::Arc;

use cihub_config::{QueueSettings, RepoConfig};
use cihub_domain::{
    Actor, BranchProtectionInjection, ConflictCulprit, EventKind, JobClass, JobError,
    RequiredStatusesSource, RunId, RunStatus, Trigger, temp_branch,
};
use cihub_mq_core::{Head, TestBranch};
use cihub_platform::{Job, MAX_ATTEMPTS, Outcome, error_chain};
use cihub_scm::{MergeIntoRef, Scm, ScmError};
use cihub_store::{PgPool, begin_system};
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::{Duration, OffsetDateTime};
use tracing::{info, warn};

use crate::MergeQueueError;
use crate::config::read_config_file;
use crate::jobs::{
    Failure, give_back, job, outcome, queue_status_context, read_payload, target_of,
};
use crate::target::QueueTarget;

pub const ASSEMBLE_TEST_BRANCH: &str = "assemble_test_branch";
pub const CLEANUP_TEST_BRANCH: &str = "cleanup_test_branch";

/// After this many attempts, an assembly that the SCM still refuses as not
/// valid (422) gives up with `start_test_run_retry_exhausted` (`AGENTS.md`
/// D133).
pub const ASSEMBLY_ATTEMPTS: i32 = 5;

/// An assembly that meets another passing error (5xx, 429, a stop) tries
/// again until this long after the start of its run (`AGENTS.md` D133).
pub const ASSEMBLY_WAIT: Duration = Duration::minutes(15);

/// The test branch that a run needs, as the plan describes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AssembleTestBranch {
    pub run_id: RunId,
    pub test_branch: String,
    pub base_sha: String,
    pub predecessors: Vec<Head>,
    pub members: Vec<Head>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CleanupTestBranch {
    pub run_id: RunId,
}

/// The assembly of the test branch of `branch`, one job per run.
pub(crate) fn assemble(target: &QueueTarget, branch: &TestBranch) -> Job {
    let request = AssembleTestBranch {
        run_id: branch.run_id,
        test_branch: branch.name.clone(),
        base_sha: branch.base_sha.clone(),
        predecessors: branch.predecessors.clone(),
        members: branch.members.clone(),
    };
    let key = format!("{ASSEMBLE_TEST_BRANCH}:{}:1", branch.run_id);
    job(
        target,
        key,
        ASSEMBLE_TEST_BRANCH,
        JobClass::Speculate,
        json!(request),
    )
}

/// The cleanup of the run `run_id` once it ended.
pub(crate) fn cleanup(target: &QueueTarget, run_id: RunId) -> Job {
    let key = format!("{CLEANUP_TEST_BRANCH}:{run_id}:1");
    job(
        target,
        key,
        CLEANUP_TEST_BRANCH,
        JobClass::Repair,
        json!(CleanupTestBranch { run_id }),
    )
}

/// The title of the draft PR of a run (`docs/CI.md`).
pub fn draft_pr_title(members: &[u64], target_branch: &str) -> String {
    format!("CIHub: test {} on {target_branch}", numbers(members))
}

/// The body of the draft PR of a run (`docs/CI.md`): the run, its base and
/// the PRs it tests, each on its own line.
pub fn draft_pr_body(
    run_id: RunId,
    target_branch: &str,
    base_sha: &str,
    predecessors: &[u64],
    members: &[u64],
) -> String {
    let mut body = format!("**CIHub merge queue** test run `{run_id}`.\n\n");
    let _ = writeln!(body, "- Target: `{target_branch}` at `{base_sha}`");
    let _ = writeln!(body, "- Tests: {}", numbers(members));
    let on_top = if predecessors.is_empty() {
        "nothing".to_owned()
    } else {
        numbers(predecessors)
    };
    let _ = writeln!(body, "- On top of: {on_top}");
    body.push_str(
        "\nCIHub opened this pull request for the CI and closes it when the run ends. \
         Do not merge, review or edit it.\n",
    );
    body
}

fn numbers(prs: &[u64]) -> String {
    prs.iter()
        .map(|n| format!("#{n}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The required statuses that `.cihub.yaml` gives for the queue `name` on
/// `branch`, or `None` when the file leaves them to the branch protection
/// (`docs/CONFIGURATION.md`: `null` or no key inherits). A queue without an
/// entry in the file inherits them (`AGENTS.md` D138).
///
/// # Errors
///
/// The file is not valid, or it gives an empty list.
pub fn required_statuses_of_file(
    text: &str,
    branch: &str,
    name: &str,
) -> Result<Option<Vec<String>>, JobError> {
    let config = RepoConfig::parse(text).map_err(|err| err.job_error())?;
    let setting = config.queue(branch, name).and_then(|queue| {
        queue
            .settings
            .required_statuses
            .as_ref()
            .or(config.defaults.required_statuses.as_ref())
    });
    match setting.map(|setting| &setting.value) {
        Some(Some(names)) if names.is_empty() => Err(JobError::ConfigMissingRequiredStatuses),
        Some(Some(names)) => Ok(Some(names.clone())),
        Some(None) | None => Ok(None),
    }
}

/// The names of the required statuses at `base_sha` (`docs/ALGORITHMS.md`):
/// the list of the file of the repository on the target branch at
/// `base_sha`, or else the rules of the target branch when
/// `branch_protection_injection` lets them in (`AGENTS.md` D147), without the
/// status of the queue itself (D98); then the checks of the
/// `merge_conditions` (D146). `settings` gives the injection without a file.
pub(crate) async fn required_statuses(
    scm: &dyn Scm,
    target: &QueueTarget,
    settings: &QueueSettings,
    base_sha: &str,
    class: JobClass,
) -> Result<(Vec<String>, RequiredStatusesSource), Step> {
    let branch = target.target_branch.as_str();
    let name = target.name.as_str();
    let file = read_config_file(scm, &target.repo, base_sha, class)
        .await?
        .map_err(|err| err.job_error())?;
    let (listed, merge_checks, injection) = match file {
        Some(text) => {
            let listed = required_statuses_of_file(&text, branch, name)?;
            let config = RepoConfig::parse(&text).map_err(|err| err.job_error())?;
            let settings = config
                .settings(branch, name)
                .map_err(|err| err.job_error())?;
            let injection = settings.branch_protection_injection.value;
            (listed, config.merge_checks(branch, name), injection)
        }
        None => (None, Vec::new(), settings.branch_protection_injection.value),
    };
    let (mut names, source) = match listed {
        Some(names) => (names, RequiredStatusesSource::RepoConfig),
        None if injection == BranchProtectionInjection::None => {
            (Vec::new(), RequiredStatusesSource::RepoConfig)
        }
        None => {
            let rules = scm.required_checks(&target.repo, branch, class).await?;
            let own = queue_status_context(branch);
            let names: Vec<String> = rules.names.into_iter().filter(|n| *n != own).collect();
            let source = if names.is_empty() {
                RequiredStatusesSource::RepoConfig
            } else {
                RequiredStatusesSource::BranchProtection
            };
            (names, source)
        }
    };
    for check in merge_checks {
        if !names.contains(&check) {
            names.push(check);
        }
    }
    if names.is_empty() {
        return Err(JobError::NoRequiredStatuses.into());
    }
    Ok((names, source))
}

/// Why a step of the assembly did not give a test branch.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Step {
    /// The run fails with this error: a retry would not change it.
    GiveUp(JobError, Option<ConflictCulprit>),
    /// The SCM refused a call as not valid: the job tries again, at most
    /// [`ASSEMBLY_ATTEMPTS`] times.
    Transient(String),
    /// A call failed for now (server error, rate limit): the job tries again
    /// until [`ASSEMBLY_WAIT`] after the start of the run.
    Passing(String),
    /// A predecessor conflicts: its own run decides, this one waits for the
    /// queue to restart it without that predecessor.
    Wait(String),
}

impl From<ScmError> for Step {
    fn from(err: ScmError) -> Self {
        match err {
            ScmError::Forbidden(_) => Self::GiveUp(JobError::PermissionsBlockTestBranch, None),
            ScmError::Validation(_) => Self::Transient(error_chain(&err)),
            other => Self::Passing(error_chain(&other)),
        }
    }
}

impl From<JobError> for Step {
    fn from(error: JobError) -> Self {
        Self::GiveUp(error, None)
    }
}

/// What the assembly gives back when it succeeds.
#[derive(Debug, PartialEq, Eq)]
struct Assembled {
    test_sha: String,
    draft_pr_number: Option<u64>,
    required_statuses: Vec<String>,
    required_statuses_source: RequiredStatusesSource,
}

/// One assembly, on the SCM of the queue.
struct Assembly<'a> {
    scm: &'a dyn Scm,
    settings: &'a QueueSettings,
    target: &'a QueueTarget,
    request: &'a AssembleTestBranch,
    class: JobClass,
}

impl Assembly<'_> {
    async fn build(&self) -> Result<Assembled, Step> {
        let (required_statuses, required_statuses_source) = self.required_statuses().await?;
        let repo = &self.target.repo;
        let test_sha = match self
            .scm
            .branch_head(repo, &self.request.test_branch, self.class)
            .await?
        {
            Some(sha) => sha,
            None => self.merge_heads().await?,
        };
        let draft_pr_number = match self.settings.trigger.value {
            Trigger::DraftPr => Some(self.open_draft_pr().await?),
            Trigger::Push => None,
            Trigger::Inplace => {
                warn!("trigger inplace is not available before its lot");
                return Err(Step::GiveUp(JobError::InvalidEvent, None));
            }
        };
        Ok(Assembled {
            test_sha,
            draft_pr_number,
            required_statuses,
            required_statuses_source,
        })
    }

    async fn required_statuses(&self) -> Result<(Vec<String>, RequiredStatusesSource), Step> {
        required_statuses(
            self.scm,
            self.target,
            self.settings,
            &self.request.base_sha,
            self.class,
        )
        .await
    }

    /// Creates the test branch: the temporary branch at `base_sha`, the heads
    /// merged in order, the test branch at the result, and the temporary
    /// branch deleted. Returns the test SHA.
    async fn merge_heads(&self) -> Result<String, Step> {
        let repo = &self.target.repo;
        let request = self.request;
        let temp = temp_branch(&request.test_branch).ok_or(JobError::InvalidEvent)?;
        self.scm
            .set_branch(repo, &temp, &request.base_sha, true, self.class)
            .await?;
        let mut current = request.base_sha.clone();
        for head in &request.predecessors {
            match self.merge(&temp, head).await? {
                MergeIntoRef::Merged { sha } => current = sha,
                MergeIntoRef::AlreadyContained => {}
                MergeIntoRef::Conflict => {
                    return Err(Step::Wait(format!(
                        "predecessor #{} conflicts with the test branch: its own run decides",
                        head.pr_number
                    )));
                }
            }
        }
        for head in &request.members {
            match self.merge(&temp, head).await? {
                MergeIntoRef::Merged { sha } => current = sha,
                // The head is already in the target: nothing to test
                // (`AGENTS.md` D130).
                MergeIntoRef::AlreadyContained => {
                    return Err(Step::GiveUp(
                        JobError::MergeConflict,
                        Some(ConflictCulprit::BaseBranch),
                    ));
                }
                MergeIntoRef::Conflict => {
                    let culprit = self.culprit(&temp, head).await?;
                    return Err(Step::GiveUp(JobError::MergeConflict, Some(culprit)));
                }
            }
        }
        self.scm
            .set_branch(repo, &request.test_branch, &current, false, self.class)
            .await?;
        self.scm.delete_branch(repo, &temp, self.class).await?;
        Ok(current)
    }

    async fn merge(&self, branch: &str, head: &Head) -> Result<MergeIntoRef, ScmError> {
        let message = format!(
            "CIHub merge queue: merge #{} into {}",
            head.pr_number, self.request.test_branch
        );
        self.scm
            .merge_into_branch(&self.target.repo, branch, &head.sha, &message, self.class)
            .await
    }

    /// What `member` conflicts with: the target branch when it does not merge
    /// on `base_sha` alone, otherwise the first predecessor that does not
    /// merge on `base_sha` plus the member.
    async fn culprit(&self, temp: &str, member: &Head) -> Result<ConflictCulprit, ScmError> {
        let request = self.request;
        if request.predecessors.is_empty() {
            return Ok(ConflictCulprit::BaseBranch);
        }
        self.scm
            .set_branch(&self.target.repo, temp, &request.base_sha, true, self.class)
            .await?;
        if self.merge(temp, member).await? == MergeIntoRef::Conflict {
            return Ok(ConflictCulprit::BaseBranch);
        }
        for head in &request.predecessors {
            if self.merge(temp, head).await? == MergeIntoRef::Conflict {
                return Ok(ConflictCulprit::QueuePr {
                    pr_number: head.pr_number,
                });
            }
        }
        Ok(ConflictCulprit::BaseBranch)
    }

    /// Opens the draft PR of the test branch, or finds the open one.
    async fn open_draft_pr(&self) -> Result<u64, Step> {
        let request = self.request;
        let target = self.target.target_branch.as_str();
        let numbers = |heads: &[Head]| heads.iter().map(|h| h.pr_number).collect::<Vec<_>>();
        let (predecessors, members) = (numbers(&request.predecessors), numbers(&request.members));
        let title = draft_pr_title(&members, target);
        let body = draft_pr_body(
            request.run_id,
            target,
            &request.base_sha,
            &predecessors,
            &members,
        );
        let opened = self
            .scm
            .open_draft_pr(
                &self.target.repo,
                &request.test_branch,
                target,
                &title,
                &body,
                self.class,
            )
            .await;
        match opened {
            Ok(number) => Ok(number),
            Err(ScmError::Validation(message)) if message.to_lowercase().contains("draft") => {
                Err(JobError::DraftPrsNotAllowed.into())
            }
            // The test branch holds nothing more than the target: its heads
            // are already in it (`AGENTS.md` D130).
            Err(ScmError::Validation(message))
                if message.to_lowercase().contains("no commits between") =>
            {
                Err(Step::GiveUp(
                    JobError::MergeConflict,
                    Some(ConflictCulprit::BaseBranch),
                ))
            }
            Err(err) => Err(err.into()),
        }
    }
}

/// What a step that failed after `attempts` attempts gives, `elapsed` after
/// the start of the run: a new attempt later (`Ok`, with the error for the
/// operator), or the end of the run. A new attempt must end in a fact
/// before the job goes `dead`.
fn give_up(
    step: Step,
    attempts: i32,
    elapsed: Duration,
) -> Result<String, (JobError, Option<ConflictCulprit>)> {
    // A retry of the attempt `MAX_ATTEMPTS` would make the job `dead`.
    let last = attempts >= MAX_ATTEMPTS;
    match step {
        Step::GiveUp(error, culprit) => Err((error, culprit)),
        Step::Wait(text) => Ok(text),
        Step::Transient(text) if attempts < ASSEMBLY_ATTEMPTS && !last => Ok(text),
        Step::Passing(text) if elapsed < ASSEMBLY_WAIT && !last => Ok(text),
        Step::Transient(_) | Step::Passing(_) => Err((JobError::RetryExhausted, None)),
    }
}

/// The status and the start of the run `run_id` in `test_runs`, or `None`
/// for an unknown run.
pub(crate) async fn run_state(
    pool: &PgPool,
    run_id: RunId,
) -> Result<Option<(RunStatus, OffsetDateTime)>, Failure> {
    let mut tx = begin_system(pool)
        .await
        .map_err(|err| Failure::from(MergeQueueError::Store(err)))?;
    let row: Option<(String, OffsetDateTime)> =
        sqlx::query_as("SELECT status::text, started_at FROM test_runs WHERE id = $1")
            .bind(run_id.to_string())
            .fetch_optional(&mut *tx)
            .await
            .map_err(|err| Failure::from(MergeQueueError::Query(err)))?;
    row.map(|(status, started_at)| {
        status
            .parse::<RunStatus>()
            .map(|status| (status, started_at))
            .map_err(|err| Failure::Never(error_chain(&err)))
    })
    .transpose()
}

/// Builds the test branch of the run of the job and gives back the result.
/// A run that no longer waits for its assembly leaves the SCM as it is.
pub(crate) async fn execute_assemble(
    pool: PgPool,
    scm: Arc<dyn Scm>,
    job: Job,
    now: OffsetDateTime,
) -> Outcome {
    outcome(assemble_run(&pool, scm.as_ref(), &job, now).await)
}

async fn assemble_run(
    pool: &PgPool,
    scm: &dyn Scm,
    job: &Job,
    now: OffsetDateTime,
) -> Result<(), Failure> {
    let request: AssembleTestBranch = read_payload(job)?;
    let run_id = request.run_id;
    let target = target_of(pool, job).await?;
    let settings = &target.settings()?;
    let Some((RunStatus::Assembling, started_at)) = run_state(pool, run_id).await? else {
        info!(%run_id, "run no longer waits for its test branch");
        return Ok(());
    };
    let assembly = Assembly {
        scm,
        settings,
        target: &target,
        request: &request,
        class: job.class,
    };
    let result = match assembly.build().await {
        Ok(assembled) => Ok(assembled),
        Err(step) => match give_up(step, job.attempts, now - started_at) {
            Ok(text) => return Err(Failure::Retry(text)),
            Err((error, culprit)) => {
                if error == JobError::RetryExhausted {
                    warn!(%run_id, "assembly gave up after its attempts");
                }
                Err((error, culprit))
            }
        },
    };
    let fact = match result {
        Ok(assembled) => {
            info!(%run_id, test_sha = assembled.test_sha, "test branch assembled");
            EventKind::TestBranchAssembled {
                run_id,
                test_sha: assembled.test_sha,
                draft_pr_number: assembled.draft_pr_number,
                required_statuses: assembled.required_statuses,
                required_statuses_source: assembled.required_statuses_source,
            }
        }
        Err((error, conflict_culprit)) => {
            info!(%run_id, error = error.as_str(), "test branch not assembled");
            EventKind::TestBranchAssemblyFailed {
                run_id,
                error,
                conflict_culprit,
            }
        }
    };
    let mut tx = begin_system(pool)
        .await
        .map_err(|err| Failure::from(MergeQueueError::Store(err)))?;
    give_back(
        &mut tx,
        target.org_id,
        target.queue_id,
        vec![(Actor::System, fact)],
        now,
    )
    .await?;
    tx.commit()
        .await
        .map_err(|err| Failure::from(MergeQueueError::Query(err)))?;
    Ok(())
}

/// Closes the draft PR and deletes the branches of the run of the job, once
/// the run ended. Each step is a success when its state is already reached.
pub(crate) async fn execute_cleanup(pool: PgPool, scm: Arc<dyn Scm>, job: Job) -> Outcome {
    outcome(cleanup_run(&pool, scm.as_ref(), &job).await)
}

async fn cleanup_run(pool: &PgPool, scm: &dyn Scm, job: &Job) -> Result<(), Failure> {
    let request: CleanupTestBranch = read_payload(job)?;
    let target = target_of(pool, job).await?;
    let mut tx = begin_system(pool)
        .await
        .map_err(|err| Failure::from(MergeQueueError::Store(err)))?;
    let run: Option<(String, Option<i64>, String)> = sqlx::query_as(
        "SELECT test_branch, draft_pr_number, status::text FROM test_runs WHERE id = $1",
    )
    .bind(request.run_id.to_string())
    .fetch_optional(&mut *tx)
    .await
    .map_err(|err| Failure::from(MergeQueueError::Query(err)))?;
    tx.commit()
        .await
        .map_err(|err| Failure::from(MergeQueueError::Query(err)))?;
    let Some((test_branch, draft_pr_number, status)) = run else {
        return Ok(());
    };
    let status = status
        .parse::<RunStatus>()
        .map_err(|err| Failure::Never(error_chain(&err)))?;
    if !status.is_terminal() {
        info!(run_id = %request.run_id, "run still live: nothing to clean");
        return Ok(());
    }
    let repo = &target.repo;
    if let Some(number) = draft_pr_number.and_then(|n| u64::try_from(n).ok()) {
        scm.close_pr(repo, number, job.class).await?;
    }
    scm.delete_branch(repo, &test_branch, job.class).await?;
    if let Some(temp) = temp_branch(&test_branch) {
        scm.delete_branch(repo, &temp, job.class).await?;
    }
    info!(run_id = %request.run_id, "test branch cleaned");
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use cihub_config::{Layer, Setting};
    use cihub_domain::{ConnectionId, OrgId, QueueId, RepoId, ScmInstanceId};
    use cihub_scm::RequiredChecks;
    use cihub_scm::sim::{Fault, Sim, bot};
    use ulid::Ulid;

    use super::*;

    fn target() -> QueueTarget {
        QueueTarget {
            queue_id: QueueId::from_ulid(Ulid(2)),
            org_id: OrgId::from_ulid(Ulid(1)),
            scm_instance_id: ScmInstanceId::from_ulid(Ulid(3)),
            repo: cihub_scm::RepoRef {
                connection: cihub_scm::ConnectionRef {
                    id: ConnectionId::from_ulid(Ulid(4)),
                    external_id: "4".into(),
                },
                external_id: "5".into(),
                namespace: "acme".into(),
                name: "app".into(),
            },
            target_branch: "main".into(),
            name: "main".into(),
            config_text: None,
            config_sha: None,
            enabled: true,
            repo_id: RepoId::from_ulid(Ulid(6)),
        }
    }

    #[test]
    fn only_refusals_count_and_passing_errors_wait_15_minutes() {
        let minutes = Duration::minutes;
        let retry = |step, attempts, elapsed| give_up(step, attempts, elapsed).is_ok();
        let transient = || Step::Transient("422".into());
        let passing = || Step::Passing("502".into());
        assert!(retry(transient(), ASSEMBLY_ATTEMPTS - 1, minutes(1)));
        assert!(!retry(transient(), ASSEMBLY_ATTEMPTS, minutes(1)));
        // Server errors and stops do not count, until the wait ends.
        assert!(retry(passing(), ASSEMBLY_ATTEMPTS + 3, minutes(14)));
        assert_eq!(
            give_up(passing(), 2, ASSEMBLY_WAIT),
            Err((JobError::RetryExhausted, None))
        );
        // The last attempt before `dead` gives up in any case.
        assert!(retry(passing(), MAX_ATTEMPTS - 1, minutes(1)));
        assert!(!retry(passing(), MAX_ATTEMPTS, minutes(1)));
        assert!(retry(Step::Wait("predecessor".into()), 1, minutes(1)));
        assert_eq!(
            give_up(Step::GiveUp(JobError::MergeConflict, None), 1, minutes(0)),
            Err((JobError::MergeConflict, None))
        );
        // The backoff of the jobs covers the wait before `dead`.
        let covered: i64 = (1..MAX_ATTEMPTS)
            .map(|n| cihub_platform::backoff(n).whole_seconds())
            .sum();
        assert!(covered >= ASSEMBLY_WAIT.whole_seconds(), "{covered} s");
    }

    #[test]
    fn keys_name_the_run() {
        let run_id = RunId::from_ulid(Ulid(7));
        let branch = TestBranch {
            run_id,
            name: cihub_domain::test_branch("cihub", 12, run_id),
            base_sha: "base".into(),
            predecessors: vec![Head {
                pr_number: 10,
                sha: "a".into(),
            }],
            members: vec![Head {
                pr_number: 12,
                sha: "b".into(),
            }],
        };
        let assemble = assemble(&target(), &branch);
        assert_eq!(
            assemble.idempotency_key,
            format!("assemble_test_branch:{run_id}:1")
        );
        assert_eq!(assemble.class, JobClass::Speculate);
        let request: AssembleTestBranch = serde_json::from_value(assemble.payload).unwrap();
        assert_eq!(request.members[0].pr_number, 12);
        let cleanup = cleanup(&target(), run_id);
        assert_eq!(
            cleanup.idempotency_key,
            format!("cleanup_test_branch:{run_id}:1")
        );
        assert_eq!(cleanup.class, JobClass::Repair);
    }

    #[test]
    fn the_draft_pr_names_the_run_and_the_prs() {
        let run_id = RunId::from_ulid(Ulid(7));
        assert_eq!(draft_pr_title(&[12], "main"), "CIHub: test #12 on main");
        let body = draft_pr_body(run_id, "main", "abc", &[10, 11], &[12]);
        assert_eq!(
            body,
            format!(
                "**CIHub merge queue** test run `{run_id}`.\n\n\
                 - Target: `main` at `abc`\n\
                 - Tests: #12\n\
                 - On top of: #10, #11\n\n\
                 CIHub opened this pull request for the CI and closes it when the run ends. \
                 Do not merge, review or edit it.\n"
            )
        );
        let alone = draft_pr_body(run_id, "main", "abc", &[], &[12]);
        assert!(alone.contains("- On top of: nothing\n"));
    }

    /// A configuration file, a target branch, and the required statuses
    /// that the file gives for it.
    type FileCase = (String, &'static str, Result<Option<Vec<String>>, JobError>);

    #[test]
    fn the_file_gives_its_required_statuses_or_inherits() {
        let queue =
            |body: &str| format!("version: 1\nqueues:\n  - name: main\n    branch: main\n{body}");
        let cases: Vec<FileCase> = vec![
            (
                queue("    required_statuses: [ci, lint]\n"),
                "main",
                Ok(Some(vec!["ci".into(), "lint".into()])),
            ),
            (queue("    required_statuses: null\n"), "main", Ok(None)),
            (queue(""), "main", Ok(None)),
            // The entry without the key: the defaults of the file apply.
            (
                format!("{}defaults:\n  required_statuses: [build]\n", queue("")),
                "main",
                Ok(Some(vec!["build".into()])),
            ),
            // No entry for the queue: neither the file nor its defaults apply.
            (
                format!(
                    "{}defaults:\n  required_statuses: [build]\n",
                    queue("    required_statuses: [ci]\n")
                ),
                "release",
                Ok(None),
            ),
            (
                queue("    required_statuses: []\n"),
                "main",
                Err(JobError::ConfigMissingRequiredStatuses),
            ),
            (
                queue("    required_statuses: [ci, ci]\n"),
                "main",
                Err(JobError::ConfigBadRequiredStatuses),
            ),
            (
                "queues: []\n".into(),
                "main",
                Err(JobError::ConfigBadVersion),
            ),
            (
                queue("    colour: blue\n"),
                "main",
                Err(JobError::ConfigParsingFailure),
            ),
        ];
        for (text, branch, want) in cases {
            assert_eq!(
                required_statuses_of_file(&text, branch, "main"),
                want,
                "{text}"
            );
        }
    }

    /// A repository on the simulator whose `main` requires `ci`, and the
    /// head of `main`.
    fn world() -> (Sim, QueueTarget, String) {
        let target = target();
        let sim = Sim::new();
        let main = sim.add_repo(&target.repo, "main", &[("a.txt", "a")]);
        let rules = RequiredChecks {
            names: BTreeSet::from(["ci".to_owned()]),
            up_to_date: false,
        };
        sim.set_rules(&target.repo, "main", rules).unwrap();
        (sim, target, main)
    }

    /// The head of a PR `n` with `files` on `parent`.
    fn head(
        sim: &Sim,
        target: &QueueTarget,
        parent: &str,
        n: u64,
        files: &[(&str, Option<&str>)],
    ) -> Head {
        let sha = sim.commit(&target.repo, parent, files).unwrap();
        sim.push(&target.repo, &format!("pr-{n}"), &sha, &bot())
            .unwrap();
        Head { pr_number: n, sha }
    }

    fn request(base_sha: &str, predecessors: Vec<Head>, members: Vec<Head>) -> AssembleTestBranch {
        let run_id = RunId::from_ulid(Ulid(9));
        AssembleTestBranch {
            run_id,
            test_branch: cihub_domain::test_branch("cihub", members[0].pr_number, run_id),
            base_sha: base_sha.to_owned(),
            predecessors,
            members,
        }
    }

    fn settings(edit: impl FnOnce(&mut Layer)) -> QueueSettings {
        let mut queue = Layer::default();
        edit(&mut queue);
        QueueSettings::resolve(&queue, &Layer::default()).unwrap()
    }

    async fn build(
        sim: &Sim,
        settings: &QueueSettings,
        target: &QueueTarget,
        request: &AssembleTestBranch,
    ) -> Result<Assembled, Step> {
        let assembly = Assembly {
            scm: sim,
            settings,
            target,
            request,
            class: JobClass::Speculate,
        };
        assembly.build().await
    }

    #[tokio::test]
    async fn the_assembly_merges_the_heads_in_order_and_opens_the_draft_pr() {
        let (sim, target, main) = world();
        let p1 = head(&sim, &target, &main, 1, &[("f1.txt", Some("1"))]);
        let m = head(&sim, &target, &main, 2, &[("f2.txt", Some("2"))]);
        let first = request(&main, vec![p1], vec![m]);
        let defaults = settings(|_| {});
        let assembled = build(&sim, &defaults, &target, &first).await.unwrap();
        assert_eq!(
            sim.branch(&target.repo, &first.test_branch),
            Some(assembled.test_sha.clone())
        );
        let temp = temp_branch(&first.test_branch).unwrap();
        assert_eq!(
            sim.branch(&target.repo, &temp),
            None,
            "the temporary branch is gone"
        );
        for path in ["f1.txt", "f2.txt"] {
            let file = sim
                .file(&target.repo, &assembled.test_sha, path, JobClass::Speculate)
                .await
                .unwrap();
            assert!(file.is_some(), "{path}");
        }
        assert_eq!(assembled.required_statuses, vec!["ci".to_owned()]);
        assert_eq!(
            assembled.required_statuses_source,
            RequiredStatusesSource::BranchProtection
        );
        let draft = assembled.draft_pr_number.unwrap();
        let pr = sim
            .pull_request(&target.repo, draft, JobClass::Speculate)
            .await
            .unwrap();
        assert!(pr.draft);
        assert_eq!(pr.base_branch, "main");
        // A second build finds the branch and the draft PR, and changes nothing.
        let again = build(&sim, &defaults, &target, &first).await.unwrap();
        assert_eq!(again.test_sha, assembled.test_sha);
        assert_eq!(again.draft_pr_number, Some(draft));
        // The trigger `push` opens no PR.
        let push = settings(|l| l.trigger = Some(Setting::new(Trigger::Push)));
        let m = head(&sim, &target, &main, 3, &[("f3.txt", Some("3"))]);
        let pushed = build(&sim, &push, &target, &request(&main, vec![], vec![m]))
            .await
            .unwrap();
        assert_eq!(pushed.draft_pr_number, None);
    }

    #[tokio::test]
    async fn a_conflict_names_the_base_or_the_predecessor_at_fault() {
        let (sim, target, root) = world();
        let settings = settings(|_| {});
        // The member changes a file that the target branch changed too.
        let m = head(&sim, &target, &root, 1, &[("a.txt", Some("x"))]);
        let main = sim
            .commit(&target.repo, &root, &[("a.txt", Some("y"))])
            .unwrap();
        sim.push(&target.repo, "main", &main, &bot()).unwrap();
        let base = build(&sim, &settings, &target, &request(&main, vec![], vec![m])).await;
        assert_eq!(
            base,
            Err(Step::GiveUp(
                JobError::MergeConflict,
                Some(ConflictCulprit::BaseBranch)
            ))
        );
        // The member conflicts with the second of its predecessors.
        let p1 = head(&sim, &target, &main, 2, &[("f.txt", Some("f"))]);
        let p2 = head(&sim, &target, &main, 3, &[("shared.txt", Some("3"))]);
        let m = head(&sim, &target, &main, 4, &[("shared.txt", Some("4"))]);
        let request_m = request(&main, vec![p1, p2.clone()], vec![m]);
        let culprit = build(&sim, &settings, &target, &request_m).await;
        assert_eq!(
            culprit,
            Err(Step::GiveUp(
                JobError::MergeConflict,
                Some(ConflictCulprit::QueuePr { pr_number: 3 })
            ))
        );
        // Two predecessors conflict with each other: the run of the second
        // one decides, this run waits.
        let p3 = head(&sim, &target, &main, 5, &[("shared.txt", Some("5"))]);
        let m = head(&sim, &target, &main, 6, &[("g.txt", Some("g"))]);
        let waits = build(
            &sim,
            &settings,
            &target,
            &request(&main, vec![p2, p3], vec![m]),
        )
        .await;
        assert!(
            matches!(waits, Err(Step::Wait(ref text)) if text.contains("#5")),
            "{waits:?}"
        );
        // A member whose head is already in the target has nothing to test
        // (`AGENTS.md` D130).
        let merged = head(&sim, &target, &main, 7, &[("h.txt", Some("h"))]);
        sim.push(&target.repo, "main", &merged.sha, &bot()).unwrap();
        let contained = build(
            &sim,
            &settings,
            &target,
            &request(&merged.sha, vec![], vec![merged.clone()]),
        )
        .await;
        assert_eq!(
            contained,
            Err(Step::GiveUp(
                JobError::MergeConflict,
                Some(ConflictCulprit::BaseBranch)
            ))
        );
    }

    #[tokio::test]
    async fn the_errors_of_the_scm_become_the_errors_of_the_run() {
        let (sim, target, main) = world();
        let settings = settings(|_| {});
        let forbidden = request(
            &main,
            vec![],
            vec![head(&sim, &target, &main, 1, &[("f1.txt", Some("1"))])],
        );
        sim.fail(Fault {
            call: Some("set_branch"),
            error: ScmError::Forbidden("Protected branch update failed".into()),
            times: 1,
        });
        assert_eq!(
            build(&sim, &settings, &target, &forbidden).await,
            Err(Step::GiveUp(JobError::PermissionsBlockTestBranch, None))
        );
        let down = request(
            &main,
            vec![],
            vec![head(&sim, &target, &main, 2, &[("f2.txt", Some("2"))])],
        );
        sim.fail(Fault {
            call: Some("merge_into_branch"),
            error: ScmError::Unavailable("502".into()),
            times: 1,
        });
        assert!(matches!(
            build(&sim, &settings, &target, &down).await,
            Err(Step::Passing(_))
        ));
        sim.fail(Fault {
            call: Some("merge_into_branch"),
            error: ScmError::Validation("422".into()),
            times: 1,
        });
        assert!(matches!(
            build(&sim, &settings, &target, &down).await,
            Err(Step::Transient(_))
        ));
        let no_drafts = request(
            &main,
            vec![],
            vec![head(&sim, &target, &main, 3, &[("f3.txt", Some("3"))])],
        );
        sim.fail(Fault {
            call: Some("open_draft_pr"),
            error: ScmError::Validation(
                "Draft pull requests are not supported in this repository.".into(),
            ),
            times: 1,
        });
        assert_eq!(
            build(&sim, &settings, &target, &no_drafts).await,
            Err(Step::GiveUp(JobError::DraftPrsNotAllowed, None))
        );
    }

    #[tokio::test]
    async fn the_required_statuses_come_from_the_strongest_source() {
        let (sim, target, main) = world();
        let member = |n, parent: &str| {
            let path = format!("f{n}.txt");
            vec![head(&sim, &target, parent, n, &[(&path, Some("x"))])]
        };
        // The file of the repository, read on the target branch at the base.
        let config = "version: 1\nqueues:\n  - name: main\n    branch: main\n    required_statuses: [build]\n";
        let with_file = sim
            .commit(&target.repo, &main, &[(".cihub.yaml", Some(config))])
            .unwrap();
        let defaults = settings(|_| {});
        let built = build(
            &sim,
            &defaults,
            &target,
            &request(&with_file, vec![], member(3, &with_file)),
        )
        .await
        .unwrap();
        assert_eq!(built.required_statuses, vec!["build".to_owned()]);
        assert_eq!(
            built.required_statuses_source,
            RequiredStatusesSource::RepoConfig
        );
        // Both names of the file: neither one wins.
        let both = sim
            .commit(&target.repo, &with_file, &[(".cihub.yml", Some(config))])
            .unwrap();
        assert_eq!(
            build(
                &sim,
                &defaults,
                &target,
                &request(&both, vec![], member(4, &both))
            )
            .await,
            Err(Step::GiveUp(JobError::ConfigParsingFailure, None))
        );
        // Without a file, the rules; without a rule, no source gives a status.
        sim.set_rules(&target.repo, "main", RequiredChecks::default())
            .unwrap();
        assert_eq!(
            build(
                &sim,
                &defaults,
                &target,
                &request(&main, vec![], member(5, &main))
            )
            .await,
            Err(Step::GiveUp(JobError::NoRequiredStatuses, None))
        );
    }

    /// The required statuses of a run and their source.
    type Statuses = (Vec<String>, RequiredStatusesSource);

    #[tokio::test]
    async fn merge_conditions_and_the_injection_shape_the_required_statuses() {
        let (sim, target, main) = world();
        let defaults = settings(|_| {});
        let mut n = 0;
        let mut built = |body: &str| {
            n += 1;
            let config = format!("version: 1\nqueues:\n  - name: main\n    branch: main\n{body}");
            let base = sim
                .commit(
                    &target.repo,
                    &main,
                    &[(".cihub.yaml", Some(config.as_str()))],
                )
                .unwrap();
            let path = format!("f{n}.txt");
            let member = vec![head(&sim, &target, &base, n, &[(&path, Some("x"))])];
            (base, member)
        };
        let statuses = |result: Result<Assembled, Step>| {
            result.map(|a| (a.required_statuses, a.required_statuses_source))
        };
        let cases: Vec<(&str, Result<Statuses, Step>)> = vec![
            // The rules, then the checks of the merge conditions.
            (
                "    merge_conditions:\n      - check-success = e2e\n      - check-success = ci\n",
                Ok((
                    vec!["ci".into(), "e2e".into()],
                    RequiredStatusesSource::BranchProtection,
                )),
            ),
            // A list of the file replaces the rules.
            (
                "    required_statuses: [lint]\n    merge_conditions:\n      - check-success = e2e\n",
                Ok((
                    vec!["lint".into(), "e2e".into()],
                    RequiredStatusesSource::RepoConfig,
                )),
            ),
            // `merge` keeps the rules on the test SHA.
            (
                "    branch_protection_injection: merge\n",
                Ok((vec!["ci".into()], RequiredStatusesSource::BranchProtection)),
            ),
            // `none` leaves them out: only the merge conditions are left.
            (
                "    branch_protection_injection: none\n    merge_conditions:\n      - check-success = e2e\n",
                Ok((vec!["e2e".into()], RequiredStatusesSource::RepoConfig)),
            ),
            (
                "    branch_protection_injection: none\n",
                Err(Step::GiveUp(JobError::NoRequiredStatuses, None)),
            ),
        ];
        for (body, want) in cases {
            let (base, member) = built(body);
            let got = build(&sim, &defaults, &target, &request(&base, vec![], member)).await;
            assert_eq!(statuses(got), want, "{body}");
        }
    }
}

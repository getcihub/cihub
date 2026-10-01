//! The merge queue module (`AGENTS.md` D93): the log of each queue in
//! Postgres, its projections (`queue_items`, `test_groups`, `test_runs`,
//! `check_observations`), its snapshots and their replay; the scheduler that
//! reconciles each queue; and the jobs that read the SCM for it and build its
//! test branches. The decisions come from `cihub-mq-core`; this crate writes
//! them. The scheduler does not call the SCM: the jobs do, and give their
//! facts back through `queue_inbox`.

mod assembly;
mod command;
mod jobs;
mod log;
mod projection;
mod scheduler;
mod target;

use std::sync::Arc;

use cihub_config::{Layer, QueueSettings};
use cihub_domain::ModuleName;
use cihub_platform::{LoopError, Module};
use cihub_scm::Scm;
use cihub_store::PgPool;
use time::OffsetDateTime;

pub use assembly::{
    ASSEMBLE_TEST_BRANCH, ASSEMBLY_ATTEMPTS, CLEANUP_TEST_BRANCH, draft_pr_body, draft_pr_title,
    required_statuses_of_file,
};
pub use jobs::{REFRESH_PRS, SUBMIT_PR, queue_status_context};
pub use log::{COVER_AFTER, QueueLog, SNAPSHOT_EVERY, rebuild};
pub use scheduler::{
    CONNECTION_DELIVERIES_PER_PASS, COVER_EVERY, INBOX_PER_RECONCILIATION, LEASES_PER_PASS,
    Reconciler, Translate, pass_connection_deliveries, run_scheduler, wake_uncovered,
};

#[derive(Debug, thiserror::Error)]
pub enum MergeQueueError {
    #[error("query failed")]
    Query(#[source] sqlx::Error),
    #[error("store failed")]
    Store(#[source] cihub_store::StoreError),
    #[error("transport failed")]
    Platform(#[source] cihub_platform::PlatformError),
    #[error("the default settings are not valid")]
    Settings(#[source] cihub_config::ConfigError),
    #[error("queue {0} does not exist")]
    UnknownQueue(cihub_domain::QueueId),
    #[error("repository {0} does not exist")]
    UnknownRepo(cihub_domain::RepoId),
    #[error("row {id} does not match the identifiers of cihub-domain")]
    ReadQueue {
        id: String,
        #[source]
        source: cihub_domain::IdParseError,
    },
    #[error("event {event_id} does not fit the snapshot of its queue")]
    Apply {
        event_id: cihub_domain::EventId,
        #[source]
        source: cihub_mq_core::ApplyError,
    },
    #[error("plan failed")]
    Plan(#[source] cihub_mq_core::PlanError),
    #[error("the log of queue {queue} has no event after {after}: its old events are gone")]
    LogGap {
        queue: cihub_domain::QueueId,
        after: i64,
    },
    #[error("the replay ignores event {0}, which the log keeps")]
    ReplayIgnored(cihub_domain::EventId),
    #[error("the projections do not match event {0}")]
    Projection(cihub_domain::EventId),
    #[error("cannot serialize or read a snapshot")]
    Snapshot(#[source] serde_json::Error),
}

/// The settings of every queue until lot 8 reads them from the file and the
/// dashboard.
///
/// # Errors
///
/// The defaults do not pass their own checks: a bug of `cihub-config`.
pub fn default_settings() -> Result<QueueSettings, MergeQueueError> {
    QueueSettings::resolve(&Layer::default(), &Layer::default(), &Layer::default())
        .map_err(MergeQueueError::Settings)
}

/// The clock of the process, as the module receives it.
pub type Clock = Arc<dyn Fn() -> OffsetDateTime + Send + Sync>;

/// The merge queue module (`AGENTS.md` D94): the executors of its jobs, which
/// call `scm`, and its scheduler loop, which runs as `owner`.
///
/// # Errors
///
/// The default settings are not valid.
pub fn module(
    pool: PgPool,
    scm: Arc<dyn Scm>,
    translate: Translate,
    owner: String,
    clock: Clock,
) -> Result<Module, MergeQueueError> {
    let settings = Arc::new(default_settings()?);
    let reconciler = Reconciler {
        settings: settings.clone(),
        translate,
    };
    Ok(Module {
        name: ModuleName::Mergequeue,
        executors: Box::new(move |executors| {
            let (refresh_scm, submit_scm, assemble_scm) = (scm.clone(), scm.clone(), scm.clone());
            let submit_settings = settings.clone();
            executors.register(REFRESH_PRS, move |pool, job, now| {
                jobs::execute_refresh(pool, refresh_scm.clone(), job, now)
            });
            executors.register(SUBMIT_PR, move |pool, job, now| {
                jobs::execute_submit(pool, submit_scm.clone(), submit_settings.clone(), job, now)
            });
            executors.register(ASSEMBLE_TEST_BRANCH, move |pool, job, now| {
                assembly::execute_assemble(pool, assemble_scm.clone(), settings.clone(), job, now)
            });
            executors.register(CLEANUP_TEST_BRANCH, move |pool, job, _| {
                assembly::execute_cleanup(pool, scm.clone(), job)
            });
        }),
        scheduler: Box::new(move |cancel| {
            Box::pin(async move {
                run_scheduler(pool, owner, reconciler, move || clock(), cancel)
                    .await
                    .map_err(|err| Box::new(err) as LoopError)
            })
        }),
    })
}

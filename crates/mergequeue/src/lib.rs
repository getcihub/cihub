//! The merge queue module (`DECISIONS.md` D93): the log of each queue in
//! Postgres, its projections (`queue_items`, `test_groups`, `test_runs`,
//! `check_observations`), its snapshots and their replay; the scheduler that
//! reconciles each queue; and the jobs that read the SCM for it, build its
//! test branches, merge its PRs, show their state on each PR and repair what a
//! lost webhook leaves. The decisions come from `cihub-mq-core`; this crate writes
//! them. The scheduler does not call the SCM: the jobs do, and give their
//! facts back through `queue_inbox`.

mod admission;
mod api;
mod assembly;
mod command;
mod config;
mod jobs;
mod log;
mod merge;
mod projection;
mod repair;
mod scheduler;
mod surfaces;
mod target;

use std::sync::Arc;

use cihub_platform::{LoopError, Module};
use cihub_scm::Scm;
use cihub_store::PgPool;
use time::OffsetDateTime;

pub use jobs::{REFRESH_PRS, queue_status_context};
pub use log::{QueueLog, SNAPSHOT_EVERY, rebuild};
pub use repair::resync;
pub use scheduler::{
    Reconciler, Translate, pass_connection_deliveries, wake_live_queues, wake_uncovered,
};

use crate::assembly::{ASSEMBLE_TEST_BRANCH, CLEANUP_TEST_BRANCH};
use crate::command::PR_COMMAND;
use crate::config::LOAD_CONFIG;
use crate::jobs::SUBMIT_PR;
use crate::merge::MERGE_PR;
use crate::repair::{REFRESH_CHECKS, REFRESH_TARGET};
use crate::scheduler::run_scheduler;
use crate::surfaces::{PUBLISH_SURFACES, WELCOME_PR};

#[derive(Debug, thiserror::Error)]
pub enum MergeQueueError {
    #[error("query failed")]
    Query(#[source] sqlx::Error),
    #[error("store failed")]
    Store(#[source] cihub_store::StoreError),
    #[error("transport failed")]
    Platform(#[source] cihub_platform::PlatformError),
    #[error("the settings of the queue are not valid")]
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
    #[error("event {id} does not match the events of cihub-domain")]
    ReadEvent {
        id: String,
        #[source]
        source: serde_json::Error,
    },
}

/// Reads `text`, a column of the row `row`, as an identifier of
/// `cihub-domain`.
pub(crate) fn parse_id<T>(text: &str, row: impl std::fmt::Display) -> Result<T, MergeQueueError>
where
    T: std::str::FromStr<Err = cihub_domain::IdParseError>,
{
    text.parse().map_err(|source| MergeQueueError::ReadQueue {
        id: row.to_string(),
        source,
    })
}

/// The clock of the process, as the module receives it.
pub type Clock = Arc<dyn Fn() -> OffsetDateTime + Send + Sync>;

/// Runs the jobs of `job_type` with `run`, an `async fn(&PgPool, &dyn Scm,
/// &Job, OffsetDateTime) -> Result<(), Failure>`. A macro: a generic
/// function would need a helper trait to name the future of an `async fn`
/// that borrows its arguments.
macro_rules! register {
    ($executors:expr, $scm:expr, $job_type:expr, $run:path) => {{
        let scm = Arc::clone(&$scm);
        $executors.register($job_type, move |pool, job, now| {
            let scm = Arc::clone(&scm);
            async move { jobs::outcome($run(&pool, scm.as_ref(), &job, now).await) }
        });
    }};
}

/// The merge queue module (`DECISIONS.md` D94): the executors of its jobs, which
/// call `scm`, and its scheduler loop, which runs as `owner`.
pub fn module(
    pool: PgPool,
    scm: Arc<dyn Scm>,
    translate: Translate,
    owner: String,
    clock: Clock,
) -> Module {
    let reconciler = Reconciler { translate };
    Module {
        api: api::api(pool.clone(), clock.clone(), scm.clone()),
        executors: Box::new(move |executors| {
            register!(executors, scm, REFRESH_PRS, jobs::refresh);
            register!(executors, scm, SUBMIT_PR, jobs::submit_pr);
            register!(executors, scm, ASSEMBLE_TEST_BRANCH, assembly::assemble_run);
            register!(executors, scm, CLEANUP_TEST_BRANCH, assembly::cleanup_run);
            register!(executors, scm, MERGE_PR, merge::merge_pr);
            register!(executors, scm, REFRESH_CHECKS, repair::read_checks);
            register!(executors, scm, REFRESH_TARGET, repair::read_target);
            register!(executors, scm, LOAD_CONFIG, config::load);
            register!(executors, scm, PR_COMMAND, command::run_pr_command);
            register!(executors, scm, WELCOME_PR, surfaces::post_welcome);
            register!(executors, scm, PUBLISH_SURFACES, surfaces::publish);
        }),
        scheduler: Box::new(move |cancel| {
            Box::pin(async move {
                run_scheduler(pool, owner, reconciler, move || clock(), cancel)
                    .await
                    .map_err(|err| Box::new(err) as LoopError)
            })
        }),
    }
}

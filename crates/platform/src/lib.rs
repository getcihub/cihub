//! Internal transport of CIHub on Postgres: the queue leases, the inbox of the
//! webhook deliveries, the wake-up of the schedulers, the jobs, their worker
//! loop, the daily maintenance (lot 3), the budgets of the connections, the
//! routing and the sweep of the deliveries, the sync of the installations of
//! the App into organizations and connections, the read of the repositories
//! of the connections, the API tokens of an operator, the contract of a
//! module, and what the routes of the modules share. The roles talk only through these tables and their notifications.
//! It holds no queue logic: it decides who works on a queue and in which order
//! its deliveries come, not what the queue does.

mod api;
mod budget;
mod connections;
mod inbox;
mod installations;
mod jobs;
mod lease;
mod maintenance;
mod module;
mod routing;
mod sweep;
mod tokens;
mod wake;
mod worker;

pub use api::{ApiError, ApiGrant, ApiJson, ApiPath, ApiQuery};
pub use budget::{ConnectionBudget, record_budgets, run_budget_writer};
pub use connections::{
    SYNC_REPOS, request_repo_syncs, run_sync_repos, sync_repos_executor, sync_repos_job,
};
pub use inbox::{Delivery, delete_delivery, pending_deliveries, store_delivery};
pub use installations::{
    SYNC_INSTALLATION, SYNC_INSTALLATIONS, request_installations_sync,
    request_installations_sync_at_start, run_sync_installation, run_sync_installations,
    sync_installation_executor, sync_installation_job, sync_installations_executor,
};
pub use jobs::{
    JOB_LOCK, JOBS_PER_CONNECTION, Job, MAX_ATTEMPTS, Retry, backoff, claim_jobs, complete_job,
    enqueue_job, fail_job,
};
pub use lease::{LEASE_TTL, acquire_leases, lock_queue, release_leases};
pub use maintenance::{
    MAINTENANCE, execute_maintenance, maintenance_job, request_maintenance, run_maintenance,
};
pub use module::{LoopError, Module, ModuleApi, ModuleLoop};
pub use routing::{ROUTE_POLL, Route, find_route, run_router};
pub use sweep::{SWEEP_DELIVERIES, SWEEP_EVERY, request_sweep, run_sweep, sweep_executor};
pub use tokens::create_token;
pub use wake::{
    MIN_WAIT, QueueWakeup, TICK, Wake, begin_reconciliation, mark_dirty, schedule_tick,
    until_next_tick,
};
pub use worker::{Executors, JOB_POLL, Outcome, error_chain, run_worker};

#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("query failed")]
    Query(#[source] sqlx::Error),
    #[error("store failed")]
    Store(#[source] cihub_store::StoreError),
    #[error("SCM call failed")]
    Scm(#[source] cihub_scm::ScmError),
    #[error("{what} {id} does not match cihub-domain")]
    Read {
        what: &'static str,
        id: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("job {0} has no connection")]
    JobWithoutConnection(String),
    #[error("job {0} has no installation")]
    JobWithoutInstallation(String),
    #[error(
        "the installation {installation} cannot create the organization {slug}: another organization has this slug"
    )]
    SlugTaken { slug: String, installation: String },
    #[error("no organization has the slug {0}")]
    UnknownOrganization(String),
}

impl PlatformError {
    pub(crate) fn read<E: Into<Box<dyn std::error::Error + Send + Sync>>>(
        what: &'static str,
        id: &str,
    ) -> impl FnOnce(E) -> Self {
        let id = id.to_owned();
        move |source| Self::Read {
            what,
            id,
            source: source.into(),
        }
    }
}

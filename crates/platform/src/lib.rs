//! Internal transport of CIHub on Postgres: the queue leases, the inbox of the
//! webhook deliveries, the wake-up of the schedulers, the jobs, their worker
//! loop, the daily maintenance (lot 3), the budgets of the connections, and the
//! routing and the sweep of the deliveries. The roles talk only through these
//! tables and their notifications. It holds no queue logic: it decides who
//! works on a queue and in which order its deliveries come, not what the queue
//! does.

mod budget;
mod inbox;
mod jobs;
mod lease;
mod maintenance;
mod routing;
mod sweep;
mod wake;
mod worker;

pub use budget::{BUDGET_WRITE_EVERY, ConnectionBudget, record_budgets, run_budget_writer};
pub use inbox::{Delivery, INBOX_CHANNEL, mark_processed, pending_deliveries, store_delivery};
pub use jobs::{
    JOB_CHANNEL, JOB_LOCK, JOBS_PER_CONNECTION, Job, MAX_ATTEMPTS, Retry, backoff, claim_jobs,
    complete_job, enqueue_job, fail_job,
};
pub use lease::{LEASE_TTL, acquire_leases, lock_queue, release_leases};
pub use maintenance::{
    DELIVERY_RETENTION, DONE_JOB_RETENTION, MAINTENANCE, execute_maintenance, maintenance_job,
    request_maintenance, run_maintenance,
};
pub use routing::{
    ROUTE_POLL, ROUTES_PER_PASS, Route, UnroutedDelivery, find_route, route_delivery, run_router,
    unrouted_deliveries,
};
pub use sweep::{
    SWEEP_DELIVERIES, SWEEP_EVERY, SWEEP_LOOKBACK, request_sweep, run_sweep, sweep_executor,
    sweep_job,
};
pub use wake::{
    QUEUE_CHANNEL, QueueWakeup, TICK, Wake, begin_reconciliation, mark_dirty, schedule_tick,
};
pub use worker::{
    Executors, JOB_POLL, JOB_TIMEOUT, JOBS_PER_CLAIM, Outcome, error_chain, run_worker,
};

#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("query failed")]
    Query(#[source] sqlx::Error),
    #[error("store failed")]
    Store(#[source] cihub_store::StoreError),
    #[error("SCM call failed")]
    Scm(#[source] cihub_scm::ScmError),
    #[error("queue {id} does not match the identifiers of cihub-domain")]
    ReadQueue {
        id: String,
        #[source]
        source: cihub_domain::IdParseError,
    },
    #[error("delivery {id} does not match the identifiers of cihub-domain")]
    ReadDelivery {
        id: String,
        #[source]
        source: cihub_domain::IdParseError,
    },
    #[error("job {key} does not match the vocabulary of cihub-domain")]
    ReadJob {
        key: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

//! Internal transport of CIHub on Postgres: the queue leases, the inbox of the
//! webhook deliveries, the wake-up of the schedulers, the jobs, their worker
//! loop and the daily maintenance (lot 3). The roles talk only through these
//! tables and their notifications. It holds no queue logic: it decides who works on
//! a queue and in which order its deliveries come, not what the queue does.

mod inbox;
mod jobs;
mod lease;
mod maintenance;
mod wake;
mod worker;

pub use inbox::{Delivery, mark_processed, pending_deliveries, store_delivery};
pub use jobs::{
    JOB_CHANNEL, JOB_LOCK, JOBS_PER_CONNECTION, Job, MAX_ATTEMPTS, Retry, backoff, claim_jobs,
    complete_job, enqueue_job, fail_job,
};
pub use lease::{LEASE_TTL, acquire_leases, lock_queue, release_leases};
pub use maintenance::{
    DELIVERY_RETENTION, DONE_JOB_RETENTION, MAINTENANCE, execute_maintenance, maintenance_job,
    request_maintenance, run_maintenance,
};
pub use wake::{
    QUEUE_CHANNEL, QueueWakeup, TICK, Wake, begin_reconciliation, mark_dirty, schedule_tick,
};
pub use worker::{Executors, JOB_POLL, JOBS_PER_CLAIM, Outcome, error_chain, run_worker};

#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("query failed")]
    Query(#[source] sqlx::Error),
    #[error("store failed")]
    Store(#[source] cihub_store::StoreError),
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

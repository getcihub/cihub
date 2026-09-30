//! Internal transport of CIHub on Postgres: the queue leases, the inbox of the
//! webhook deliveries and the wake-up of the schedulers (lot 3). The roles talk
//! only through these tables and their notifications. It holds no queue logic: it decides who works on
//! a queue and in which order its deliveries come, not what the queue does.

mod inbox;
mod lease;
mod wake;

pub use inbox::{Delivery, mark_processed, pending_deliveries, store_delivery};
pub use lease::{LEASE_TTL, acquire_leases, lock_queue, release_leases};
pub use wake::{
    QUEUE_CHANNEL, QueueWakeup, TICK, Wake, begin_reconciliation, mark_dirty, schedule_tick,
};

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
}

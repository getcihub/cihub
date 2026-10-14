//! Analytics of the queues (`specs/ANALYTICS.md`): the facts that the log of a
//! queue concludes (an ended item, its passes through the states, an ended run
//! and its checks), written in the `an_*` tables from the events only, and the
//! Health of a queue read from them. It reads no projection, so that a replay
//! of the log writes the same facts; it does not choose when the facts are
//! written: the subscription of the merge queue does.

mod facts;
mod health;

pub use facts::{SUBSCRIPTION, record_facts};
pub use health::{Bucket, CheckHealth, Health, Period, TimeToMerge, Totals, Verdict, queue_health};

#[derive(Debug, thiserror::Error)]
pub enum AnalyticsError {
    #[error("query failed")]
    Query(#[from] sqlx::Error),
    #[error("event {id} does not match the events of cihub-domain")]
    ReadEvent {
        id: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("`{value}` does not match cihub-domain")]
    ReadValue {
        value: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

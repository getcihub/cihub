use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::sync::Arc;

use cihub_domain::JobClass;
use cihub_scm::Scm;
use cihub_store::PgPool;
use serde_json::json;
use sqlx::PgConnection;
use time::{Duration, OffsetDateTime};
use tracing::info;

use crate::jobs::hour_window;
use crate::{Job, Outcome, PlatformError, enqueue_job, error_chain};

pub const SWEEP_DELIVERIES: &str = "sweep_deliveries";

pub const SWEEP_EVERY: std::time::Duration = std::time::Duration::from_mins(5);

pub(crate) const SWEEP_LOOKBACK: Duration = Duration::minutes(10);

pub(crate) fn sweep_job(now: OffsetDateTime) -> Job {
    let window = format!("{}{:02}", hour_window(now), now.minute() - now.minute() % 5);
    Job {
        idempotency_key: format!("{SWEEP_DELIVERIES}:{window}:1"),
        job_type: SWEEP_DELIVERIES.to_owned(),
        class: JobClass::Repair,
        org_id: None,
        connection: None,
        queue_id: None,
        payload: json!({}),
        attempts: 0,
    }
}

/// # Errors
///
/// Postgres fails.
pub async fn request_sweep(
    conn: &mut PgConnection,
    now: OffsetDateTime,
) -> Result<bool, PlatformError> {
    enqueue_job(conn, &sweep_job(now), now, now).await
}

/// Lists the attempts of the last [`SWEEP_LOOKBACK`], and asks the SCM again
/// for each delivery that no attempt delivered: CIHub answers 2xx only once
/// it stores a delivery. Returns the number of
/// deliveries asked again. A delivery processed again writes no wrong fact
/// and asks each of its jobs at most once, so a delivery asked twice does no
/// harm.
///
/// # Errors
///
/// The SCM fails.
pub async fn run_sweep(scm: &dyn Scm, now: OffsetDateTime) -> Result<usize, PlatformError> {
    let recent = scm
        .recent_deliveries(now - SWEEP_LOOKBACK, JobClass::Repair)
        .await
        .map_err(PlatformError::Scm)?;

    let mut delivered = BTreeSet::new();
    let mut undelivered = BTreeMap::new();

    for attempt in recent {
        if attempt.is_delivered {
            delivered.insert(attempt.delivery_id);
            continue;
        }

        undelivered
            .entry(attempt.delivery_id)
            .or_insert(attempt.attempt_id);
    }

    let mut asked = 0;
    for (delivery_id, attempt_id) in &undelivered {
        if delivered.contains(delivery_id) {
            continue;
        }

        scm.redeliver(attempt_id, JobClass::Repair)
            .await
            .map_err(PlatformError::Scm)?;
        info!(delivery_id, "delivery.redelivery_requested");
        asked += 1;
    }

    Ok(asked)
}

pub fn sweep_executor(
    scm: Arc<dyn Scm>,
) -> impl Fn(PgPool, Job, OffsetDateTime) -> std::pin::Pin<Box<dyn Future<Output = Outcome> + Send>>
+ Send
+ Sync
+ 'static {
    move |_pool, _job, now| {
        let scm = Arc::clone(&scm);
        Box::pin(async move {
            match run_sweep(scm.as_ref(), now).await {
                Ok(_) => Outcome::Done,
                Err(err) => Outcome::Retry(error_chain(&err)),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use cihub_domain::ConnectionId;
    use cihub_scm::sim::{Sim, WebhookFaults};
    use cihub_scm::{ConnectionRef, RepoRef, ScmDelivery, ScmEvent, ScmUser};

    use super::*;

    #[test]
    fn a_sweep_has_one_job_per_five_minute_window() {
        let at = |minute| {
            OffsetDateTime::from_unix_timestamp(1_789_000_000)
                .unwrap()
                .replace_minute(minute)
                .unwrap()
        };
        let key = |minute| sweep_job(at(minute)).idempotency_key;
        assert_eq!(key(5), key(9));
        assert_ne!(key(9), key(10));
        assert_eq!(key(7), "sweep_deliveries:20260910T0005:1");
    }

    #[tokio::test]
    async fn the_sweep_asks_again_only_for_the_deliveries_that_no_attempt_delivered() {
        let repo = RepoRef {
            connection: ConnectionRef {
                id: ConnectionId::from_ulid(ulid::Ulid(1)),
                external_id: "1".into(),
            },
            external_id: "1".into(),
            owner: "acme".into(),
            name: "app".into(),
        };
        let sim = Sim::new();
        let root = sim.add_repo(&repo, "main", &[]);
        let alice = ScmUser {
            external_id: "3".into(),
            login: "alice".into(),
        };
        let lose_all = &mut WebhookFaults {
            seed: 1,
            lose_percent: 100,
            duplicate_percent: 0,
            reorder: false,
        };

        sim.push(&repo, "a", &root, &alice).unwrap();
        sim.take_deliveries();
        sim.push(&repo, "b", &root, &alice).unwrap();

        assert_eq!(sim.take_deliveries_with(lose_all), []);
        sim.push(&repo, "c", &root, &alice).unwrap();
        sim.take_deliveries();

        let now = OffsetDateTime::from_unix_timestamp(1_789_000_000).unwrap();
        assert_eq!(run_sweep(&sim, now).await.unwrap(), 1);
        let again = sim.take_deliveries();
        assert!(
            matches!(&again[..], [ScmDelivery { event: ScmEvent::Pushed { branch, .. }, .. }] if branch == "b"),
            "{again:?}"
        );
    }
}

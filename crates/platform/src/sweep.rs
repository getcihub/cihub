use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;

use cihub_domain::{JobClass, ScmInstanceId};
use cihub_scm::Scm;
use cihub_store::{PgPool, begin_system};
use serde_json::json;
use sqlx::PgConnection;
use time::{Duration, OffsetDateTime};
use tracing::info;

use crate::{Job, Outcome, PlatformError, enqueue_job, error_chain};

/// The type of the job that asks the SCM again for the deliveries missing
/// from the inbox (`AGENTS.md` D88).
pub const SWEEP_DELIVERIES: &str = "sweep_deliveries";

/// A scheduler requests the sweep this often.
pub const SWEEP_EVERY: std::time::Duration = std::time::Duration::from_mins(5);

/// A sweep looks at the attempts of this last period: two periods, so that a
/// sweep that fails leaves no hole.
pub const SWEEP_LOOKBACK: Duration = Duration::minutes(10);

/// The sweep job of `instance` for the 5-minute window of `now`: its key
/// admits one job per window, whatever the number of schedulers.
pub fn sweep_job(instance: ScmInstanceId, now: OffsetDateTime) -> Job {
    let window = format!(
        "{:04}{:02}{:02}T{:02}{:02}",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute() - now.minute() % 5
    );
    Job {
        idempotency_key: format!("{SWEEP_DELIVERIES}:{instance}-{window}:1"),
        job_type: SWEEP_DELIVERIES.to_owned(),
        class: JobClass::Repair,
        org_id: None,
        connection: None,
        queue_id: None,
        payload: json!({}),
        attempts: 0,
    }
}

/// Requests the sweep of `instance` for the window of `now`.
///
/// # Errors
///
/// Postgres fails.
pub async fn request_sweep(
    conn: &mut PgConnection,
    instance: ScmInstanceId,
    now: OffsetDateTime,
) -> Result<bool, PlatformError> {
    enqueue_job(conn, &sweep_job(instance, now), now, now).await
}

/// Lists the attempts of the last [`SWEEP_LOOKBACK`], and asks the SCM again
/// for each delivery that the inbox of `instance` does not have. Returns the
/// number of deliveries asked again. A copy of a delivery changes nothing in
/// the inbox, so a delivery asked twice does no harm.
///
/// # Errors
///
/// Postgres or the SCM fails.
pub async fn run_sweep(
    pool: &PgPool,
    scm: &dyn Scm,
    instance: ScmInstanceId,
    now: OffsetDateTime,
) -> Result<usize, PlatformError> {
    let recent = scm
        .recent_deliveries(now - SWEEP_LOOKBACK, JobClass::Repair)
        .await
        .map_err(PlatformError::Scm)?;
    let mut attempts = BTreeMap::new();
    for attempt in recent {
        attempts
            .entry(attempt.delivery_id)
            .or_insert(attempt.attempt_id);
    }
    let ids: Vec<&String> = attempts.keys().collect();
    let mut tx = begin_system(pool).await.map_err(PlatformError::Store)?;
    let known: Vec<String> = sqlx::query_scalar(
        "SELECT delivery_id FROM webhook_inbox
         WHERE scm_instance_id = $1 AND delivery_id = ANY($2)",
    )
    .bind(instance.to_string())
    .bind(&ids)
    .fetch_all(&mut *tx)
    .await
    .map_err(PlatformError::Query)?;
    tx.commit().await.map_err(PlatformError::Query)?;
    let mut asked = 0;
    for (delivery_id, attempt_id) in &attempts {
        if known.contains(delivery_id) {
            continue;
        }
        scm.redeliver(attempt_id, JobClass::Repair)
            .await
            .map_err(PlatformError::Scm)?;
        info!(delivery_id, "delivery asked again");
        asked += 1;
    }
    Ok(asked)
}

/// The executor of [`SWEEP_DELIVERIES`] for `instance`, with its SCM
/// (`AGENTS.md` D71).
pub fn sweep_executor(
    scm: Arc<dyn Scm>,
    instance: ScmInstanceId,
) -> impl Fn(PgPool, Job, OffsetDateTime) -> std::pin::Pin<Box<dyn Future<Output = Outcome> + Send>>
+ Send
+ Sync
+ 'static {
    move |pool, _job, now| {
        let scm = Arc::clone(&scm);
        Box::pin(async move {
            match run_sweep(&pool, scm.as_ref(), instance, now).await {
                Ok(_) => Outcome::Done,
                Err(err) => Outcome::Retry(error_chain(&err)),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use ulid::Ulid;

    use super::*;

    #[test]
    fn a_sweep_has_one_job_per_five_minute_window() {
        let instance = ScmInstanceId::from_ulid(Ulid(1));
        let at = |minute| {
            OffsetDateTime::from_unix_timestamp(1_789_000_000)
                .unwrap()
                .replace_minute(minute)
                .unwrap()
        };
        let key = |minute| sweep_job(instance, at(minute)).idempotency_key;
        assert_eq!(key(5), key(9));
        assert_ne!(key(9), key(10));
        assert_eq!(
            key(7),
            format!("sweep_deliveries:{instance}-20260910T0005:1")
        );
    }
}

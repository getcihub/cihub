use std::collections::HashMap;
use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use cihub_domain::JobState;
use cihub_store::begin_system;
use sqlx::PgPool;
use sqlx::postgres::PgListener;
use time::OffsetDateTime;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, error, info, info_span, warn};

use crate::{JOB_CHANNEL, Job, PlatformError, Retry, claim_jobs, complete_job, fail_job};

/// A worker without a notification looks for due jobs this often: a retry
/// after its backoff sends no notification.
pub const JOB_POLL: Duration = Duration::from_secs(5);

/// A worker claims at most this many jobs at a time and runs them together.
pub const JOBS_PER_CLAIM: i64 = 16;

/// A worker stops an executor after this long and retries its job, so that
/// the lock of the job ([`JOB_LOCK`](crate::JOB_LOCK)) does not expire while
/// the executor runs and another worker does not run the job at the same
/// time. An executor must accept to stop at any `.await`.
pub const JOB_TIMEOUT: Duration = Duration::from_secs(50);

/// What an executor answers for a job (`AGENTS.md` D66).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Done,
    /// Try again after the backoff, with the error for the operator.
    Retry(String),
    /// A retry cannot fix the error.
    Failed(String),
}

type ExecutorFuture = Pin<Box<dyn Future<Output = Outcome> + Send>>;
type Executor = Box<dyn Fn(PgPool, Job, OffsetDateTime) -> ExecutorFuture + Send + Sync>;

/// The registry of the worker: the executor of each job type.
#[derive(Default)]
pub struct Executors {
    by_type: HashMap<&'static str, Executor>,
}

impl Executors {
    /// Runs the jobs of `job_type` with `run`, which receives the pool, the
    /// job and the time of the claim.
    pub fn register<F, Fut>(&mut self, job_type: &'static str, run: F)
    where
        F: Fn(PgPool, Job, OffsetDateTime) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Outcome> + Send + 'static,
    {
        self.by_type.insert(
            job_type,
            Box::new(move |pool, job, now| Box::pin(run(pool, job, now))),
        );
    }

    fn run(&self, pool: PgPool, job: Job, now: OffsetDateTime) -> ExecutorFuture {
        if let Some(executor) = self.by_type.get(job.job_type.as_str()) {
            return executor(pool, job, now);
        }
        let outcome = Outcome::Failed(format!("no executor for job type `{}`", job.job_type));
        Box::pin(async move { outcome })
    }
}

/// Claims the due jobs as `worker` and runs each with its executor, until
/// `cancel`; then waits for the jobs in progress. Wakes up on a new job, or
/// every `idle_wait`. An error of Postgres is logged and the worker tries
/// again after `idle_wait`: a job it could not record goes back to another
/// worker when its lock expires.
///
/// # Errors
///
/// Postgres cannot be reached at the start, to listen to [`JOB_CHANNEL`].
pub async fn run_worker(
    pool: PgPool,
    worker: String,
    executors: Arc<Executors>,
    clock: impl Fn() -> OffsetDateTime + Send + Sync,
    idle_wait: Duration,
    cancel: CancellationToken,
) -> Result<(), PlatformError> {
    let mut listener = PgListener::connect_with(&pool)
        .await
        .map_err(PlatformError::Query)?;
    listener
        .listen(JOB_CHANNEL)
        .await
        .map_err(PlatformError::Query)?;
    while !cancel.is_cancelled() {
        let now = clock();
        let jobs = match claim(&pool, &worker, now).await {
            Ok(jobs) => jobs,
            Err(err) => {
                warn!(error = &err as &dyn Error, "jobs not claimed");
                Vec::new()
            }
        };
        if jobs.is_empty() {
            tokio::select! {
                () = cancel.cancelled() => {}
                received = listener.recv() => {
                    if let Err(err) = received {
                        warn!(error = &err as &dyn Error, "job notifications lost");
                        tokio::select! {
                            () = cancel.cancelled() => {}
                            () = tokio::time::sleep(idle_wait) => {}
                        }
                    }
                }
                () = tokio::time::sleep(idle_wait) => {}
            }
            continue;
        }
        let mut running = JoinSet::new();
        for job in jobs {
            let span = info_span!("job", key = job.idempotency_key, job_type = job.job_type);
            let key = job.idempotency_key.clone();
            let outcome = within(JOB_TIMEOUT, executors.run(pool.clone(), job, now));
            let run = outcome.instrument(span.clone());
            running.spawn(async move { (key, run.await, span) });
        }
        while let Some(joined) = running.join_next().await {
            let Ok((key, outcome, span)) = joined else {
                warn!("job panicked");
                continue;
            };
            record(&pool, &key, &worker, clock(), &outcome)
                .instrument(span)
                .await;
        }
    }
    Ok(())
}

async fn within(limit: Duration, run: ExecutorFuture) -> Outcome {
    tokio::time::timeout(limit, run).await.unwrap_or_else(|_| {
        Outcome::Retry(format!("job still running after {} ms", limit.as_millis()))
    })
}

async fn claim(
    pool: &PgPool,
    worker: &str,
    now: OffsetDateTime,
) -> Result<Vec<Job>, PlatformError> {
    let mut tx = begin_system(pool).await.map_err(PlatformError::Store)?;
    let jobs = claim_jobs(&mut tx, worker, now, JOBS_PER_CLAIM).await?;
    tx.commit().await.map_err(PlatformError::Query)?;
    Ok(jobs)
}

/// Records the outcome of a job and writes one line for it. A result that
/// cannot be recorded is logged: the lock of the job expires and another
/// worker runs it again.
async fn record(pool: &PgPool, key: &str, worker: &str, now: OffsetDateTime, outcome: &Outcome) {
    let state = match store_outcome(pool, key, worker, now, outcome).await {
        Ok(state) => state,
        Err(err) => {
            warn!(error = &err as &dyn Error, "job result not recorded");
            return;
        }
    };
    match (state, outcome) {
        (None, _) => warn!("job taken back before its result"),
        (Some(JobState::Done), _) => info!("job done"),
        (Some(JobState::Queued), Outcome::Retry(error)) => warn!(error, "job retried"),
        (Some(JobState::Dead), Outcome::Retry(error)) => error!(error, "job dead"),
        (Some(_), Outcome::Failed(error) | Outcome::Retry(error)) => warn!(error, "job failed"),
        (Some(_), Outcome::Done) => {}
    }
}

async fn store_outcome(
    pool: &PgPool,
    key: &str,
    worker: &str,
    now: OffsetDateTime,
    outcome: &Outcome,
) -> Result<Option<JobState>, PlatformError> {
    let mut tx = begin_system(pool).await.map_err(PlatformError::Store)?;
    let state = match outcome {
        Outcome::Done => complete_job(&mut tx, key, worker, now)
            .await?
            .then_some(JobState::Done),
        Outcome::Retry(error) => fail_job(&mut tx, key, worker, now, error, Retry::Later).await?,
        Outcome::Failed(error) => fail_job(&mut tx, key, worker, now, error, Retry::Never).await?,
    };
    tx.commit().await.map_err(PlatformError::Query)?;
    Ok(state)
}

/// `err` and its sources, separated by `: `, for the `last_error` of a job.
pub fn error_chain(err: &dyn Error) -> String {
    let mut text = err.to_string();
    let mut source = err.source();
    while let Some(err) = source {
        text.push_str(": ");
        text.push_str(&err.to_string());
        source = err.source();
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_ends_before_the_lock() {
        assert!(JOB_TIMEOUT < crate::JOB_LOCK);
    }

    #[tokio::test]
    async fn a_slow_executor_is_retried() {
        let slow: ExecutorFuture = Box::pin(std::future::pending());
        let outcome = within(Duration::from_millis(10), slow).await;
        assert_eq!(
            outcome,
            Outcome::Retry("job still running after 10 ms".into())
        );
    }

    #[tokio::test]
    async fn a_fast_executor_keeps_its_outcome() {
        let fast: ExecutorFuture = Box::pin(async { Outcome::Done });
        assert_eq!(within(JOB_TIMEOUT, fast).await, Outcome::Done);
    }
}

//! The `scheduler` role: the reconciliation loop of the queues, from
//! `cihub-mergequeue`, and the regular requests of the maintenance, of the
//! reads of the repositories and of the sweep of the deliveries.

use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use anyhow::anyhow;
use cihub_domain::ScmInstanceId;
use cihub_platform::{SWEEP_EVERY, request_maintenance, request_repo_syncs, request_sweep};
use cihub_store::{PgPool, Postgres, Transaction, begin_system};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use super::start::Shared;
use super::{merge_queue, now};

pub(super) async fn run(shared: Shared) -> anyhow::Result<()> {
    let Shared {
        pool,
        process,
        github,
        cancel,
        ..
    } = shared;
    let module = merge_queue(&pool, &github, &process);
    let queues = (module.scheduler)(cancel.clone());
    let maintenance = request_hourly(&pool, &cancel);
    let sweep = request_sweep_regularly(&pool, github.scm_instance_id, &cancel);
    let (queues, (), ()) = tokio::join!(queues, maintenance, sweep);
    queues.map_err(|err| anyhow!(err).context("the scheduler of the queues stopped"))
}

async fn repeat_every<Fut: Future<Output = ()>>(
    every: Duration,
    cancel: &CancellationToken,
    mut request: impl FnMut() -> Fut,
) {
    let mut interval = tokio::time::interval(every);
    loop {
        tokio::select! {
            () = cancel.cancelled() => return,
            _ = interval.tick() => request().await,
        }
    }
}

/// The work of one system transaction; boxed, because a closure cannot name
/// the future that borrows its argument.
type InTransaction<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

async fn in_system_transaction<T, E>(
    pool: &PgPool,
    request: impl for<'a> FnOnce(
        &'a mut Transaction<'static, Postgres>,
    ) -> InTransaction<'a, Result<T, E>>,
) -> anyhow::Result<T>
where
    anyhow::Error: From<E>,
{
    let mut tx = begin_system(pool).await?;
    let result = request(&mut tx).await?;
    tx.commit().await?;
    Ok(result)
}

async fn request_hourly(pool: &PgPool, cancel: &CancellationToken) {
    repeat_every(Duration::from_hours(1), cancel, || async move {
        if let Err(err) = request_maintenance_now(pool).await {
            warn!(error = &*err as &dyn Error, "maintenance not requested");
        }
        if let Err(err) = request_repo_syncs_now(pool).await {
            warn!(
                error = &*err as &dyn Error,
                "reads of the repositories not requested"
            );
        }
    })
    .await;
}

async fn request_maintenance_now(pool: &PgPool) -> anyhow::Result<()> {
    let requested =
        in_system_transaction(pool, |tx| Box::pin(request_maintenance(tx, now()))).await?;
    if requested {
        info!("maintenance requested");
    }
    Ok(())
}

async fn request_repo_syncs_now(pool: &PgPool) -> anyhow::Result<()> {
    let requested =
        in_system_transaction(pool, |tx| Box::pin(request_repo_syncs(tx, now()))).await?;
    if requested > 0 {
        info!(requested, "reads of the repositories requested");
    }
    Ok(())
}

async fn request_sweep_regularly(
    pool: &PgPool,
    instance: ScmInstanceId,
    cancel: &CancellationToken,
) {
    repeat_every(SWEEP_EVERY, cancel, || async move {
        if let Err(err) = request_sweep_now(pool, instance).await {
            warn!(error = &*err as &dyn Error, "sweep not requested");
        }
    })
    .await;
}

async fn request_sweep_now(pool: &PgPool, instance: ScmInstanceId) -> anyhow::Result<()> {
    let requested =
        in_system_transaction(pool, |tx| Box::pin(request_sweep(tx, instance, now()))).await?;
    if requested {
        info!("sweep of the deliveries requested");
    }
    Ok(())
}

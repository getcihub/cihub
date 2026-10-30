//! The `scheduler` role: the reconciliation loop of the queues, from
//! `cihub-mergequeue`, and the regular requests of the maintenance, of the
//! sync of the installations, of the reads of the repositories and of the
//! members of each account, and of the sweep of the deliveries.

use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use anyhow::anyhow;
use cihub_platform::{
    SWEEP_EVERY, request_installations_sync, request_maintenance, request_member_syncs,
    request_repo_syncs, request_sweep,
};
use cihub_store::{PgPool, Postgres, Transaction, begin_system};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use super::start::Shared;
use super::{merge_queue, now};

pub(super) async fn run(shared: Shared) -> anyhow::Result<()> {
    let module = merge_queue(&shared);
    let Shared { pool, cancel, .. } = shared;
    let queues = (module.scheduler)(cancel.clone());
    let maintenance = request_hourly(&pool, &cancel);
    let sweep = request_sweep_regularly(&pool, &cancel);
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
        let maintenance =
            in_system_transaction(pool, |tx| Box::pin(request_maintenance(tx, now()))).await;
        match maintenance {
            Ok(true) => info!("maintenance.requested"),
            Ok(false) => {}
            Err(err) => warn!(error = &*err as &dyn Error, "maintenance.not_requested"),
        }

        let installations =
            in_system_transaction(pool, |tx| Box::pin(request_installations_sync(tx, now()))).await;
        match installations {
            Ok(true) => info!("installations_sync.requested"),
            Ok(false) => {}
            Err(err) => warn!(
                error = &*err as &dyn Error,
                "installations_sync.not_requested"
            ),
        }

        let repos = in_system_transaction(pool, |tx| Box::pin(request_repo_syncs(tx, now()))).await;
        match repos {
            Ok(0) => {}
            Ok(requested) => info!(requested, "repositories_read.requested"),
            Err(err) => warn!(
                error = &*err as &dyn Error,
                "repositories_read.not_requested"
            ),
        }

        let members =
            in_system_transaction(pool, |tx| Box::pin(request_member_syncs(tx, now()))).await;
        match members {
            Ok(0) => {}
            Ok(requested) => info!(requested, "members_read.requested"),
            Err(err) => warn!(error = &*err as &dyn Error, "members_read.not_requested"),
        }
    })
    .await;
}

async fn request_sweep_regularly(pool: &PgPool, cancel: &CancellationToken) {
    repeat_every(SWEEP_EVERY, cancel, || async move {
        let sweep = in_system_transaction(pool, |tx| Box::pin(request_sweep(tx, now()))).await;
        match sweep {
            Ok(true) => info!("deliveries_sweep.requested"),
            Ok(false) => {}
            Err(err) => warn!(
                error = &*err as &dyn Error,
                "deliveries_sweep.not_requested"
            ),
        }
    })
    .await;
}

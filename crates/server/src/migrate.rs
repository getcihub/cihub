//! `cihub-server migrate up`: applies the missing migrations, creates the
//! partitions of the events, then stops. `cihub-server migrate down` reverts
//! the migrations newer than a version.

use cihub_store::{MIGRATOR, PgPool};
use tracing::info;

use super::now;

pub(super) async fn up(pool: &PgPool) -> anyhow::Result<()> {
    let applied = cihub_store::migrate(&MIGRATOR, pool).await?;
    for version in &applied {
        info!(version, "migration.applied");
    }

    info!(applied = applied.len(), "migrations.done");
    cihub_store::create_event_partitions(pool, now().date()).await?;
    info!("event_partitions.ready");
    Ok(())
}

pub(super) async fn down(pool: &PgPool, target: i64) -> anyhow::Result<()> {
    let reverted = cihub_store::revert(&MIGRATOR, pool, target).await?;
    for version in &reverted {
        info!(version, "migration.reverted");
    }

    info!(reverted = reverted.len(), target, "migrations.reverted");
    Ok(())
}

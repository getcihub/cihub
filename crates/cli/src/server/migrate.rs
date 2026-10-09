//! `cihub server migrate`: applies the missing migrations, creates the
//! partitions of the events, then stops (`DECISIONS.md` D14).

use cihub_store::{MIGRATOR, PgPool};
use tracing::info;

use super::now;

pub(super) async fn run(pool: &PgPool) -> anyhow::Result<()> {
    let applied = cihub_store::migrate(&MIGRATOR, pool).await?;
    for version in &applied {
        info!(version, "migration applied");
    }

    info!(applied = applied.len(), "migrations done");
    cihub_store::create_event_partitions(pool, now().date()).await?;
    info!("event partitions ready");
    Ok(())
}

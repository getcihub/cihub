//! Postgres store of CIHub: the connection pool and the migrations embedded
//! from `migrations/` (`AGENTS.md` D14). It holds no queue logic and does not
//! decide when the migrations run: `cihub-server migrate` applies them, and
//! `cihub-server` refuses to start while one is missing.

use std::collections::BTreeSet;

use secrecy::{ExposeSecret, SecretString};
pub use sqlx::PgPool;
use sqlx::migrate::{MigrateError, Migrator};

/// The migrations of `migrations/`, embedded in the binary.
pub static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("cannot connect to Postgres")]
    Connect(#[source] sqlx::Error),
    #[error("cannot read the applied migrations")]
    ReadMigrations(#[source] sqlx::Error),
    #[error("cannot apply the migrations")]
    Migrate(#[source] MigrateError),
}

/// Opens a pool on `url` (`postgres://user:password@host/database`).
///
/// # Errors
///
/// Postgres cannot be reached or refuses the connection.
pub async fn connect(url: &SecretString) -> Result<PgPool, StoreError> {
    PgPool::connect(url.expose_secret())
        .await
        .map_err(StoreError::Connect)
}

/// Whether Postgres answers a query on `pool`.
pub async fn ping(pool: &PgPool) -> bool {
    sqlx::query("SELECT 1").execute(pool).await.is_ok()
}

/// The versions of `migrator` that the database has not applied, in order.
///
/// # Errors
///
/// The table of the applied migrations cannot be read.
pub async fn missing_migrations(
    migrator: &Migrator,
    pool: &PgPool,
) -> Result<Vec<i64>, StoreError> {
    let exists: bool = sqlx::query_scalar("SELECT to_regclass('_sqlx_migrations') IS NOT NULL")
        .fetch_one(pool)
        .await
        .map_err(StoreError::ReadMigrations)?;
    let applied: BTreeSet<i64> = if exists {
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations WHERE success")
            .fetch_all(pool)
            .await
            .map_err(StoreError::ReadMigrations)?
            .into_iter()
            .collect()
    } else {
        BTreeSet::new()
    };
    Ok(migrator
        .iter()
        .filter(|m| m.migration_type.is_up_migration() && !applied.contains(&m.version))
        .map(|m| m.version)
        .collect())
}

/// Applies the missing migrations of `migrator` and returns their versions.
/// Two processes that migrate at the same time wait for each other.
///
/// # Errors
///
/// A migration fails, or an applied migration differs from its file.
pub async fn migrate(migrator: &Migrator, pool: &PgPool) -> Result<Vec<i64>, StoreError> {
    let missing = missing_migrations(migrator, pool).await?;
    migrator.run(pool).await.map_err(StoreError::Migrate)?;
    Ok(missing)
}

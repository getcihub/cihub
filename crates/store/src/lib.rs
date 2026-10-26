//! Postgres store of CIHub: the connection pool, the transactions of an
//! organization or of the system (row level security), and
//! the migrations embedded from `migrations/`. It holds no
//! queue logic and does not decide when the migrations run: `cihub-server
//! migrate up` applies them, `cihub-server migrate down` reverts them, and
//! `cihub-server start` refuses to start while one is missing.

mod cipher;
mod device_codes;
mod events;
mod memberships;
mod persons;
mod sessions;
mod tokens;
mod users;

use std::collections::BTreeSet;
use std::time::Duration;

use cihub_domain::OrgId;
pub use cipher::{DatabaseCipher, MIN_DATABASE_SECRET_BYTES};
pub use device_codes::{
    DevicePoll, NewDeviceCode, answer_device_code, create_device_code, new_device_code_secret,
    new_user_code, poll_device_code,
};
pub use events::{
    append_events, create_event_partitions, delete_facts, deposit_fact, event_kind,
    load_event_page, load_events, next_month, pending_facts,
};
pub use memberships::{
    Membership, ORG_IS_SUSPENDED, forget_memberships_before, memberships_of, record_membership,
};
pub use persons::{
    PersonToken, forget_permissions_before, forget_person_token, forget_tokens_without_session,
    keep_person_token, lock_person_token, person_synced_at, readable_repos, record_person_sync,
    record_repo_permission, repo_permission_of,
};
use secrecy::{ExposeSecret, SecretString};
pub use sessions::{
    CLI_SESSION_IDLE_LIFETIME, NewSession, SessionClient, SessionGrant, authenticate_session,
    create_session, csrf_token, delete_session, delete_session_by_id, delete_sessions_of,
    new_cli_session_secret, new_session_secret, new_sign_in_secret,
};
use sqlx::migrate::{MigrateError, Migrator};
use sqlx::postgres::PgPoolOptions;
pub use sqlx::{PgPool, Postgres, Transaction};
pub use tokens::{
    ApiTokenGrant, NewApiToken, authenticate, create_api_token, new_api_token_secret,
};
pub use users::{UserProfile, ensure_user, profile_of, record_avatar, record_profile};

pub static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("cannot connect to Postgres")]
    Connect(#[source] sqlx::Error),
    #[error("cannot open a transaction")]
    Begin(#[source] sqlx::Error),
    #[error("cannot read the Postgres role")]
    ReadRole(#[source] sqlx::Error),
    #[error(
        "the Postgres role `{0}` is a superuser or has BYPASSRLS: it would see every organization"
    )]
    RoleBypassesRowSecurity(String),
    #[error("query failed")]
    Query(#[source] sqlx::Error),
    #[error("cannot serialize an event")]
    WriteEvent(#[source] serde_json::Error),
    #[error("event {0} is a decision: only the scheduler writes a decision")]
    DecisionDeposited(String),
    #[error("{what} {id} does not match cihub-domain")]
    Read {
        what: &'static str,
        id: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("the system has no random source")]
    Random(#[source] rand::rngs::SysError),
    #[error("cannot read the applied migrations")]
    ReadMigrations(#[source] sqlx::Error),
    #[error("cannot apply the migrations")]
    Migrate(#[source] MigrateError),
    #[error("no migration has the version {0}")]
    UnknownMigration(i64),
    #[error("cannot revert the migrations")]
    Revert(#[source] MigrateError),
    #[error("`database.secret` is shorter than {MIN_DATABASE_SECRET_BYTES} bytes")]
    ShortDatabaseSecret,
    #[error("cannot seal a secret with the database secret")]
    Seal,
    #[error("a sealed secret does not open with the database secret")]
    Unseal,
}

impl StoreError {
    fn read<E: Into<Box<dyn std::error::Error + Send + Sync>>>(
        what: &'static str,
        id: &str,
    ) -> impl FnOnce(E) -> Self {
        let id = id.to_owned();
        move |source| Self::Read {
            what,
            id,
            source: source.into(),
        }
    }
}

/// The limits of the connection pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoolSettings {
    pub size: u32,
    /// A connection unused for this long is closed.
    pub idle_timeout: Duration,
    /// The longest wait for a connection of the pool.
    pub connection_timeout: Duration,
}

/// # Errors
///
/// Postgres cannot be reached or refuses the connection.
pub async fn connect(url: &SecretString, pool: PoolSettings) -> Result<PgPool, StoreError> {
    PgPoolOptions::new()
        .max_connections(pool.size)
        .idle_timeout(pool.idle_timeout)
        .acquire_timeout(pool.connection_timeout)
        .connect(url.expose_secret())
        .await
        .map_err(StoreError::Connect)
}

/// Checks that the role of `pool` is subject to row level security: a
/// superuser or a `BYPASSRLS` role sees every organization.
///
/// # Errors
///
/// The role bypasses row level security, or cannot be read.
pub async fn check_role(pool: &PgPool) -> Result<(), StoreError> {
    let (role, bypasses): (String, bool) = sqlx::query_as(
        "SELECT rolname::text, rolsuper OR rolbypassrls FROM pg_roles WHERE rolname = current_user",
    )
    .fetch_one(pool)
    .await
    .map_err(StoreError::ReadRole)?;
    if bypasses {
        return Err(StoreError::RoleBypassesRowSecurity(role));
    }
    Ok(())
}

/// Opens a transaction that sees and writes only the rows of `org`. The API
/// uses it once the request is authenticated.
///
/// # Errors
///
/// Postgres refuses the transaction.
pub async fn begin_org(
    pool: &PgPool,
    org: OrgId,
) -> Result<Transaction<'static, Postgres>, StoreError> {
    begin_with(pool, "cihub.org_id", &org.to_string()).await
}

/// Opens a transaction that sees and writes the rows of every organization:
/// for the work that does not know the organization yet (authentication by
/// token hash) or that spans them (scheduler, partitions). Each query still
/// filters by its queue or its organization.
///
/// # Errors
///
/// Postgres refuses the transaction.
pub async fn begin_system(pool: &PgPool) -> Result<Transaction<'static, Postgres>, StoreError> {
    begin_with(pool, "cihub.system", "on").await
}

/// Sets `name` for the transaction only: it ends with the commit or the
/// rollback, so a pooled connection never keeps it.
async fn begin_with(
    pool: &PgPool,
    name: &str,
    value: &str,
) -> Result<Transaction<'static, Postgres>, StoreError> {
    let mut tx = pool.begin().await.map_err(StoreError::Begin)?;
    sqlx::query("SELECT set_config($1, $2, true)")
        .bind(name)
        .bind(value)
        .execute(&mut *tx)
        .await
        .map_err(StoreError::Begin)?;
    Ok(tx)
}

pub async fn ping(pool: &PgPool) -> bool {
    sqlx::query("SELECT 1").execute(pool).await.is_ok()
}

async fn applied_migrations(pool: &PgPool) -> Result<BTreeSet<i64>, StoreError> {
    let exists: bool = sqlx::query_scalar("SELECT to_regclass('_sqlx_migrations') IS NOT NULL")
        .fetch_one(pool)
        .await
        .map_err(StoreError::ReadMigrations)?;
    if !exists {
        return Ok(BTreeSet::new());
    }

    Ok(
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations WHERE success")
            .fetch_all(pool)
            .await
            .map_err(StoreError::ReadMigrations)?
            .into_iter()
            .collect(),
    )
}

/// # Errors
///
/// The table of the applied migrations cannot be read.
pub async fn missing_migrations(
    migrator: &Migrator,
    pool: &PgPool,
) -> Result<Vec<i64>, StoreError> {
    let applied = applied_migrations(pool).await?;
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

/// Reverts the applied migrations of `migrator` newer than `target`, the
/// newest first, and returns their versions; `0` reverts them all. Their data
/// is lost.
///
/// # Errors
///
/// `target` is neither `0` nor a migration of `migrator`, the database holds a
/// migration that `migrator` does not know, or a revert fails.
pub async fn revert(
    migrator: &Migrator,
    pool: &PgPool,
    target: i64,
) -> Result<Vec<i64>, StoreError> {
    let is_known = target == 0 || migrator.iter().any(|m| m.version == target);
    if !is_known {
        return Err(StoreError::UnknownMigration(target));
    }

    let applied = applied_migrations(pool).await?;
    let reverted = migrator
        .iter()
        .rev()
        .filter(|m| m.migration_type.is_down_migration())
        .filter(|m| m.version > target && applied.contains(&m.version))
        .map(|m| m.version)
        .collect();

    migrator
        .undo(pool, target)
        .await
        .map_err(StoreError::Revert)?;
    Ok(reverted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_migration_has_an_up_and_a_down() {
        let versions = |is_down: bool| -> Vec<i64> {
            MIGRATOR
                .iter()
                .filter(|m| m.migration_type.is_down_migration() == is_down)
                .map(|m| m.version)
                .collect()
        };

        assert!(MIGRATOR.iter().all(|m| m.migration_type.is_reversible()));
        assert_eq!(versions(false), versions(true));
    }
}

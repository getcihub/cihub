//! Tests against Postgres: each `#[sqlx::test]` gets a new database.

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

use std::path::Path;

use cihub_store::{migrate, missing_migrations};
use sqlx::PgPool;
use sqlx::migrate::Migrator;

async fn fixtures(name: &str) -> Migrator {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/migrations")
        .join(name);
    Migrator::new(dir).await.unwrap()
}

#[sqlx::test(migrations = false)]
async fn migrate_twice_changes_nothing_the_second_time(pool: PgPool) {
    let both = fixtures("both").await;
    assert_eq!(missing_migrations(&both, &pool).await.unwrap(), [1, 2]);
    assert_eq!(migrate(&both, &pool).await.unwrap(), [1, 2]);
    assert_eq!(migrate(&both, &pool).await.unwrap(), Vec::<i64>::new());
    assert_eq!(
        missing_migrations(&both, &pool).await.unwrap(),
        Vec::<i64>::new()
    );
}

#[sqlx::test(migrations = false)]
async fn a_newer_binary_sees_its_missing_migration(pool: PgPool) {
    migrate(&fixtures("first").await, &pool).await.unwrap();
    let both = fixtures("both").await;
    assert_eq!(missing_migrations(&both, &pool).await.unwrap(), [2]);
}

#[sqlx::test(migrations = false)]
async fn an_older_binary_ignores_a_newer_migration(pool: PgPool) {
    migrate(&fixtures("both").await, &pool).await.unwrap();
    let first = fixtures("first").await;
    assert_eq!(
        missing_migrations(&first, &pool).await.unwrap(),
        Vec::<i64>::new()
    );
}

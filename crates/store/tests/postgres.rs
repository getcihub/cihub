//! Tests against Postgres: each `#[sqlx::test]` gets a new database.

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

use std::path::Path;

use cihub_domain::{ApiScope, AppKind, MembershipRole, OrgId};
use cihub_store::{begin_org, begin_system, migrate, missing_migrations};
use sqlx::migrate::Migrator;
use sqlx::{AssertSqlSafe, PgConnection, PgPool};
use strum::IntoEnumIterator;
use ulid::Ulid;

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

fn org(n: u128) -> OrgId {
    OrgId::from_ulid(Ulid(n))
}

const TENANT_TABLES: [&str; 6] = [
    "orgs",
    "installations",
    "repos",
    "memberships",
    "api_tokens",
    "audit_log",
];

/// Inserts the rows of `org` in every tenant table; `n` keeps the unique
/// columns apart.
fn rows(org: OrgId, n: u8) -> String {
    format!(
        "INSERT INTO orgs (id, slug, github_org_id) VALUES ('{org}', 'org-{n}', {n});
         INSERT INTO installations (id, org_id, github_installation_id, app_kind, account_type)
             VALUES ('installation-{n}', '{org}', {n}, 'main', 'organization');
         INSERT INTO repos (id, org_id, installation_id, github_repo_id, owner, name, default_branch)
             VALUES ('repo-{n}', '{org}', 'installation-{n}', {n}, 'owner-{n}', 'name', 'main');
         INSERT INTO memberships (org_id, user_id, role) VALUES ('{org}', 'user-1', 'admin');
         INSERT INTO api_tokens (id, org_id, name, token_hash, scopes)
             VALUES ('apitoken-{n}', '{org}', 'ci', sha256('{n}'), '{{targets:write}}');
         INSERT INTO audit_log (id, org_id, action, target)
             VALUES ('audit-{n}', '{org}', 'queue.paused', 'queue-{n}');"
    )
}

async fn seed(pool: &PgPool) {
    let mut tx = begin_system(pool).await.unwrap();
    sqlx::raw_sql("INSERT INTO users (id, github_user_id, login) VALUES ('user-1', 1, 'octocat')")
        .execute(&mut *tx)
        .await
        .unwrap();
    for n in [1, 2] {
        sqlx::raw_sql(AssertSqlSafe(rows(org(n.into()), n)))
            .execute(&mut *tx)
            .await
            .unwrap();
    }
    tx.commit().await.unwrap();
}

async fn count(conn: &mut PgConnection, table: &str) -> i64 {
    sqlx::query_scalar(AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
        .fetch_one(conn)
        .await
        .unwrap()
}

fn org_column(table: &str) -> &'static str {
    if table == "orgs" { "id" } else { "org_id" }
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn an_organization_neither_reads_nor_writes_another(pool: PgPool) {
    seed(&pool).await;
    let (org_a, org_b) = (org(1), org(2));
    for table in TENANT_TABLES {
        let column = org_column(table);
        let mut tx = begin_org(&pool, org_a).await.unwrap();
        assert_eq!(count(&mut tx, table).await, 1, "{table}");
        for statement in [
            format!("UPDATE {table} SET {column} = {column} WHERE {column} = '{org_b}'"),
            format!("DELETE FROM {table} WHERE {column} = '{org_b}'"),
        ] {
            let done = sqlx::query(AssertSqlSafe(statement.clone()))
                .execute(&mut *tx)
                .await
                .unwrap();
            assert_eq!(done.rows_affected(), 0, "{statement}");
        }
        tx.rollback().await.unwrap();
    }

    // Moving a row of A to B, or writing a row of B, breaks the policy.
    let mut tx = begin_org(&pool, org_a).await.unwrap();
    let err = sqlx::query(AssertSqlSafe(format!(
        "UPDATE repos SET org_id = '{org_b}'"
    )))
    .execute(&mut *tx)
    .await
    .unwrap_err();
    assert_eq!(err.as_database_error().unwrap().code().unwrap(), "42501");
    tx.rollback().await.unwrap();
    let mut tx = begin_org(&pool, org_a).await.unwrap();
    let err = sqlx::raw_sql(AssertSqlSafe(rows(org_b, 3)))
        .execute(&mut *tx)
        .await
        .unwrap_err();
    assert_eq!(err.as_database_error().unwrap().code().unwrap(), "42501");
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_transaction_without_context_sees_nothing(pool: PgPool) {
    seed(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    for table in TENANT_TABLES {
        assert_eq!(count(&mut tx, table).await, 0, "{table}");
    }
    assert_eq!(count(&mut tx, "users").await, 1);
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_system_sees_every_organization(pool: PgPool) {
    seed(&pool).await;
    let mut tx = begin_system(&pool).await.unwrap();
    for table in TENANT_TABLES {
        assert_eq!(count(&mut tx, table).await, 2, "{table}");
    }
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_committed_context_does_not_stay_on_the_connection(pool: PgPool) {
    seed(&pool).await;
    let org_a = org(1);
    let mut conn = pool.acquire().await.unwrap();
    let mut tx = sqlx::Connection::begin(&mut *conn).await.unwrap();
    sqlx::query("SELECT set_config('cihub.org_id', $1, true)")
        .bind(org_a.to_string())
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(count(&mut conn, "orgs").await, 0);
}

/// A new table with `org_id` fails here until it has its policy.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn every_tenant_table_forces_row_level_security(pool: PgPool) {
    let tables: Vec<(String, bool, bool)> = sqlx::query_as(
        "SELECT c.relname::text, c.relrowsecurity, c.relforcerowsecurity
         FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE n.nspname = 'public' AND c.relkind IN ('r', 'p')
           AND (c.relname = 'orgs' OR EXISTS (
               SELECT 1 FROM pg_attribute a
               WHERE a.attrelid = c.oid AND a.attname = 'org_id' AND NOT a.attisdropped))
         ORDER BY 1",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let mut names: Vec<_> = tables.iter().map(|(name, ..)| name.as_str()).collect();
    names.sort_unstable();
    let mut expected = TENANT_TABLES.to_vec();
    expected.sort_unstable();
    assert_eq!(names, expected);
    for (name, enabled, forced) in &tables {
        assert!(*enabled && *forced, "{name}");
    }
}

/// A new Postgres enum fails here until it is compared with its domain enum.
#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn postgres_enums_match_the_domain(pool: PgPool) {
    fn values<E: IntoEnumIterator + Into<&'static str>>() -> Vec<String> {
        E::iter().map(|value| value.into().to_owned()).collect()
    }
    let expected: Vec<(&str, Vec<String>)> = vec![
        ("api_scope", values::<ApiScope>()),
        ("app_kind", values::<AppKind>()),
        ("membership_role", values::<MembershipRole>()),
    ];
    let names: Vec<String> = sqlx::query_scalar(
        "SELECT t.typname::text FROM pg_type t JOIN pg_namespace n ON n.oid = t.typnamespace
         WHERE n.nspname = 'public' AND t.typtype = 'e' ORDER BY 1",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        names,
        expected.iter().map(|(name, _)| *name).collect::<Vec<_>>()
    );
    for (name, domain) in expected {
        let labels: Vec<String> = sqlx::query_scalar(
            "SELECT e.enumlabel::text FROM pg_enum e JOIN pg_type t ON t.oid = e.enumtypid
             WHERE t.typname = $1 ORDER BY e.enumsortorder",
        )
        .bind(name)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(labels, domain, "{name}");
    }
}

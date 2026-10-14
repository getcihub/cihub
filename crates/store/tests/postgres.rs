//! Tests against Postgres: each `#[sqlx::test]` gets a new database.

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

use std::path::Path;

use cihub_domain::{
    Actor, ActorKind, ApiScope, CheckConclusion, CheckKind, ConnectionKind, ConnectionMode, Event,
    EventId, EventKind, EventType, GroupState, ItemId, ItemState, JobClass, JobState, OrgId,
    Permission, Priority, QueueId, QueueMode, QueueState, Reason, RequiredStatusesSource,
    RunStatus, SubmittedVia, UserId,
};
use cihub_domain::{ApiTokenId, DeviceCodeId, DeviceCodeState, MembershipRole, SessionId};
use cihub_store::{
    ApiTokenGrant, CLI_SESSION_IDLE_LIFETIME, DevicePoll, Membership, NewApiToken, NewDeviceCode,
    NewSession, SessionClient, SessionGrant, answer_device_code, append_events, authenticate,
    authenticate_session, begin_org, begin_system, create_api_token, create_device_code,
    create_event_partitions, create_session, delete_session, delete_session_by_id,
    forget_memberships_before, load_events, memberships_of, migrate, missing_migrations,
    new_api_token_secret, new_cli_session_secret, new_device_code_secret, new_session_secret,
    new_user_code, poll_device_code, record_membership,
};
use secrecy::{ExposeSecret, SecretString};
use sqlx::migrate::Migrator;
use sqlx::{AssertSqlSafe, PgConnection, PgPool};
use strum::IntoEnumIterator;
use time::{Date, Duration, Month, OffsetDateTime};
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

const TENANT_TABLES: [&str; 24] = [
    "orgs",
    "scm_connections",
    "repos",
    "queues",
    "webhook_inbox",
    "jobs",
    "api_tokens",
    "audit_log",
    "events",
    "queue_items",
    "test_groups",
    "test_runs",
    "queue_inbox",
    "check_observations",
    "pr_surfaces",
    "pr_titles",
    "config_checks",
    "pr_comments",
    "memberships",
    "log_cursors",
    "an_item_conclusions",
    "an_state_durations",
    "an_test_run_results",
    "an_check_results",
];

fn september() -> Date {
    Date::from_calendar_date(2026, Month::September, 15).unwrap()
}

fn rows(org: OrgId, n: u8) -> String {
    let queue = QueueId::from_ulid(Ulid(100 + u128::from(n)));
    format!(
        "INSERT INTO orgs (id, slug) VALUES ('{org}', 'org-{n}');
         INSERT INTO scm_connections (id, org_id, account_external_id, kind, external_id,
                                      synced_at, permissions)
             VALUES ('connection-{n}', '{org}', '{n}', 'github_app', '{n}', now(), '{{}}');
         INSERT INTO repos (id, org_id, connection_id, external_id, namespace, name,
                            default_branch)
             VALUES ('repo-{n}', '{org}', 'connection-{n}', '{n}', 'account-{n}', 'name', 'main');
         INSERT INTO queues (id, org_id, repo_id, target_branch, name, snapshot_seq, snapshot)
             VALUES ('{queue}', '{org}', 'repo-{n}', 'main', 'main', 1, '{{}}');
         INSERT INTO webhook_inbox (delivery_id, org_id, event, payload, received_at)
             VALUES ('delivery-{n}', '{org}', 'push', '{{}}', '2026-09-15T00:00:00Z');
         INSERT INTO jobs (idempotency_key, type, class, org_id, payload, run_after, created_at)
             VALUES ('job-{n}', 'test', 'repair', '{org}', '{{}}', '2026-09-15T00:00:00Z',
                     '2026-09-15T00:00:00Z');
         INSERT INTO api_tokens (id, org_id, name, token_hash, scopes)
             VALUES ('apitoken-{n}', '{org}', 'ci', sha256('{n}'), '{{prs:write}}');
         INSERT INTO audit_log (id, org_id, action, target)
             VALUES ('audit-{n}', '{org}', 'queue.paused', 'queue-{n}');
         INSERT INTO events (id, org_id, queue_id, seq, type, actor_kind, payload, occurred_at)
             VALUES ('event-{n}', '{org}', '{queue}', 1, 'access.lost', 'system', 'null',
                     '2026-09-15T00:00:00Z');
         INSERT INTO queue_items (id, org_id, queue_id, pr_number, pr_head_sha, pr_author_id, state,
                                  state_reason, state_changed_at, priority_value, no_batch,
                                  force_enqueued, submitted_via, submitted_at, block_reasons,
                                  is_current)
             VALUES ('item-{n}', '{org}', '{queue}', 1, 'abc', 'user-1', 'not_ready', 'submitted',
                     '2026-09-15T00:00:00Z', 100, false, false, 'comment',
                     '2026-09-15T00:00:00Z', '[]', true);
         INSERT INTO test_groups (id, org_id, queue_id, item_ids, predecessor_ids, state)
             VALUES ('group-{n}', '{org}', '{queue}', '{{item-{n}}}', '{{}}', 'testing');
         INSERT INTO test_runs (id, org_id, queue_id, group_id, attempt, test_branch, status,
                                status_reason, required_statuses, started_at,
                                failure_causing_statuses, base_sha, head_shas,
                                tested_pr_numbers)
             VALUES ('run-{n}', '{org}', '{queue}', 'group-{n}', 1, 'cihub-merge/pr-1',
                     'assembling', 'test_run_started', '{{}}', '2026-09-15T00:00:00Z', '{{}}',
                     'abc', '{{abc}}', '{{1}}');
         INSERT INTO check_observations (org_id, run_id, name, kind, required, observed_at)
             VALUES ('{org}', 'run-{n}', 'ci', 'check', true, '2026-09-15T00:00:00Z');
         INSERT INTO queue_inbox (id, org_id, queue_id, actor_kind, type, payload, received_at)
             VALUES ('fact-{n}', '{org}', '{queue}', 'system', 'access.lost', 'null',
                     '2026-09-15T00:00:00Z');
         INSERT INTO pr_surfaces (org_id, queue_id, pr_number, desired, desired_hash)
             VALUES ('{org}', '{queue}', 1, '{{}}', 'h');
         INSERT INTO pr_comments (org_id, repo_id, pr_number, comment_external_id)
             VALUES ('{org}', 'repo-{n}', 1, 'comment-{n}');
         INSERT INTO pr_titles (org_id, repo_id, pr_number, title, read_at)
             VALUES ('{org}', 'repo-{n}', 1, 'Fix the build', '2026-09-15T00:00:00Z');
         INSERT INTO config_checks (org_id, queue_id, findings, checked_at)
             VALUES ('{org}', '{queue}', '[]', '2026-09-15T00:00:00Z');
         INSERT INTO memberships (org_id, user_id, role, refreshed_at)
             VALUES ('{org}', 'user-1', 'member', '2026-09-15T00:00:00Z');
         INSERT INTO log_cursors (org_id, queue_id, subscription, seq)
             VALUES ('{org}', '{queue}', 'analytics', 1);
         INSERT INTO an_item_conclusions (item_id, org_id, queue_id, conclusion, reason,
                                          submitted_at, concluded_at)
             VALUES ('item-{n}', '{org}', '{queue}', 'merged', 'merge_graph_merge',
                     '2026-09-15T00:00:00Z', '2026-09-15T00:10:00Z');
         INSERT INTO an_state_durations (org_id, queue_id, item_id, pass, state, entered_at,
                                         left_at, exit_reason)
             VALUES ('{org}', '{queue}', 'item-{n}', 0, 'pending', '2026-09-15T00:00:00Z',
                     '2026-09-15T00:10:00Z', 'merge_graph_merge');
         INSERT INTO an_test_run_results (run_id, org_id, queue_id, group_id, attempt, status,
                                          reason, started_at, ended_at)
             VALUES ('run-{n}', '{org}', '{queue}', 'group-{n}', 1, 'succeeded',
                     'test_run_passed', '2026-09-15T00:00:00Z', '2026-09-15T00:05:00Z');
         INSERT INTO an_check_results (org_id, run_id, name, kind, required, conclusion)
             VALUES ('{org}', 'run-{n}', 'ci', 'check', true, 'success');"
    )
}

async fn seed_global(conn: &mut PgConnection) {
    sqlx::raw_sql("INSERT INTO users (id, external_id, login) VALUES ('user-1', '1', 'octocat')")
        .execute(conn)
        .await
        .unwrap();
}

async fn seed(pool: &PgPool) {
    create_event_partitions(pool, september()).await.unwrap();
    let mut tx = begin_system(pool).await.unwrap();
    seed_global(&mut tx).await;
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

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn an_external_id_is_unique(pool: PgPool) {
    let mut tx = begin_system(&pool).await.unwrap();
    seed_global(&mut tx).await;

    let err =
        sqlx::query("INSERT INTO users (id, external_id, login) VALUES ('user-2', '1', 'other')")
            .execute(&mut *tx)
            .await
            .unwrap_err();
    assert_eq!(err.as_database_error().unwrap().code().unwrap(), "23505");
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn every_tenant_table_forces_row_level_security(pool: PgPool) {
    create_event_partitions(&pool, september()).await.unwrap();
    let tables: Vec<(String, bool, bool, bool)> = sqlx::query_as(
        "SELECT c.relname::text, c.relrowsecurity, c.relforcerowsecurity, c.relispartition
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
    let mut names: Vec<_> = tables
        .iter()
        .filter(|(.., partition)| !partition)
        .map(|(name, ..)| name.as_str())
        .collect();

    names.sort_unstable();
    let mut expected = TENANT_TABLES.to_vec();
    expected.sort_unstable();

    assert_eq!(names, expected);
    assert!(tables.iter().any(|(name, ..)| name == "events_2026_09"));
    for (name, enabled, forced, _) in &tables {
        assert!(*enabled && *forced, "{name}");
    }
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn postgres_enums_match_the_domain(pool: PgPool) {
    fn values<E: IntoEnumIterator + Into<&'static str>>() -> Vec<String> {
        E::iter().map(|value| value.into().to_owned()).collect()
    }
    let expected: Vec<(&str, Vec<String>)> = vec![
        ("actor_kind", values::<ActorKind>()),
        ("api_scope", values::<ApiScope>()),
        ("check_conclusion", values::<CheckConclusion>()),
        ("check_kind", values::<CheckKind>()),
        ("connection_kind", values::<ConnectionKind>()),
        ("connection_mode", values::<ConnectionMode>()),
        ("device_code_state", values::<DeviceCodeState>()),
        ("event_type", values::<EventType>()),
        ("group_state", values::<GroupState>()),
        ("item_state", values::<ItemState>()),
        ("job_class", values::<JobClass>()),
        ("job_state", values::<JobState>()),
        ("membership_role", values::<MembershipRole>()),
        ("permission", values::<Permission>()),
        ("queue_mode", values::<QueueMode>()),
        ("queue_state", values::<QueueState>()),
        ("reason", values::<Reason>()),
        (
            "required_statuses_source",
            values::<RequiredStatusesSource>(),
        ),
        ("run_status", values::<RunStatus>()),
        ("submitted_via", values::<SubmittedVia>()),
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

fn event(n: u128, actor: Actor, cause_id: Option<EventId>, kind: EventKind) -> Event {
    Event {
        id: EventId::from_ulid(Ulid(n)),
        org_id: org(1),
        queue_id: QueueId::from_ulid(Ulid(1)),
        actor,
        occurred_at: OffsetDateTime::from_unix_timestamp(1_789_000_000 + i64::try_from(n).unwrap())
            .unwrap(),
        cause_id,
        kind,
    }
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn the_log_reads_back_in_order(pool: PgPool) {
    seed(&pool).await;
    let mut tx = begin_system(&pool).await.unwrap();
    let user_id = insert_user(&mut tx).await;

    sqlx::query(
        "INSERT INTO queues (id, org_id, repo_id, target_branch, name)
         VALUES ($1, $2, 'repo-1', 'next', 'next')",
    )
    .bind(QueueId::from_ulid(Ulid(1)).to_string())
    .bind(org(1).to_string())
    .execute(&mut *tx)
    .await
    .unwrap();

    tx.commit().await.unwrap();

    let submitted = event(
        10,
        Actor::User { user_id },
        None,
        EventKind::ItemSubmitted {
            pr_number: 12,
            head_sha: "a1".into(),
            priority: Priority::MEDIUM,
            no_batch: false,
            direct: false,
            submitted_via: SubmittedVia::Comment,
            author_id: user_id,
            additions: 3,
            deletions: 1,
            changed_files: 2,
            sender_permission: None,
        },
    );
    let created = event(
        11,
        Actor::System,
        Some(submitted.id),
        EventKind::ItemTransitioned {
            item_id: ItemId::from_ulid(Ulid(1)),
            from: None,
            to: ItemState::NotReady,
            reason: Reason::Submitted,
            position: None,
            verified_by_run_id: None,
            merged_sha: None,
        },
    );
    let lost = event(12, Actor::Bot, None, EventKind::AccessLost);
    let log = vec![submitted, created, lost];

    let mut tx = begin_org(&pool, org(1)).await.unwrap();
    assert_eq!(append_events(&mut tx, &log[..2]).await.unwrap(), [1, 2]);
    assert_eq!(append_events(&mut tx, &log[2..]).await.unwrap(), [3]);
    tx.commit().await.unwrap();

    let mut tx = begin_org(&pool, org(1)).await.unwrap();
    let queue = QueueId::from_ulid(Ulid(1));
    let read = load_events(&mut tx, queue, 0).await.unwrap();

    assert_eq!(
        read,
        [1, 2, 3].into_iter().zip(log.clone()).collect::<Vec<_>>()
    );
    let tail = load_events(&mut tx, queue, 2).await.unwrap();
    assert_eq!(tail, [(3, log[2].clone())]);
    let rows: Vec<(i64, Option<String>)> =
        sqlx::query_as("SELECT seq, item_id FROM events WHERE queue_id = $1 ORDER BY seq")
            .bind(QueueId::from_ulid(Ulid(1)).to_string())
            .fetch_all(&mut *tx)
            .await
            .unwrap();
    let item = Some(ItemId::from_ulid(Ulid(1)).to_string());
    assert_eq!(rows, [(1, None), (2, item), (3, None)]);

    let mut tx = begin_org(&pool, org(2)).await.unwrap();
    assert_eq!(
        load_events(&mut tx, QueueId::from_ulid(Ulid(1)), 0)
            .await
            .unwrap(),
        []
    );
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_partition_isolates_organizations_too(pool: PgPool) {
    seed(&pool).await;
    let mut tx = begin_org(&pool, org(1)).await.unwrap();
    assert_eq!(count(&mut tx, "events_2026_09").await, 1);
    tx.rollback().await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    assert_eq!(count(&mut tx, "events_2026_09").await, 0);
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn partitions_cover_this_month_and_the_next(pool: PgPool) {
    let december = Date::from_calendar_date(2026, Month::December, 31).unwrap();
    create_event_partitions(&pool, december).await.unwrap();
    create_event_partitions(&pool, december).await.unwrap();
    let partitions: Vec<String> = sqlx::query_scalar(
        "SELECT c.relname::text FROM pg_inherits i JOIN pg_class c ON c.oid = i.inhrelid
         WHERE i.inhparent = 'events'::regclass ORDER BY 1",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(partitions, ["events_2026_12", "events_2027_01"]);

    let mut tx = begin_system(&pool).await.unwrap();
    sqlx::query("INSERT INTO orgs (id, slug) VALUES ($1, 'org')")
        .bind(org(1).to_string())
        .execute(&mut *tx)
        .await
        .unwrap();
    let err = append_events(
        &mut tx,
        &[event(1, Actor::System, None, EventKind::AccessLost)],
    )
    .await
    .unwrap_err();

    assert!(format!("{err:?}").contains("no partition"), "{err:?}");
}

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(1_789_000_000 + seconds).unwrap()
}

async fn last_used_at(pool: &PgPool) -> Option<OffsetDateTime> {
    let mut tx = begin_system(pool).await.unwrap();
    sqlx::query_scalar("SELECT last_used_at FROM api_tokens")
        .fetch_one(&mut *tx)
        .await
        .unwrap()
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_token_authenticates_until_it_expires(pool: PgPool) {
    let mut tx = begin_system(&pool).await.unwrap();
    sqlx::query("INSERT INTO orgs (id, slug) VALUES ($1, 'org')")
        .bind(org(1).to_string())
        .execute(&mut *tx)
        .await
        .unwrap();

    tx.commit().await.unwrap();

    let secret = new_api_token_secret().unwrap();
    assert!(secret.expose_secret().starts_with("cht_"));
    let scopes = [ApiScope::QueuesRead, ApiScope::PrsWrite];
    let token = NewApiToken {
        id: ApiTokenId::from_ulid(Ulid(1)),
        org_id: org(1),
        name: "ci",
        scopes: &scopes,
        expires_at: Some(at(3600)),
    };
    let mut tx = begin_org(&pool, org(1)).await.unwrap();
    create_api_token(&mut tx, &token, &secret).await.unwrap();
    tx.commit().await.unwrap();

    let mut tx = begin_system(&pool).await.unwrap();
    let hash: Vec<u8> = sqlx::query_scalar("SELECT token_hash FROM api_tokens")
        .fetch_one(&mut *tx)
        .await
        .unwrap();

    assert_eq!(hash.len(), 32);
    tx.rollback().await.unwrap();

    let grant = ApiTokenGrant {
        id: token.id,
        org_id: org(1),
        scopes: scopes.to_vec(),
    };

    assert_eq!(
        authenticate(&pool, &secret, at(0)).await.unwrap(),
        Some(grant)
    );
    assert_eq!(last_used_at(&pool).await, Some(at(0)));
    authenticate(&pool, &secret, at(30)).await.unwrap().unwrap();

    assert_eq!(last_used_at(&pool).await, Some(at(0)));
    authenticate(&pool, &secret, at(90)).await.unwrap().unwrap();

    assert_eq!(last_used_at(&pool).await, Some(at(90)));

    assert_eq!(authenticate(&pool, &secret, at(3600)).await.unwrap(), None);
    let other = new_api_token_secret().unwrap();
    assert_ne!(other.expose_secret(), secret.expose_secret());
    assert_eq!(authenticate(&pool, &other, at(0)).await.unwrap(), None);
    let malformed = SecretString::from("Bearer cht_");
    assert_eq!(authenticate(&pool, &malformed, at(0)).await.unwrap(), None);
}

async fn insert_user(conn: &mut PgConnection) -> UserId {
    let user_id = UserId::from_ulid(Ulid(7));
    sqlx::query("INSERT INTO users (id, external_id, login) VALUES ($1, '7', 'hubot')")
        .bind(user_id.to_string())
        .execute(conn)
        .await
        .unwrap();
    user_id
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_session_authenticates_until_it_expires_or_signs_out(pool: PgPool) {
    let mut tx = begin_system(&pool).await.unwrap();
    let user_id = insert_user(&mut tx).await;
    let secret = new_session_secret().unwrap();
    let other = new_session_secret().unwrap();
    let session = NewSession {
        id: SessionId::from_ulid(Ulid(1)),
        user_id,
        created_at: at(0),
        expires_at: at(3600),
    };
    let grant = SessionGrant {
        id: session.id,
        user_id,
        expires_at: at(3600),
        client: SessionClient::Dashboard,
    };

    create_session(&mut tx, &session, &secret).await.unwrap();
    tx.commit().await.unwrap();

    assert!(secret.expose_secret().starts_with("chs_"));
    assert_eq!(
        authenticate_session(&pool, &secret, at(0)).await.unwrap(),
        Some(grant)
    );
    assert_eq!(
        authenticate_session(&pool, &secret, at(3600))
            .await
            .unwrap(),
        None
    );
    assert_eq!(authenticate(&pool, &secret, at(0)).await.unwrap(), None);
    assert_eq!(
        authenticate_session(&pool, &other, at(0)).await.unwrap(),
        None
    );

    delete_session(&pool, &secret).await.unwrap();

    assert_eq!(
        authenticate_session(&pool, &secret, at(0)).await.unwrap(),
        None
    );
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_session_of_the_command_line_ends_after_it_is_not_used(pool: PgPool) {
    let mut tx = begin_system(&pool).await.unwrap();
    let user_id = insert_user(&mut tx).await;
    let secret = new_cli_session_secret().unwrap();
    let session = NewSession {
        id: SessionId::from_ulid(Ulid(1)),
        user_id,
        created_at: at(0),
        expires_at: at(0) + CLI_SESSION_IDLE_LIFETIME,
    };
    let expires_at = |grant: Option<SessionGrant>| grant.unwrap().expires_at;

    create_session(&mut tx, &session, &secret).await.unwrap();
    tx.commit().await.unwrap();

    assert!(secret.expose_secret().starts_with("chc_"));
    let grant = authenticate_session(&pool, &secret, at(60)).await.unwrap();

    assert_eq!(
        grant.as_ref().map(|grant| grant.client),
        Some(SessionClient::CommandLine)
    );
    assert_eq!(expires_at(grant), at(0) + CLI_SESSION_IDLE_LIFETIME);
    let later = at(2 * 3600);
    let grant = authenticate_session(&pool, &secret, later).await.unwrap();

    assert_eq!(expires_at(grant), later + CLI_SESSION_IDLE_LIFETIME);
    assert_eq!(authenticate(&pool, &secret, later).await.unwrap(), None);
    let past_the_end = later + CLI_SESSION_IDLE_LIFETIME;

    assert_eq!(
        authenticate_session(&pool, &secret, past_the_end)
            .await
            .unwrap(),
        None
    );

    delete_session_by_id(&pool, session.id).await.unwrap();

    assert_eq!(
        authenticate_session(&pool, &secret, later).await.unwrap(),
        None
    );
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_device_code_gives_one_session_once_its_person_approves_it(pool: PgPool) {
    let mut tx = begin_system(&pool).await.unwrap();
    let user_id = insert_user(&mut tx).await;
    tx.commit().await.unwrap();
    let interval = Duration::seconds(5);
    let device_code = new_device_code_secret().unwrap();
    let user_code = new_user_code().unwrap();
    let code = NewDeviceCode {
        id: DeviceCodeId::from_ulid(Ulid(1)),
        device_code: &device_code,
        user_code: &user_code,
        created_at: at(0),
        expires_at: at(900),
    };
    let poll = |now: OffsetDateTime| {
        let (pool, device_code) = (pool.clone(), device_code.clone());
        async move {
            let mut tx = pool.begin().await.unwrap();
            let poll = poll_device_code(&mut tx, &device_code, interval, now)
                .await
                .unwrap();
            tx.commit().await.unwrap();
            poll
        }
    };

    assert!(create_device_code(&pool, &code).await.unwrap());
    let twin = NewDeviceCode {
        id: DeviceCodeId::from_ulid(Ulid(2)),
        ..code
    };
    assert!(!create_device_code(&pool, &twin).await.unwrap());
    assert_eq!(poll(at(5)).await, DevicePoll::Pending);
    assert_eq!(poll(at(6)).await, DevicePoll::TooFast);

    let approve = async |code: &str, now| {
        answer_device_code(&pool, code, user_id, DeviceCodeState::Approved, now)
            .await
            .unwrap()
    };
    assert!(!approve("BCDF-GHJK", at(20)).await);
    assert!(!approve(&user_code, at(900)).await);
    assert!(approve(&user_code, at(20)).await);
    assert!(!approve(&user_code, at(21)).await);

    assert_eq!(poll(at(30)).await, DevicePoll::Approved(user_id));
    assert_eq!(poll(at(40)).await, DevicePoll::Expired);
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_denied_or_expired_device_code_gives_no_session(pool: PgPool) {
    let mut tx = begin_system(&pool).await.unwrap();
    let user_id = insert_user(&mut tx).await;
    tx.commit().await.unwrap();
    let interval = Duration::seconds(5);
    let codes: Vec<_> = (1..=2)
        .map(|_| (new_device_code_secret().unwrap(), new_user_code().unwrap()))
        .collect();

    for (n, (device_code, user_code)) in (1..).zip(&codes) {
        let code = NewDeviceCode {
            id: DeviceCodeId::from_ulid(Ulid(n)),
            device_code,
            user_code,
            created_at: at(0),
            expires_at: at(900),
        };
        assert!(create_device_code(&pool, &code).await.unwrap());
    }
    let mut tx = pool.begin().await.unwrap();
    let (denied, expired) = (&codes[0], &codes[1]);

    assert!(
        answer_device_code(&pool, &denied.1, user_id, DeviceCodeState::Denied, at(10))
            .await
            .unwrap()
    );
    assert_eq!(
        poll_device_code(&mut tx, &denied.0, interval, at(20))
            .await
            .unwrap(),
        DevicePoll::Denied
    );
    assert_eq!(
        poll_device_code(&mut tx, &expired.0, interval, at(900))
            .await
            .unwrap(),
        DevicePoll::Expired
    );
    let malformed = SecretString::from("chd_");
    assert_eq!(
        poll_device_code(&mut tx, &malformed, interval, at(20))
            .await
            .unwrap(),
        DevicePoll::Expired
    );
}

#[sqlx::test(migrator = "cihub_store::MIGRATOR")]
async fn a_refresh_keeps_the_memberships_it_sees(pool: PgPool) {
    seed(&pool).await;
    let mut tx = begin_system(&pool).await.unwrap();
    let user_id = insert_user(&mut tx).await;
    let now = at(30 * 86_400);
    let expected =
        [(1, MembershipRole::Admin), (2, MembershipRole::Member)].map(|(n, role)| Membership {
            org_id: org(n),
            org_slug: format!("org-{n}"),
            role,
        });

    tx.commit().await.unwrap();
    for n in [1, 2] {
        let mut tx = begin_org(&pool, org(n)).await.unwrap();
        record_membership(&mut tx, org(n), user_id, MembershipRole::Member, now)
            .await
            .unwrap();
        tx.commit().await.unwrap();
    }

    let mut tx = begin_org(&pool, org(1)).await.unwrap();
    record_membership(
        &mut tx,
        org(1),
        user_id,
        MembershipRole::Admin,
        now + Duration::minutes(1),
    )
    .await
    .unwrap();
    let forgotten = forget_memberships_before(&mut tx, org(1), now + Duration::minutes(1))
        .await
        .unwrap();
    tx.commit().await.unwrap();

    assert_eq!(forgotten, 1, "the refresh of org 1 did not see user-1");
    assert_eq!(memberships_of(&pool, user_id).await.unwrap(), expected);
}

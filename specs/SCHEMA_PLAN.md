# Plan: a smaller Postgres schema

Context: a proposal for the project lead, written after the review of October 2026 and the reading of the
Codecov schema (`umbrella`). Nothing here is decided: the project lead validates each step before the code
changes. The code stays the source of truth; this file is deleted once its steps are done or refused.

**Goal.** Fewer tables, fewer columns, fewer enum values, with the same features. Every object in the schema
must be read by code that exists today. What a future lot needs is created by that lot, in its final shape.

**What does not change.** Identifiers are `text` in the typedid form. Every tenant table carries `org_id`
and goes through `cihub_tenant_table`. The log is the source of truth and the other tables are projections
(principle I). Postgres is the only dependency. Before the first published version, a migration is
rewritten, never stacked (`migrations/AGENTS.md`).

## Where we are

23 tables, 23 enums, 17 multi-column foreign keys, 8 migration files. The review found:

| Object                                            | Readers in the code                   |
| ------------------------------------------------- | ------------------------------------- |
| `org_modules`                                     | 4 `JOIN`/`EXISTS`, always true        |
| `memberships`, enum `membership_role`             | none (a test seed)                    |
| `scm_accounts`                                    | none (one `INSERT` at bootstrap)      |
| `queue_leases`                                    | 1:1 with `queues`                     |
| `queue_snapshots`                                 | only the last row per queue           |
| `webhook_inbox.action`                            | none                                  |
| `audit_log.before`                                | read by the API, never written        |
| `api_tokens.created_by`                           | always `NULL`, no route creates a token |
| `scm_connections.claimed_at`                      | one test; equals `created_at`         |
| `jobs.scm_instance_id`                            | only the composite foreign key        |
| `connection_kind.github_sudo_app`                 | none                                  |
| `api_scope.metrics:read`, `targets:write`         | no route requires them                |

## Step 1: a module is not an opt-in

**Idea.** A module adds features to the architecture; every organization has every module. "Module absent"
means "not linked in `cihub-server`", never a row in a table. This is the opposite of Codecov, which gates each
feature three times (a boolean per offer in `Tier`, a boolean per repository, a feature flag) and whose
`Repository` table gained one `*_enabled` column per feature over the years.

**Change.**

- Drop `org_modules` and the enum `module_name`; remove `ModuleName` from `cihub-domain` and the field
  `Module.name` from `cihub-platform`.
- Remove the four `JOIN org_modules` / `EXISTS (... org_modules ...)` in `mergequeue/src/target.rs` and
  `mergequeue/src/scheduler.rs`, the `INSERT` of the bootstrap in `platform/src/connections.rs`, and the seeds
  in the tests of `store`, `platform` and `mergequeue`.
- Principle IX in `AGENTS.md` is reworded: "Modules do not know each other: they talk through the traits of
  `cihub-contracts` and through events. A module that is not compiled in leaves the others unchanged."

**Risk.** Low. About 70 lines and one table.
**Status.** Done on `refactor/simplify-merge-queue` (`7a98aac`). The rewording of principle IX and the
validation of the step wait for the project lead.

## Step 2: tables that nothing reads

### `memberships`

Drop the table, the enum `membership_role` and `MembershipRole`. The roles come from the SCM membership and
serve the dashboard of lot 10 (`specs/SECURITY.md`): lot 10 creates the table in its final shape, with the
refresh job that fills it. Until then `api_tokens` is the only authentication.

**Risk.** None.
**Status.** Done (`6656c6b`).

### `scm_accounts` into `scm_connections`

A connection is the means by which CIHub reaches the repositories of one SCM account. The account has two
facts: its external identifier and its path, and the path is already `orgs.slug`. Add `account_external_id` to
`scm_connections`, drop `scm_accounts`, `ScmAccountId`, the composite foreign key
`(org_id, scm_instance_id, scm_account_id)` and the index `scm_accounts_org_id`.

**Risk.** Low: one `INSERT` moves, four test seeds shrink.
**Status.** Done (`d1182ad`), with `UNIQUE (scm_instance_id, account_external_id)` on `scm_connections`, which
keeps the uniqueness of the account.

### `queue_leases` into `queues`

The table is 1:1 with `queues` by primary key. Two columns `lease_owner` and `lease_expires_at` on `queues`
replace it, and `acquire_leases` becomes one statement:

```sql
UPDATE queues SET lease_owner = $1, lease_expires_at = $3
WHERE id IN (
    SELECT id FROM queues
    WHERE (dirty OR next_tick_at <= $2)
      AND (lease_owner IS NULL OR lease_owner = $1 OR lease_expires_at <= $2)
    ORDER BY dirty DESC, next_tick_at NULLS LAST, id
    LIMIT $4
    FOR UPDATE SKIP LOCKED)
RETURNING id
```

`release_leases` sets the two columns to `NULL` where `lease_owner = $1`. The row lock replaces the
`INSERT … ON CONFLICT DO UPDATE … WHERE` trick and its comment. The advisory lock of each reconciliation
(`lock_queue`) does not change.

**Risk.** Medium: the test with two owners in
`platform/tests/postgres.rs` must stay green; the queue row is updated more often (lease renewal every pass),
which is the same write volume as today on another table.
**Status.** Done (`b743741`); the result is ordered by a CTE, because `RETURNING` has no order.

### `queue_snapshots` into `queues`

`QueueLog::load` reads only the last snapshot (`ORDER BY seq DESC LIMIT 1`); `maintenance.rs`, `scheduler.rs`
and `events.rs` read `max(seq)` per queue. Two columns `snapshot_seq` and `snapshot jsonb` on `queues` give the
same answers with one row per queue, and `rebuild` sets them to `NULL` instead of deleting rows. The snapshot
is still taken every 500 events (`SNAPSHOT_EVERY`).

**Risk.** Medium: the `jsonb` goes to TOAST; an `UPDATE` of `dirty` on the same row
copies the TOAST pointer, not the value, so the hot path does not grow. Measure with the fault-injection test
before and after (`mise run rust:sim`).
**Status.** Done (`008b299`); the fault-injection tests pass on 25 seeds.

## Step 3: columns and values that nothing uses

| Object                            | Change                                                                 |
| --------------------------------- | ---------------------------------------------------------------------- |
| `webhook_inbox.action`            | Drop; the action is in `payload` and the router reads `(event, payload)` |
| `audit_log.before`                | Drop the column and `AuditEntry.before` (API); nothing writes it. Add both back with the first write that has a "before" |
| `api_tokens.created_by`           | Drop the column and `NewApiToken.created_by`; the route that creates a token adds it back |
| `scm_connections.claimed_at`      | Drop; `created_at` carries the same instant                            |
| `connection_kind.github_sudo_app` | Drop the value and `ConnectionKind::GithubSudoApp`                     |
| `api_scope.metrics:read`          | Drop the value and `ApiScope::MetricsRead`; lot 11 adds it with `GET /metrics` |
| `api_scope.targets:write`         | Drop the value and `ApiScope::TargetsWrite`; lot 13 adds it with `POST …/impacted-targets` |

**Risk.** None; the enum test (`postgres_enums_match_the_domain`) and the API tests catch a forgotten side.

**Status.** Done (`6656c6b`, `ad0e0ab`), except `audit_log.before`: dropping `AuditEntry.before` changes the
API response, which this change keeps as it is.

## Step 4: simpler keys

- `scm_connections` gains `UNIQUE (org_id, id)`, like `repos` and `queues`. `jobs` and `webhook_inbox` then
  reference `(org_id, connection_id)` and **`jobs.scm_instance_id` is dropped** with its `CHECK`; `Job.connection`
  becomes `Option<ConnectionId>` (nine construction sites in `mergequeue`, one decode in `platform/src/jobs.rs`).
  `repos.scm_instance_id` and `webhook_inbox.scm_instance_id` are gone: a deployment serves a single SCM.
- `created_at … DEFAULT now()` on `orgs`, `scm_connections`, `repos`, `api_tokens`, `audit_log` and `queues`
  is a second clock next to the one the code passes as a parameter. Remove the defaults and bind the
  time, as the other tables do.

**Risk.** Low.

**Status.** `jobs.scm_instance_id` is gone, since a deployment serves a single SCM: the composite foreign key of `jobs` becomes (`org_id`, `connection_id`).

## Step 5: rules for the tables of the next lots

Learned from the Codecov schema, so that lots 10 to 14 do not undo this plan:

- **No enable flag**, per organization, per repository or per offer. A module that needs a setting reads
  `.cihub.yaml` (`specs/CONFIGURATION.md`).
- **Typed hourly tables for the time series** (lot 11a): `an_hourly_conclusions`, `an_hourly_durations` and
  `an_hourly_checks`, keyed by queue and hour, behind `AnalyticsStore`. A single `measurements (name, bucket,
  value)` table would write the reason, the check and the bucket of a histogram into `name`, outside the closed
  vocabulary (decided October 15, 2026, by the project lead). Codecov has eight aggregate tables (hourly and
  daily, crossed with repository, branch and test); a daily grain comes only if a range needs it.
- **No `outbox` table** (lot 11): `events` is the outbox. A consumer keeps a cursor per queue (`seq` is per
  queue), and reads the events after it. One table less and no double write.
- **Flaky tests** (lot 14): `ft_tests` keyed by `test_id bytea`, a 128-bit hash of `(suite, class, name)`, as
  Codecov does; `ft_results` partitioned by month like `events`, with `outcome = 'flaky_fail'` written at
  ingestion so that the `FlakeOracle` reads one column; `ft_flakes` as intervals (`started_at`, `ended_at NULL`
  while flaky, `pass_streak`); one `ft_rollups`. Raw JUnit and OTLP payloads go to `ft_uploads bytea` with a
  short retention, not to an object store: Codecov needs MinIO, Redis, Postgres and Timescale for this path;
  CIHub needs Postgres (GOAL, point 9).
- **One CIHub comment per PR**. Codecov's `Pull` has `commentid`, `bundle_analysis_commentid` and
  `sentry_commentid`: one column per module. A module writes a section of the comment of `pr_comments`, or
  `pr_comments` gains a `module` column in its key; never a column per module.
- **A setup is a cache of reads of the SCM** (lot 16): `queue_setups` (per queue) and `repo_setups` (per
  repository, its default branch and its file, with the enum `repo_config_state`) hold one row per subject,
  outside the log, as `pr_titles`; no state of a queue depends on them. A repository that the SCM no longer lists
  keeps its row with `repos.removed_at`: its queues keep their log.

## What stays as it is, and why

- `pr_surfaces` (per queue and PR) and `pr_comments` (per PR, shared by the queues that it enters): different keys,
  different lifetimes. `pr_conditions` (per queue and PR) stays apart from `pr_surfaces`: each has one writer, the
  reconciliation for one, `observe_pr` for the other, and a PR not submitted has only the second.
- `webhook_inbox` (raw deliveries, per connection) and `queue_inbox` (translated facts, per queue): two stages,
  and only the scheduler writes the log.
- `test_groups` and `test_runs`: a group has several attempts.
- `check_observations`: the API reads it for the run detail.
- `events` and its monthly partitions, `jobs`, `queue_items`: the log and its projections.
- Operator columns `jobs.last_error`, `jobs.locked_by`, `scm_connections.mode_reason`,
  `scm_connections.suspended_at`: read by a person, written by the code. `webhook_inbox.error` is gone:
  a translation error is a log line.

## Order, verification, documents

Order: step 1, then 2, 3 and 4 in one change each, step 5 as the lots arrive. Each change rewrites the
migration files that create the objects (no new file for a drop), then `mise run db:reset`.

Verification for each change:

1. Build one database from the migrations of `main` and one from the new files, and compare
   `pg_dump --schema-only`: only the intended differences.
2. `mise run check`, `mise run rust:integration` (includes `every_tenant_table_forces_row_level_security` and
   `postgres_enums_match_the_domain`), `CIHUB_SIM_SEEDS=25 mise run rust:sim`.
3. For step 2 (`queue_leases`): the two-owner lease test; for `queue_snapshots`: the replay test
   `a_replay_of_the_log_rebuilds_the_same_projections` and the partition pruning tests.

Documents updated in the same change: `specs/DATA_MODEL.md` (rows), the "Tracking" block of the current lot in
`specs/LOTS.md`, `specs/SECURITY.md` (roles arrive with lot 10), `specs/API.md` (`metrics:read`,
`AuditEntry.before`), `AGENTS.md` (principle IX).

## Result

| Measure                | Before | After steps 1 to 4 |
| ---------------------- | ------ | ------------------ |
| Tables                 | 23     | 18                 |
| Enums                  | 23     | 21                 |
| Multi-column foreign keys | 17  | 14                 |
| Three-column foreign keys | 4   | 0                  |
| Write-only columns     | 6      | 0                  |
| Rust lines (estimate)  |        | about 350 fewer    |

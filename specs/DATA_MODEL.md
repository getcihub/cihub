# Postgres data model

This spec gives the rules of the Postgres schema that the migrations do not show: why a table exists, why it is
split, what a replay keeps, how long a row lives. The columns, the keys and the indexes are in `migrations/`; the
rules to write a migration are in `AGENTS.md`, "Migrations".

## Conventions

Code: `migrations/`, `crates/store/src/lib.rs`

- **DATA-1** Identifiers are ULIDs stored as `text` in their typedid form (`org_0627…`), because they sort by time
  and read well in a log.
- **DATA-2** Timestamps are `timestamptz`.
- **DATA-3** A Postgres enum mirrors its `cihub-domain` enumeration; `postgres_enums_match_the_domain` compares them.
- **DATA-4** No column depends on an SCM: an SCM identifier is an `external_id` in `text`, unique within the
  deployment, because a deployment serves a single SCM.
- **DATA-5** Every object of the schema is read by code that exists. A table that a future feature needs is created
  with that feature, in its final shape.

## Tenancy and RLS

Code: `migrations/20261026100000_create_tenancy_functions.up.sql` (`cihub_tenant_table`),
`crates/store/src/lib.rs` (`begin_org`, `begin_system`)

- **DATA-6** Every tenant table carries `org_id` and an index that starts with it, and goes through
  `cihub_tenant_table`, which enables and forces RLS and creates the `tenant` policy
  `cihub_system() OR org_id = cihub_org_id()` for reads and writes. `orgs` uses its `id` instead of `org_id`.
- **DATA-7** `cihub_store::begin_org` sets `cihub.org_id` for a request after authentication. `begin_system` sets
  `cihub.system` for work that does not know the organization yet (authentication by token fingerprint) or walks
  through all of them (the scheduler, the partitions); these queries filter by `queue_id`, `org_id` or `user_id`.
- **DATA-8** A transaction without context sees no row.
- **DATA-9** The CIHub Postgres role is neither superuser nor `BYPASSRLS`, because it would then see every
  organization; `cihub-server` refuses to start otherwise (`crates/store/src/lib.rs`).
- **DATA-10** `users`, `sessions` and `device_codes` are the only global tables: a session covers every
  organization of its user, and a device code belongs to no organization. The organization of a request is checked
  through `memberships`. `every_tenant_table_forces_row_level_security` fails on a tenant table without its policy.

## Log and projections

Code: `crates/mergequeue/src/projection.rs`, `crates/store/src/events.rs`

- **DATA-11** `events` is the log, partitioned by UTC month on `occurred_at`. `queue_items`, `test_groups`,
  `test_runs`, `check_observations` and the `an_*` fact tables are projections that a replay writes again.
- **DATA-12** `events` is the outbox: a consumer keeps a cursor per queue in `log_cursors` (`seq` is per queue) and
  advances it in the transaction of its effect. There is no `outbox` table, so no double write.
- **DATA-13** `check_observations` has no foreign key to `test_runs`, so that a replay, which rewrites the runs,
  keeps the late reports of checks that have no fact.
- **DATA-14** `pr_titles`, `pr_conditions`, `pr_surfaces`, `queue_setups` and `repo_setups` are outside the log: a
  replay keeps them and no state of a queue depends on them. A setup is a cache of reads of the SCM, one row per
  subject.
- **DATA-15** `webhook_inbox` (raw deliveries, per connection) and `queue_inbox` (translated facts, per queue) are
  two stages, because only the scheduler writes the log, under the lock of the queue.
- **DATA-16** `pr_surfaces` (per queue and PR) and `pr_comments` (per PR, shared by the queues of its branch) have
  different keys and lifetimes. `pr_conditions` stays apart from `pr_surfaces`: each has one writer, and a PR not
  submitted has only `pr_conditions`.
- **DATA-17** CIHub writes one comment per PR. A module writes a section of the comment of `pr_comments`, or
  `pr_comments` gains a `module` column in its key; never one column per module.
- **DATA-18** No enable flag, per organization, per repository or per offer: a module that needs a setting reads
  `.cihub.yaml` (`specs/CONFIGURATION.md`).
- **DATA-19** The settings of a queue come only from `.cihub.yaml`, never from the interface. `queues.config_text`
  keeps the last valid file read on the head of the target, `config_sha` the head of the last read, and
  `config_error` the reason a read file is refused.

## Hot path

Code: `crates/mergequeue/src/projection.rs` (`ItemTransitioned`), `crates/mq-core/src/graph.rs` (`POSITION_STEP`),
`crates/platform/src/inbox.rs` (`record_delivery_failure`)

- **DATA-20** A transition of `queue_items` is an `UPDATE … WHERE id = $1 AND state = $2` whose row count is
  checked, so that two reconcilers never apply the same transition, even if the advisory lock is lost.
- **DATA-21** `position` is the materialized order of the queue (`specs/ALGORITHMS.md`), assigned in steps of
  `POSITION_STEP`; the reconciler renumbers the queue when a gap is exhausted.
- **DATA-22** A PR has at most one live item (`is_current`, unique per queue and PR); its older items stay
  queryable for the history of the PR comment.
- **DATA-23** The result of a job, such as the setup of a queue (`queue_setups`), is never a column of `queues`:
  the lease takes their rows with `FOR UPDATE SKIP LOCKED`, so the scheduler skips the queue while the job holds
  the row (`specs/LESSONS.md`, L-PR-6).
- **DATA-24** A delivery that fails `MAX_DELIVERY_ATTEMPTS` times is set aside (`set_aside_at`) and no pass reads
  it. An operator puts it back with
  `UPDATE webhook_inbox SET attempts = 0, set_aside_at = NULL WHERE delivery_id = '…'`, or deletes it.

## Retention

Code: `crates/platform/src/maintenance.rs` (`ENDED_JOB_RETENTION`, `EVENT_RETENTION`, `PERMISSION_RETENTION`)

- **DATA-25** `webhook_inbox` and `queue_inbox` keep no history: the processing deletes a row in the transaction of
  its facts. A delivery set aside stays for an operator.
- **DATA-26** The daily maintenance deletes the ended `jobs` after `ENDED_JOB_RETENTION`, the expired `sessions`
  and `device_codes`, and the `repo_permissions` checked more than `PERMISSION_RETENTION` ago.
- **DATA-27** The maintenance drops an `events` partition `EVENT_RETENTION` after the end of its month, and only
  when a snapshot of each queue covers each of its events, so that a replay never needs a dropped event.
- **DATA-28** Nothing deletes `queue_items`, `test_runs`, `check_observations`, `audit_log` or the analytics tables:
  they live as long as their organization.

## Where to read the columns

- **DATA-29** Each table is created by one migration, `migrations/<version>_create_<table>.up.sql`, whose comment
  gives the reason of the table when it is not obvious. The API types (`crates/api-types/src/lib.rs`) and the
  OpenAPI document (`schema/openapi/v1.json`) show what a client reads of them.

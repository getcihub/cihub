# Store, migrations and internal transport

- A migration is named `<YYYYMMDDHHMMSS>_<description>.sql` (D41) and is applied by `cihub server migrate` (D14),
  never at startup. Before the first published version, we rewrite a migration rather than add one that fixes it;
  `mise run db-reset` starts again from an empty database.
- Every tenant table carries `org_id`, forced row level security and the `tenant` policy (D38). Only
  `scm_instances` and `users` are global. The test `every_tenant_table_forces_row_level_security` fails otherwise.
- A transaction opens with `begin_org(org_id)` or `begin_system()`; without a context it sees nothing. A system
  query always filters by `queue_id` or `org_id`.
- The identifiers are `text` in the typedid format (D37). The Postgres enums mirror `cihub-domain` through
  `as_str` and `FromStr`; a test compares `pg_enum` with the variants.
- A state transition is an `UPDATE … WHERE id = $1 AND state = $2` whose row count is checked.
- Jobs: key `<type>:<subject>:<version>` (D60, D68), class `JobClass` (D59), executor that answers done, retry
  later or definitive failure (D66), stop after `JOB_TIMEOUT` (D69): an executor accepts being stopped at every
  `.await` and has no effect if the state is already reached. A result comes back to the queue through
  `queue_inbox` (D100): only the scheduler writes the log.
- Tests: `#[sqlx::test]` in `tests/postgres.rs`, a fresh database per test, `mise run test-integration`.
- Documents: `docs/DATA_MODEL.md`, `docs/RELIABILITY.md`.

# cihub-store

- A transaction opens with `begin_org(org_id)` or `begin_system()`; without a context it sees nothing. A system
  query always filters by `queue_id` or `org_id`, or by `user_id` for the organizations of a session user
  (`cihub_platform::memberships_of`); only a sweep of the scheduler reads every organization, to find the queues that have
  work, and it selects their identifiers, then acts on each queue alone.
- The store holds no query of a table of a module: the module that writes a table keeps its queries
  (`cihub-platform` for the users, memberships and API tokens, `cihub-web` for the sessions and device codes).
- A state transition is an `UPDATE … WHERE id = $1 AND state = $2` whose row count is checked.
- The Postgres enums mirror `cihub-domain` through `as_str` and `FromStr`; a test compares `pg_enum` with the
  variants.
- Tests: `#[sqlx::test]` in `tests/postgres.rs`, a fresh database per test, `mise run rust:integration`.
- Spec: `specs/DATA_MODEL.md`.

# Migrations

- A migration is named `<YYYYMMDDHHMMSS>_<description>.sql` and is applied by `cihub server migrate`, never at
  startup.
- Before the first published version, rewrite a migration rather than add one that fixes it; `mise run db-reset`
  starts again from an empty database.
- Every tenant table carries `org_id`, forced row level security and the `tenant` policy. Only `users` and
  `sessions` are global: a session covers every organization of its user. The test
  `every_tenant_table_forces_row_level_security` fails otherwise.
- Identifiers are `text` in the typedid format. A Postgres enum mirrors its `cihub-domain` enumeration.
- Spec: `specs/DATA_MODEL.md`.

# cihub-domain

- A new state, reason or event type is a variant with its `///` comment here, a value of the Postgres enum
  (`migrations/`) and a definition in `specs/DOMAIN.md`, in the same change. The enum test of `cihub-store`
  compares `pg_enum` with the variants.
- A single `Reason` enumeration and one table of allowed transitions per state machine.
- Identifiers are typed ULIDs in the `typedid` text format. In tests, use fixed identifiers
  (`OrgId::from_ulid(Ulid(1))`).

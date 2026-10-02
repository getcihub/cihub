# Pure crates (principle II)

- No I/O, no log, no reading of the clock or of randomness: the time, the identifiers and the settings arrive as
  parameters (`plan(snapshot, settings, now)`, D25). `clippy.toml` and `deny.toml` check it.
- Replay applies the events with `apply` and never calls `plan` (D5). The identifiers of the decisions are
  deterministic ULIDs.
- A single `Reason` enumeration and one table of allowed transitions per state machine (D6). A new state, reason
  or event type: variant with its `///` comment in `cihub-domain`, value in the Postgres enum (`migrations/`),
  definition in `docs/DOMAIN.md`, in the same change. The enum test of `cihub-store` compares the two.
- `cihub-mq-core` depends on `cihub-domain` and `cihub-config`, not on `cihub-conditions` (D24, D99). A strategy
  is a table of default values in `cihub-config`, not code in the core.
- Tests: tables (one row per transition), `proptest` (random queues verified by `check`), `insta` for the plans.
  Fixed identifiers in tests (`OrgId::from_ulid(Ulid(1))`). Coverage 90% at least: `mise run cover`.
- Documents: `docs/ALGORITHMS.md`, `docs/DOMAIN.md`, `docs/CONFIGURATION.md`.

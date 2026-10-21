# cihub-mq-core

- The time, the identifiers and the settings arrive as parameters of `plan`. The identifiers of the decisions are
  deterministic ULIDs.
- Replay applies the events with `apply` and never calls `plan`.
- It depends on `cihub-domain` and `cihub-config` only, not on `cihub-conditions`. A strategy is a table of
  default values in `cihub-config`, not code here.
- Tests: tables (one row per transition), `proptest` (random queues verified by `check`), `insta` for the plans.
  Coverage 90% at least: `mise run rust:cover`.
- Spec: `specs/ALGORITHMS.md`.

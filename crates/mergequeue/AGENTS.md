# cihub-mergequeue

- Only the scheduler writes the log of a queue, under its lease. The jobs call the SCM and give their facts back
  through `queue_inbox`; the scheduler never calls the SCM. The one read before it starts is the mention of the
  App (`read_app_mention`), which it routes the commands and writes the comments with.
- The decisions come from `cihub-mq-core`; this crate loads, writes and shows them. No queue logic here that
  `plan` could hold.
- Every queue scenario runs on the simulator (`cihub-scm`, feature `sim`); `tests/postgres.rs` runs with
  `mise run rust:integration`.
- Specs: `specs/ALGORITHMS.md`, `specs/DOMAIN.md`, `specs/CI.md`, `specs/RELIABILITY.md`.

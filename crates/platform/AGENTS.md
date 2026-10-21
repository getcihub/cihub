# cihub-platform

- A job has the key `<type>:<subject>:<version>` and a `JobClass`. Its executor answers `Outcome::Done`, `Retry`
  or `Failed`, and stops after `JOB_TIMEOUT`: it accepts being stopped at every `.await` and has no effect if the
  state is already reached.
- A job gives its result back to a queue through `queue_inbox`: only the scheduler writes the log.
- Tests: `#[sqlx::test]` in `tests/postgres.rs`, `mise run rust:integration`.
- Spec: `specs/RELIABILITY.md`.

# Analytics

This spec covers the facts that CIHub derives from the log of a queue and the Health of a queue that reads them.
It does not list the columns: the migrations `create_an_*` and the `///` comments of `cihub-analytics` hold them.

## Facts and aggregate

Code: `crates/analytics/src/facts.rs` (`record_facts`), `crates/analytics/src/hourly.rs`,
`crates/platform/src/subscriptions.rs`, `crates/mergequeue/src/lib.rs` (the `analytics` subscription)

- **AN-1** The facts come only from the log `events`, read after the cursor of the `analytics` subscription in
  `log_cursors`; they read no projection, because a replay of the log must write the same facts.
- **AN-2** The facts, their hourly aggregate and the advance of the cursor are written in one transaction, so
  that each fact counts once, also after a crash or a log read again.
- **AN-3** The Health of a queue reads only the hourly aggregate (`an_hourly_*`, keyed by queue and UTC hour), so
  that its cost follows the length of a period, not the number of PRs. On a queue of 30,000 PRs a day, the Health
  of 30 days took 37 s read from the facts and takes 0.1 s read from the aggregate.
- **AN-4** Each bucket of a histogram of durations is 10 % wider than the one before it, so that a median or a 90th
  percentile read from it is within 5 % of the exact value.
- **AN-5** The aggregate is three typed tables (conclusions, durations, checks), not one generic measurements
  table: a reason, a check and a bucket stay columns of their own type.
- **AN-6** A fact carries no actor: the log keeps the user ID, not the login (`specs/LESSONS.md`, L-LOG-3).
- **AN-7** A period of the Health starts at most 13 months back (`KEPT` of `crates/analytics/src/health.rs`).

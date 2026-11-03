# Observability

This spec gives the contract of the logs of CIHub: their format, their correlation keys, and the rules to write
them; then the rules of the error reports, which an operator can send to a Sentry server. The settings are
`log.*` and `sentry.*` (`crates/server/src/config.rs` (`KEYS`)); the subscriber is in
`crates/telemetry/src/logs.rs` (`Logs`).

## Purpose

- **OBS-1** The event timeline of an item says why it changed state, and a customer reads it; the logs say what the
  process did, for an operator. An information that a customer needs goes in an event with its reason
  (`specs/DOMAIN.md`), never only in a log.
- **OBS-2** Only `cihub-server` installs the subscriber, through `cihub-telemetry`; the library crates write with
  the `tracing` macros and open spans. The pure crates write no logs: their caller writes their decisions.

## Log format

Code: `crates/telemetry/src/flat_json.rs` (`FlatJson`)

- **OBS-3** In `json`, a record is one flat JSON object per line: the fields of the event and of every open span
  are side by side at the top level, so that a query reads `queue_id` whatever span carries it. When two give the
  same field, the event wins, then the span closest to the event.
- **OBS-4** A record has no list, because the `json` parser of Loki skips the arrays.
- **OBS-5** `timestamp` is RFC 3339 UTC in milliseconds; `level` is `DEBUG`, `INFO`, `WARN` or `ERROR`; `target`
  is the Rust module that writes.
- **OBS-6** `pretty` is one colored line per record, for a terminal; `mise run dev` uses it.

## Correlation keys

Code: `crates/web/src/lib.rs` (`webhook` span), `crates/mergequeue/src/scheduler.rs` (`reconcile` span),
`crates/platform/src/worker.rs` (`job` span), `crates/web/src/api.rs` (`request` span),
`crates/server/src/start.rs` (`role` span)

- **OBS-7** Each unit of work opens one span that carries its keys, and every line written inside inherits them.
  The called code does not repeat them.
- **OBS-8** A key that the unit of work learns after it starts is declared `tracing::field::Empty` and recorded
  as soon as it is known.
- **OBS-9** These names are the only ones allowed for these data: `org_id`, `queue_id`, `repo_id`, `pr_number`,
  `item_id`, `group_id`, `run_id`, `delivery_id`, `job_key`, `job_type`, `request_id`, `role`. A new key shared
  between crates is added to this list.
- **OBS-10** A key is an identifier, never a name: a repository can be renamed, its `repo_id` stays.

## Log writing rules

The lints check the rules marked _lint_ (clippy, and `scripts/lint-rules.sh`); the review checks the others.

- **OBS-11** No `println!`, `eprintln!` or `dbg!` outside the examples and the normal output of the CLI.
  _(lint: clippy `print_stdout`, `print_stderr`, `dbg_macro`)_
- **OBS-12** The message is the name of the event, `<object>.<action>` in snake_case, the action as a past
  participle (`"submission.refused"`). It carries no variable data: the reason goes in `reason`, with a value of
  the closed vocabulary when one exists, else a snake_case constant, and the other data in their fields, so that a
  query on `message` finds every line of one event. _(lint: `scripts/lint-rules.sh`)_
- **OBS-13** The fields are in snake_case.
- **OBS-14** An error goes in the `error` field (`error = &err as &dyn std::error::Error`) and is written once,
  where it is handled. A function that returns the error wraps it (`#[source]`) and does not write it.
- **OBS-15** A duration is an integer suffixed by its unit (`duration_ms`).
- **OBS-16** The level says who must read the line. `DEBUG`: a developer, off in production (each SCM call, a
  request refused with a 4XX). `INFO`: one line per decision or state change. `WARN`: a problem that CIHub handles
  alone (a retry, a lost lease). `ERROR`: an operator must act (a dead job, a delivery set aside, a request failed
  with a 5XX).
- **OBS-17** A red CI, an evicted PR or a duplicate webhook is a normal result: `INFO`, never `ERROR`.
- **OBS-18** No secret and no raw tenant data: no token, key, webhook payload, diff or SCM request body; write the
  identifier (`delivery_id`, `run_id`). A secret is a `secrecy::SecretString`, whose display is masked.
- **OBS-19** An expensive field is computed only when it is written: guard it with
  `tracing::enabled!(Level::DEBUG)`, or use a value that formats lazily.
- **OBS-20** A test does not write to standard output. A test that checks a log installs
  `cihub_telemetry::CapturedLogs` (feature `capture`) and reads its `records`.

## Error reports

Code: `crates/telemetry/src/error_reports.rs`, `crates/web/src/dashboard.rs` (`Index`),
`apps/console/src/lib/error-reports.ts`

- **OBS-21** Error reports are off until the operator gives a DSN: `sentry.dsn` for the server and
  `sentry.browser_dsn` for the dashboard. Without a DSN, nothing is sent and the dashboard does not load the SDK.
  A DSN that does not parse stops the process at startup (DEP-6).
- **OBS-22** The server sends an `ERROR` record that the log filter keeps, and a panic, as a Sentry event with the
  keys of the open spans (`org_id`, `queue_id`...). The other levels are not sent.
- **OBS-23** The dashboard sends what its scripts throw and what its React root catches, from its own origin only.
  It drops the refusals of the API (4xx) and the errors that only the browser or an extension causes.
- **OBS-24** A DSN is public, so it lives in the ConfigMap. The console is embedded in the binary, so `cihub-web`
  writes the DSN, the environment and the release as `<meta>` elements into `index.html`, and the script reads
  them before its first render.
- **OBS-25** The release is `cihub@<version>` for the server and the dashboard; `sentry.environment` is optional
  and common to both.
- **OBS-26** The console build injects a Debug ID into each bundle and its source map (`sentry-cli sourcemaps inject`,
  offline), and the SDK sends it with each frame. The instance also serves the maps next to its bundles; a Sentry
  server that can reach the instance needs nothing else. One that cannot gets minified frames until it has the maps.
- **OBS-27** Each release attaches `console-sourcemaps.tar.gz` (the bundles and their maps) to its GitHub release,
  because the release CI cannot upload to the Sentry server of an operator. An operator of a private instance runs
  `sentry-cli sourcemaps upload` on the unpacked archive, once per version. The Debug IDs match any URL.

## Not built yet

- Metrics (no `/metrics` route), traces with `trace_id` and `span_id` in the records, alerts and dashboards.

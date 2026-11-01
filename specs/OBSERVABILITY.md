# Observability

Each process emits OpenTelemetry traces, Prometheus metrics and structured logs with the same correlation keys (`org_id`, `queue_id`, `item_id`, `run_id`, `delivery_id`): a question like "why did PR 4821 restart at 2:03 pm" is answered first by the event timeline of the item, then by the traces. The timeline is the main debugging tool and it is exposed to customers; the rest serves to operate the service.

| Signal                  | Content                                                                                                                                                                                                                                                                                                                                                                                                  | Where                                                                                                                                                                       |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Item timeline           | Each transition with its reason, its actor and its payload                                                                                                                                                                                                                                                                                                                                                | `events` Postgres (90 days), fact tables or ClickHouse (13 months), PR page of the dashboard, API `GET …/prs/{number}/events`                                                       |
| Service metrics         | `cihub_reconcile_duration_seconds{queue}`, `cihub_reconcile_errors_total`, `cihub_jobs_inflight{type}`, `cihub_job_duration_seconds{type}`, `cihub_job_failures_total{type,reason}`, `cihub_webhook_inbox_lag_seconds`, `cihub_outbox_lag_seconds`, `cihub_github_requests_total{installation,endpoint,status}`, `cihub_github_rate_limit_remaining{installation}`, `cihub_clickhouse_insert_batch_size` | Prometheus, collected on `/metrics` of the internal address of each process, anonymous by default or with authentication (machine account or admin, `specs/SECURITY.md`)    |
| Product metrics         | The `mq_*` series per queue (depth, wait for mergeability, slots, conclusions, restarts, histograms of wait, test and time in queue)                                                                                                                                                                                                                                                                      | `GET /api/v1/metrics` on the customer side, and the same series in the internal Prometheus with an `org` label for the fleet views                                          |
| Traces                  | One trace per webhook delivery, from the inbox to the reconciliation then to the jobs and the GitHub calls; one per API request                                                                                                                                                                                                                                                                           | OpenTelemetry collector, sampling 10 % plus 100 % of the errors                                                                                                             |
| Logs                    | Structured, one line per reconciliation decision (`decision=start_run group=... predecessors=...`) and per job step                                                                                                                                                                                                                                                                                       | Loki or the log storage of the cloud, 30 days                                                                                                                               |

## Logs

The logs use `tracing` and `tracing-subscriber`. Only `cihub-server` installs the
subscriber, through `cihub-telemetry`; the library crates write with the `tracing` macros
(`info!`, `warn!`...) and open spans. The pure crates write no logs: the caller writes their
decisions.

### Settings

| Key          | Variable            | Default | Role                                                                                                                             |
| ------------ | ------------------- | ------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `log.filter` | `CIHUB_LOG_FILTER`  | `info`  | Level filter, global or per crate, in the `EnvFilter` syntax: `info,cihub_github=debug` puts a single crate in `debug`          |
| `log.format` | `CIHUB_LOG_FORMAT`  | `json`  | `json` (one JSON line per record, for the log storage) or `pretty` (one colored text line, for a terminal)                      |

The keys are those of the configuration file (`specs/DEPLOYMENT.md`). An invalid value stops the process at startup. `mise run dev` uses `pretty`.

A `web` replica that loses its connection to `cihub_queue_changed` writes `warn` (`queue_changes.not_heard`, or `queue_changes.connection_lost`): the open pages then read everything again. No metric counts the open streams yet.

### Format

In `json`, a record is one flat JSON object on one line. The fields of the event and the fields of all the
open spans are at the top level, side by side: a query on a queue reads `queue_id`, whatever the span that
carries it. When two give the same field, the event wins, then the span closest to the event. The record
has no list, because the `json` parser of Loki skips the arrays:

```json
{"timestamp":"2026-09-29T14:03:12.481Z","level":"INFO","message":"submission.refused","reason":"permission","target":"cihub_mergequeue::jobs::submit_pr","role":"worker","job_type":"submit_pr","job_key":"submit_pr:72d3162e-cc78-11e3-81ab-4c9367dc0958:1","org_id":"org_0627ank54ajth78mw4353dqpw5dvt5kp","queue_id":"queue_05ywjdspm06ftp1ahh11w4vk086z2kjk","repo_id":"repo_01hx3v9q2k8m4n6p7r5s3t2w1yzab","pr_number":4821}
```

`timestamp` is in RFC 3339 UTC, in milliseconds; `level` is `DEBUG`, `INFO`, `WARN` or `ERROR`; `target` is
the Rust module that writes. The `trace_id` and `span_id` are added when the OpenTelemetry traces arrive.

### Correlation keys

Each unit of work opens a span that carries its keys; all the lines written inside inherit them. A line
says where it happened (organization, queue, repository, PR) without repeating it in each call.

| Span        | Opened by                                     | Keys                                                                                                   |
| ----------- | --------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| `webhook`   | reception of a webhook (`web` role)           | `delivery_id`                                                                                          |
| `reconcile` | reconciliation of a queue (`scheduler` role)  | `org_id`, `queue_id`                                                                                   |
| `job`       | execution of a job (`worker` role)            | `job_type`, `job_key`, `org_id` and `queue_id` of the job; then `repo_id`, `pr_number`, `item_id`, `run_id` when the job reads them from its payload |
| `request`   | API request (`web` role)                      | `request_id`, then `org_id` after authentication                                                       |

Each process also opens one span per role, with the `role` key. A key that the unit of work learns after
it starts is declared `tracing::field::Empty` in the span and set with `Span::current().record(...)` as soon
as it is known. The key names above are the only ones allowed for these data (`org_id`, `queue_id`,
`repo_id`, `pr_number`, `item_id`, `group_id`, `run_id`, `delivery_id`, `job_key`, `job_type`, `request_id`,
`role`); the same names serve as span attributes and as metric labels. A key is an identifier, never a
name: a repository can be renamed, its `repo_id` stays.

### Log writing rules

These rules apply to all the code, written by a human or by an agent. The lints (clippy and
`scripts/lint-rules.sh`) check those that carry the mention _lint_; review checks the others.

1. **No direct output.** No `println!`, no `eprintln!`, no `dbg!` outside the command-line examples and
   the normal output of the CLI. _(lint: `print_stdout`, `print_stderr`, `dbg_macro`)_
2. **The correlation keys are added once, at the start of the unit of work**, in its span
   (`#[instrument(skip_all, fields(queue_id = %queue_id))]` or `info_span!`). The called code does not repeat them.
3. **The message is the name of the event: `<object>.<action>`**, in snake_case, the object first and the
   action as a past participle (`"submission.refused"`, `"test_branch.assembled"`,
   `"webhook.signature.refused"`). The message carries no reason and no variable data: the reason goes in
   the `reason` field, with a value of the closed vocabulary of `specs/DOMAIN.md` when one exists, else a
   constant in snake_case; the other data go in their fields. Write
   `info!(reason = "permission", "submission.refused")`, never `info!("submission refused: permission")` nor
   `info!("item {pr_number} submitted")`. A query on `message` thus finds all the lines of the same event,
   and a query on `reason` splits them. _(lint: `scripts/lint-rules.sh`)_
4. **The fields are in snake_case.** A new key shared between crates is added to the list above.
5. **An error uses the `error` field** and is written only once, where it is handled:
   `warn!(error = &err as &dyn std::error::Error, "github call retried")`. The `pretty` format also writes the
   chain of causes (`error.sources`); the `json` format of `tracing-subscriber` writes only the message of
   the error. A function that returns the error wraps it (`#[source]`) and does not write it.
6. **A duration is an integer suffixed by its unit** (`duration_ms`), not a `Duration` nor a string.
7. **The level says who must read the line.**

   | Level   | When                                              | Examples                                                                                                                                                    |
   | ------- | ------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
   | `DEBUG` | Detail for a developer; off in production         | Each GitHub call and its cost, each retried request, the gap between the plan and the observed state, an API request refused with a 4XX status            |
   | `INFO`  | One line per decision or state change             | Reconciliation decision (`decision=start_run group=... predecessors=...`), job step, webhook stored, migration applied, degraded mode change                |
   | `WARN`  | A problem that CIHub handles alone                | Retry after a GitHub error, failed delivery of a connection (counted, retried at the next pass), budget in `throttled` mode, GitHub unavailable, queue lease lost                                                                 |
   | `ERROR` | An operator must act                              | Job sent to the dead-letter queue, delivery of a connection set aside after 5 failures, invariant violated, migration failed, an API request failed with a 5XX status |

   A red CI, a PR evicted from the queue or a duplicate webhook are normal results: `INFO`, never
   `ERROR`. An API request refused with a 4XX status is an error of the client, not of CIHub: `DEBUG`.

8. **No secret, no raw tenant data.** No token, key, webhook secret, webhook payload,
   diff or GitHub request body: write the identifier (`delivery_id`, `run_id`). A secret is
   a `secrecy::SecretString`, whose display is masked.
9. **An expensive field is computed only when it is written**: the call is guarded by
   `tracing::enabled!(Level::DEBUG)`, or the field is a value that formats lazily.
10. **The logs do not replace the timeline.** The event timeline says why an item changed
    state; the logs say what the process did. An information that a customer needs goes in an
    event with its reason (`specs/DOMAIN.md`), never only in a log.
11. **The tests do not write to standard output.** A test that checks a log installs the subscriber of
    `cihub_telemetry::CapturedLogs` (feature `capture`, in the dev-dependencies) (`tracing::subscriber::with_default`, or `set_default` in an async
    test), which keeps the lines of the `json` format in memory, then reads its `records`.

## Alerts and dashboards

Alerts that wake someone up: more than 1 % of reconciliation errors for 5 min; webhook inbox lag beyond 60 s; a queue where a run exceeds `deadline_at` by more than 10 min and stays in progress (stuck timeout); outbox lag beyond 10 min; GitHub rate limit budget under 10 % for an installation with live items; job dead-letter queue not empty. Alerts as simple notification: ClickHouse insert failures, customer webhook endpoints disabled automatically, App installation suspended.

Operations dashboards: fleet view (queues by state, items by state, runs in progress, merges per hour), SCM budget per connection, job throughput and error breakdown by type and reason, and a "slowest queues" view sorted by p95 time in queue over the last hour. Customers see the Health view (conclusions, time in queue, test duration, restarts by cause) and the live graph.

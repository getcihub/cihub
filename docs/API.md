# APIs

The public API is a resource-oriented REST under `/api/v1`, on the address of the instance, next to the dashboard (`https://cihub.example.com/api/v1`); it has structured errors and a queue list endpoint; no route creates a queue, the first `/cihub merge` on a branch does (`DECISIONS.md` D172). Authentication is by `Authorization: Bearer <token>` with API tokens owned by the organization (scopes `queues:read`, `queues:write`, `prs:write`, `targets:write`, `metrics:read`) or by a dashboard session; the CLI uses the same API.

| Method             | Path                                                     | Role                                                                                                                                                                                                                                           |
| ------------------ | -------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| GET                | `/repos`                                                 | Repositories visible to the token                                                                                                                                                                                                              |
| GET                | `/repos/{owner}/{name}/queues`                           | List the queues; the settings come from the `.cihub.yaml` file (`DECISIONS.md` D134) |
| GET, PATCH         | `/queues/{queue_id}`                                     | Read, or request a `state` (`running`, `paused`, `draining`); deletion is deferred (`DECISIONS.md` D160) |
| GET                | `/queues/{queue_id}/items`                               | Live items and history, filters `state`, `since`, `until`, cursor pagination with `take` ≤ 100                                                                                                                                                 |
| POST               | `/queues/{queue_id}/items`                               | Submit a PR: `pr_number`, `priority` (name or 0–255), `no_batch`, `direct`                                                                                                                                                                     |
| GET                | `/queues/{queue_id}/prs/{pr_number}`                     | Latest item of a PR, `?include=timeline`; the search by `?branch=` is deferred (D160) |
| POST               | `/queues/{queue_id}/prs:batchGet`                        | Up to 50 PRs by number, returns `not_found[]` |
| POST               | `/items/{item_id}/cancel`, `/items/{item_id}/restart`    | Cancel or restart the tests                                                                                                                                                                                                                    |
| GET                | `/queues/{queue_id}/graph`                               | The merge graph as nodes and edges for the dashboard                                                                                                                                                                                           |
| GET                | `/test-runs/{run_id}`                                    | Detail of a run (format below, frozen at Lot 6, `DECISIONS.md` D115): branch, SHA, all the observed check runs and statuses with the required ones marked, tested PRs, impacted targets, dependents                                            |
| GET                | `/queues/{queue_id}/test-results`                        | One row per PR and per attempt over a range (≤ 45 days): `outcome` (`passed`, `failed`, `interrupted`, `cancelled`), `cycle_conclusion`, `bisection` (`entered_bisection`, `in_bisection`), `run_id`; offset pagination with `total`           |
| POST               | `/repos/{owner}/{name}/prs/{pr_number}/impacted-targets` | Body `{head_sha, targets}` with a list or `"ALL"`; 20 MB limit; also accepts a fork workflow identifier instead of a token                                                                                                                     |
| POST               | `/queues/{queue_id}:resync`                              | Admin: re-read from GitHub each live PR and each run                                                                                                                                                                                           |
| GET                | `/metrics`                                               | Prometheus exposition, filters `repo`, `branch`                                                                                                                                                                                                |
| GET, POST, DELETE  | `/orgs/{org}/webhooks`                                   | Manage the outgoing webhook endpoints, test a delivery                                                                                                                                                                                         |
| GET                | `/orgs/{org}/audit`                                      | Setting changes and admin actions                                                                                                                                                                                                              |

An item (`Item` in `cihub-api-types`) carries `state_reason`, the code of the closed vocabulary, and
`state_description`, its sentence for a person (`Reason::describe`, `docs/DOMAIN.md`). A client shows the
sentence and decides on the code: the sentence can change without notice.

**`GET /test-runs/{run_id}`** returns a JSON object whose fields are the columns of `test_runs` and of
`check_observations` (`docs/DATA_MODEL.md`), with the names of the closed vocabulary: `id`, `queue_id`, `group_id`,
`attempt`, `status`, `status_reason`, `test_branch`, `test_sha`, `base_sha`, `draft_pr_number`,
`tested_pr_numbers`, `head_shas` (the merged heads in order: predecessors then members),
`predecessor_pr_numbers`, `required_statuses`, `required_statuses_source`, `started_at`, `ended_at`,
`last_report_at`, `failure_reason`, `failure_causing_statuses`, `conflict_culprit` (`{kind: queue_pr, pr_number}`
or `{kind: base_branch}`), `checks` (one entry per observed check: `name`, `kind`, `required`, `conclusion`,
`details_url`, `observed_at`), `impacted_targets` (`all` or the list, Lot 13) and `dependents` (the live runs
whose predecessors contain a tested PR: `run_id`, `pr_numbers`). An absent value is
`null`.

Errors are in `application/problem+json` with `type`, `title`, `status`, `detail` and a machine `code`, whose list is `ErrorCode` in `cihub-api-types` (`unauthorized`, `forbidden`, `not_found`, `invalid_request`, `rate_limited`, `invalid_state`, `internal`). Rate limit: 600 requests per minute and per token, in each replica of the server (`DECISIONS.md` D159), `Retry-After` on 429s. The scopes of each route are in D160. A submission, a cancellation, a restart, a state request and a re-read respond `202`: the queue applies them at its next reconciliation. The OpenAPI 3.1 document is generated from the code (`cihub-web`, `cihub-api-types` and the routes of the modules) and published without a token on `/api/v1/openapi.json`; the TypeScript client of the dashboard will be derived from it, and the Rust client `cihub-client` shares the types of `cihub-api-types` directly. Deferred (D160): `GET /queues/{queue_id}/test-results` (Lot 11), `impacted-targets` (Lot 13), `/metrics` and `/orgs/{org}/webhooks` (Lot 11).

Outgoing webhooks follow the [Standard Webhooks](https://www.standardwebhooks.com) specification (headers `webhook-id`, `webhook-timestamp`, `webhook-signature` in HMAC-SHA256), are retried with exponential backoff over 24 hours and disabled after 7 days of failures. The event types, the payload fields and the reasons follow CIHub's vocabulary (`docs/DOMAIN.md`), without translation to that of another product (`DECISIONS.md` D139); their list is frozen at Lot 11. Per-PR events are emitted for each member of a batch.

The command line is the `cihub` binary (crate `cihub-cli`, `DECISIONS.md` D161, D168, D169). `cihub login` and `cihub queue` only talk to the public API, through `cihub-client`; `cihub server` talks to Postgres and is described in `docs/DEPLOYMENT.md`. `cihub login --url <address> [--token <token>]` keeps the address and the token in `$XDG_CONFIG_HOME/cihub/credentials.json` (mode 0600), and reads the token from the standard input without `--token`; `CIHUB_URL` and `CIHUB_TOKEN` take precedence over the file.

| Command                                                  | Role                                                     | Scope          |
| -------------------------------------------------------- | -------------------------------------------------------- | -------------- |
| `cihub queue merge <pr> [-p urgent\|high\|medium\|low\|0-255] [--no-batch]` | submits a PR                      | `prs:write`    |
| `cihub queue cancel <pr>`                                | takes the PR out of the queue                            | `prs:write`    |
| `cihub queue restart <pr>`                               | tests the PR again                                       | `prs:write`    |
| `cihub queue status [<pr>]`                              | the state of the queue, or of a PR                       | `queues:read`  |
| `cihub queue graph`                                      | the merge graph of the queue                             | `queues:read`  |
| `cihub queue pause`                                      | stops the tests and the merges; the queue keeps its order | `queues:write` |
| `cihub queue resume`                                     | starts the queue again                                   | `queues:write` |
| `cihub queue drain`                                      | finishes the items in the queue and refuses new ones     | `queues:write` |

Every `cihub queue` command takes `--repo <owner/name>`, else the repository of the `origin` remote of git, and `--queue <name>`, else the only queue of the repository. With two or more queues and no `--queue`, the command fails, lists the names and does nothing; with none, it fails and says that the first `/cihub merge` on a PR of the default branch creates the queue. `cihub queue targets upload` arrives at Lot 13.

The internal interfaces are not RPC: the processes share Postgres, so the "internal API" consists of the `jobs` and `outbox` tables and the `NOTIFY` channels `cihub_queue`, `cihub_jobs`, `cihub_outbox`.

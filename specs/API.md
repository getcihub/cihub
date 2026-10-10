# APIs

The public API is a resource-oriented REST under `/api/v1`, on the address of the instance, next to the dashboard (`https://cihub.example.com/api/v1`); it has structured errors and a queue list endpoint; no route creates a queue, the first `/cihub merge` on a branch does. Authentication is by `Authorization: Bearer <token>` with API tokens owned by the organization (scopes `queues:read`, `queues:write`, `prs:write`) or by a session of the dashboard; the CLI uses the same API. A session is the cookie `__Host-cihub_session`; the role of its user in the organization of the URL gives its scopes (`member`: `queues:read` and `prs:write`; `admin`: every scope), and an organization where the user has no role answers `404`. A request of a session that can change a state (any method but `GET`, `HEAD` and `OPTIONS`) also carries the header `X-CSRF-Token`, whose value the cookie `__Host-cihub_csrf` gives; without it, `403` with the code `csrf_refused`, which tells it apart from a missing scope. `GET /auth/github` and its callback, outside `/api/v1`, sign a person in and create the session (`specs/GITHUB.md`); `POST /auth/sign-out` deletes it and clears both cookies (`204`).

Every route names its organization by its slug, then the repository and the queue by their names, not by an
identifier: `/orgs/{org}/repos/{repo}/queues/{queue}`. The URL thus says which organization it reads, so a
link or two browser tabs never depend on a hidden active organization. The token or the session must belong to
the organization of the URL; otherwise the answer is `404 not_found`, never `403`, so that a caller does not
learn that another organization exists. The slug and the name of the repository do not tell the case apart, as
the SCM. An operation that the methods of HTTP do not name is a `POST` with its name after a colon in the last
segment (`:resync`, `:cancel`, `:restart`, `:batchGet`). The paths below follow `/api/v1`; `…` stands for
`/orgs/{org}/repos/{repo}/queues/{queue}`.

| Method             | Path                                                     | Role                                                                                                                                                                                                                                           |
| ------------------ | -------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| GET                | `/me`                                                    | The user of a session, the organizations that they reach with their role (none for a person who has not installed the App yet), the version of the instance, the page of the SCM that installs the App; a token answers `401`                    |
| GET                | `/orgs/{org}/repos`                                      | Repositories of the organization                                                                                                                                                                                                               |
| GET                | `/orgs/{org}/queues`                                     | Every queue of the organization with its counters, by repository name then queue name: `counts` (the current items in each state that is not terminal), `merged_last_24h` and `failed_last_24h` (the items that reached `merged` or `failed` in the 24 hours before the request, not in a calendar day; a PR that failed, then came back, still counts its failure), `last_merged_at` |
| GET                | `/orgs/{org}/repos/{repo}/queues`                        | List the queues; the settings come from the `.cihub.yaml` file |
| GET, PATCH         | `…`                                                      | Read, or request a `state` (`running`, `paused`, `draining`); deletion is deferred |
| GET                | `…/prs`                                                  | Live items and history, filters `state`, `since`, `until`, cursor pagination with `take` ≤ 100                                                                                                                                                 |
| POST               | `…/prs`                                                  | Submit a PR: `pr_number`, `priority` (name or 0–255), `no_batch`, `direct`                                                                                                                                                                     |
| GET                | `…/prs/{number}`                                         | Latest item of a PR, `?include=timeline`; the search by `?branch=` is deferred |
| POST               | `…/prs:batchGet`                                         | Up to 50 PRs by number, returns `not_found[]` |
| POST               | `…/prs/{number}:cancel`, `…/prs/{number}:restart`        | Cancel or restart the tests of the current item of the PR                                                                                                                                                                                      |
| GET                | `…/graph`                                                | The merge graph as nodes and edges for the dashboard                                                                                                                                                                                           |
| GET                | `…/config-check`                                         | What the configuration of the target branch misses; no finding means a sound configuration                                                                                                                               |
| GET                | `/orgs/{org}/runs/{run_id}`                              | Detail of a run (format below, frozen at Lot 6): branch, SHA, all the observed check runs and statuses with the required ones marked, tested PRs, impacted targets, dependents                                            |
| GET                | `…/test-results`                                         | One row per PR and per attempt over a range (≤ 45 days): `outcome` (`passed`, `failed`, `interrupted`, `cancelled`), `cycle_conclusion`, `bisection` (`entered_bisection`, `in_bisection`), `run_id`; offset pagination with `total`           |
| POST               | `/orgs/{org}/repos/{repo}/prs/{number}/impacted-targets` | Body `{head_sha, targets}` with a list or `"ALL"`; 20 MB limit; also accepts a fork workflow identifier instead of a token                                                                                                                     |
| POST               | `…:resync`                                               | Admin: re-read from GitHub each live PR and each run                                                                                                                                                                                           |
| GET                | `/metrics`                                               | Prometheus exposition, filters `repo`, `branch`                                                                                                                                                                                                |
| GET, POST, DELETE  | `/orgs/{org}/webhooks`                                   | Manage the outgoing webhook endpoints, test a delivery                                                                                                                                                                                         |
| GET                | `/orgs/{org}/audit`                                      | Setting changes and admin actions                                                                                                                                                                                                              |

An item (`Item` in `cihub-api-types`) carries `state_reason`, the code of the closed vocabulary, and
`state_description`, its sentence for a person (`Reason::describe`, `specs/DOMAIN.md`). A client shows the
sentence and decides on the code: the sentence can change without notice.

**`GET /orgs/{org}/runs/{run_id}`** returns a JSON object whose fields are the columns of `test_runs` and of
`check_observations` (`specs/DATA_MODEL.md`), with the names of the closed vocabulary: `id`, `queue_id`, `group_id`,
`attempt`, `status`, `status_reason`, `test_branch`, `test_sha`, `base_sha`, `draft_pr_number`,
`tested_pr_numbers`, `head_shas` (the merged heads in order: predecessors then members),
`predecessor_pr_numbers`, `required_statuses`, `required_statuses_source`, `started_at`, `ended_at`,
`last_report_at`, `failure_reason`, `failure_causing_statuses`, `conflict_culprit` (`{kind: queue_pr, pr_number}`
or `{kind: base_branch}`), `checks` (one entry per observed check: `name`, `kind`, `required`, `conclusion`,
`details_url`, `observed_at`), `impacted_targets` (`all` or the list, Lot 13) and `dependents` (the live runs
whose predecessors contain a tested PR: `run_id`, `pr_numbers`). An absent value is
`null`.

Errors are in `application/problem+json` with `type`, `title`, `status`, `detail` and a machine `code`, whose list is `ErrorCode` in `cihub-api-types` (`unauthorized`, `forbidden`, `csrf_refused`, `not_found`, `invalid_request`, `rate_limited`, `invalid_state`, `internal`). Rate limit: 600 requests per minute and per token or session, in each replica of the server, `Retry-After` on 429s. The scopes of each route are in the OpenAPI document (`/api/v1/openapi.json`). A submission, a cancellation, a restart, a state request and a re-read respond `202`: the queue applies them at its next reconciliation. The OpenAPI 3.1 document is generated from the code (`cihub-web`, `cihub-api-types` and the routes of the modules) and published without a token on `/api/v1/openapi.json`; the TypeScript client of the dashboard will be derived from it, and the Rust client `cihub-client` shares the types of `cihub-api-types` directly. Deferred: `GET …/test-results` (Lot 11), `impacted-targets` (Lot 13), `/metrics` and `/orgs/{org}/webhooks` (Lot 11).

Outgoing webhooks follow the [Standard Webhooks](https://www.standardwebhooks.com) specification (headers `webhook-id`, `webhook-timestamp`, `webhook-signature` in HMAC-SHA256), are retried with exponential backoff over 24 hours and disabled after 7 days of failures. The event types, the payload fields and the reasons follow CIHub's vocabulary (`specs/DOMAIN.md`), without translation to that of another product; their list is frozen at Lot 11. Per-PR events are emitted for each member of a batch.

The command line is the `cihub` binary (crate `cihub-cli`). `cihub login` and `cihub queue` only talk to the public API, through `cihub-client`; `cihub server` talks to Postgres and is described in `specs/DEPLOYMENT.md`; `cihub server token create --org <slug>` creates the first API token of an organization. `cihub login --url <address> [--token <token>]` keeps the address and the token in `$XDG_CONFIG_HOME/cihub/credentials.json` (mode 0600), and reads the token from the standard input without `--token`; `CIHUB_URL` and `CIHUB_TOKEN` take precedence over the file.

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

Every `cihub queue` command takes `--repo <owner/name>`, else the repository of the `origin` remote of git, and `--queue <name>`, else the only queue of the repository. The owner is the slug of the organization: an organization is the account of an installation of the App. `cihub login` does not check the token, because every route names an organization that it does not know: the first `cihub queue` command does. With two or more queues and no `--queue`, the command fails, lists the names and does nothing; with none, it fails and says that the first `/cihub merge` on a PR of the default branch creates the queue. `cihub queue targets upload` arrives at Lot 13.

The internal interfaces are not RPC: the processes share Postgres, so the "internal API" consists of the `jobs` table, the `events` log and the `NOTIFY` channels `cihub_queue`, `cihub_jobs`.

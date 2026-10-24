# APIs

The public API is a resource-oriented REST under `/api/v1`, on the address of the instance, next to the dashboard (`https://cihub.example.com/api/v1`); it has structured errors and a queue list endpoint; no route creates a queue, the first `/cihub merge` on a branch does. Authentication is by `Authorization: Bearer <token>` with API tokens owned by the organization (scopes `queues:read`, `queues:write`, `prs:write`) or by a session of the dashboard; the CLI uses the same API. A session is the cookie `__Host-cihub_session`; the role of its user in the organization of the URL gives its scopes (`member`: `queues:read` and `prs:write`; `admin`: every scope), and an organization where the user has no role answers `404`; on a route of a repository, the permission of the user on it gives them instead, and a repository that they do not read answers `404 repo_not_found` (`specs/SECURITY.md`). A request of a session that can change a state (any method but `GET`, `HEAD` and `OPTIONS`) also carries the header `X-CSRF-Token`, whose value the cookie `__Host-cihub_csrf` gives; without it, `403` with the code `csrf_refused`, which tells it apart from a missing scope. `GET /auth/github` and its callback, outside `/api/v1`, sign a person in and create the session (`specs/GITHUB.md`): `?next=` gives the page to open after, a path of the instance only; `GET /auth/github/installed`, the Setup URL of the App, starts the sign-in again after an install; `POST /auth/sign-out` deletes it and clears both cookies (`204`). A person signs the command line in with a device code (below): its session, `chc_…`, goes in `Authorization: Bearer`, its role gives its scopes as for the dashboard, and it needs no CSRF token.

Every route names its organization by its slug, then the repository by its name and the queue by its target
branch, percent-encoded (`release%2F2.0`), not by an identifier: `/orgs/{org}/repos/{repo}/queues/{queue}`. A
PR belongs to its repository, whose number names it: `/orgs/{org}/repos/{repo}/prs/{number}`; its base branch
gives its queue, as for a command in a comment. A branch names a queue only in its repository (`acme/web` and
`acme/api` each have `main`): no route names a queue without its repository. The URL thus says which organization it reads, so a link or two browser tabs never depend on a hidden
active organization. The token or the session must belong to the organization of the URL; otherwise the answer is
`404 org_not_found`, never `403`, so that a caller does not learn that another organization exists. The slug and the
name of the repository do not tell the case apart, as the SCM. Everything that belongs to a repository is under
it (its queues, its PRs, its runs); a route of the organization (`/orgs/{org}/queues`, `/orgs/{org}/prs`) only
searches across its repositories, and takes no filter that a route of a repository gives. An operation that the
methods of HTTP do not name is a `POST` on a segment that names it, under the resource that it acts on (`…/sync`,
`…/prs/{number}/submit`, `…/prs/{number}/cancel`, `…/prs/{number}/restart`). That segment never stands where an
identifier can: `/orgs/{org}/repos/sync` would hide a repository named `sync`. A cancellation is not a `DELETE`: the submission stays,
`cancelled`, and its PR still reads it. The paths below follow `/api/v1`; `…` stands for
`/orgs/{org}/repos/{repo}/queues/{queue}`, and `…pr` for `/orgs/{org}/repos/{repo}/prs/{number}`.

**Lists.** Every list gives a page: `limit` from 1 to 100 (50 by default; `400 limit_out_of_range` past it), and
`from`, the `cursor` of the previous page. The answer names its entries after the resource (`prs`, `queues`,
`repos`, `orgs`, `events`, `entries`) and adds `has_more`, `count`, how many entries the list has with its
filters, every page together, and `cursor`, the `from` of the next page (`null` on the last one). A cursor is opaque
to a client: base64url of the name of the list and the sort key of the last entry of the page (`Page` and
`cursor_of` in `cihub-platform`), so that it stays valid when the entry changes and a cursor of another list, or one
that does not read, answers `400 invalid_cursor`. A single resource is the object itself, without an envelope;
a resource has one form, the same in a list and alone, but for a repository: its list gives the verdict of each
setup (`Repo`), and the repository alone its whole setup (`RepoDetail`), whose file can weigh 64 KiB.

| Method             | Path                                                     | Role                                                                                                                                                                                                                                           |
| ------------------ | -------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| GET                | `/user`                                                  | The person of a session (`login`, `name`, `avatar_url`, `synced_at`: `null` until the first read of their repositories ends); a token stands for no person and answers `401`                                                                                         |
| GET                | `/user/orgs`                                             | The organizations of the person of a session, with their role, by slug (none before the App is installed on an account of theirs); a list; a token answers `401`                                                                                |
| DELETE             | `/user/session`                                          | Ends the session of the request, which signs out; `204`                                                                                                                                                                                         |
| POST               | `/user/sync`                                             | Reads again from the SCM now the installations and the repositories that the person of a session reaches, at most once a minute; `202`; a token answers `401` |
| POST               | `/device-codes`                                          | Starts a sign-in of the command line: a `DeviceCode` (`device_code`, `user_code`, `verification_url`, `expires_in`, `interval`); no token or session; `503 sign_in_unavailable` when nobody can sign in to the instance                         |
| POST               | `/device-codes/token`                                    | Body `{device_code}`: the session of the command line once the person approved the code, a `CliSession` (`token`, `login`); before, `400 device_code_pending`, `429 polling_too_fast`, `403 device_code_denied` or `400 device_code_expired`; no token or session |
| POST               | `/device-codes/{user_code}/approve`, `…/deny`            | The person of a session of the dashboard approves, or denies, the code that the command line shows; `204`, or `404 device_code_not_found`; a session of the dashboard only                                                                    |
| GET                | `/instance`                                              | The version of the instance and the page of the SCM that installs the App (`null` when the SCM did not answer); any token or session                                                                                                           |
| GET                | `/orgs/{org}`                                            | The organization (`slug`), the `role` of the person of the session in it, `null` for a token, and `is_suspended` (every connection of the organization is suspended); any token or session of the organization            |
| GET                | `/orgs/{org}/repos`                                      | The repositories that the App reaches, by namespace then name, each a `Repo`; filters `state` (states of the setup, separated by commas; an unknown state answers `400 invalid_request`) and `q` (a part of the name, whatever its case, 1 to 100 characters); `queues:read` |
| GET                | `/orgs/{org}/repos/{repo}`                               | The repository with its whole setup, a `RepoDetail`; `404 repo_not_found`; `queues:read`                                                                                                                                                      |
| POST               | `/orgs/{org}/repos/{repo}/sync`                          | Admin: read the setup of the repository from the SCM now (class `interactive`, at most once a minute); `202`; `queues:write`                                                                   |
| GET                | `/orgs/{org}/queues`                                     | The queues of every repository of the organization, by repository name then target branch, each a `Queue`; filter `q` (a part of the name of the repository, whatever its case, 1 to 100 characters)                                         |
| GET                | `/orgs/{org}/repos/{repo}/queues`                        | The queues of the repository, by target branch, each a `Queue`                                                                                                                                                                                 |
| GET, PATCH         | `…`                                                      | Read a `Queue`, or request a `state` (`running`, `paused`, `draining`); deletion is deferred                                                                                                                                                    |
| POST               | `…/sync`                                                 | Admin: re-read from the SCM each live PR, each run, the head of the target and its configuration                                                                                                                                               |
| GET                | `…/graph`                                                | The merge graph for the dashboard: the head of the target branch, the live items as nodes, the edges from a predecessor to an item that its test merges after it, and the last run of each live group (`run_id`, `item_ids`, `predecessor_ids`, `attempt`, `status`, `started_at`), the oldest first |
| GET                | `…/health`                                               | The Health of the queue over `range` (`24h` by default, `7d`, `30d`), or over the UTC hours from `from` to `to` (RFC 3339, together, not with `range`, starting in the last 13 months, ending at the latest with the current hour; `invalid_request` otherwise), a `QueueHealth`: the verdict (`quiet`, `healthy`, `many_failures`, `check_passes_on_retry` with the check and its count), the PRs merged, failed and cancelled and their time to merge against the period before, the buckets of the period (hours up to 2 days, UTC days up to 90 days, weeks from a Monday beyond), the median time in each state of the merged PRs (the medians and the 90th percentiles within 5 %), the reasons of the others with their conclusion, and per check its runs, its failures and its passes on a retry; read from the hourly aggregate of `cihub-analytics`, a few seconds after the log (`specs/ANALYTICS.md`) |
| GET                | `/orgs/{org}/prs`, `/orgs/{org}/repos/{repo}/prs`        | List PRs: the current submission of each PR in each queue of the organization, or of the repository, each a `Pr`; filters `author` (a login whatever its case; the dashboard gives the login of its person), `state` (one state or several, separated by commas; an unknown state answers `400 invalid_request`), `submitted_since` and `submitted_until`, `state_changed_since`, and under a repository `queue` (its target branch); `sort` `newest` (by default) or `position` (by repository, queue, then position; then the PRs not admitted yet, then the ended ones, which can keep a position). Without a filter, every PR, live or ended |
| GET                | `…pr`                                                    | The PR: its last submission to a queue of the repository, a `Pr`; `404 pr_not_found` for a PR never submitted                                                                                                                                  |
| GET                | `…pr/events`                                             | The history of the PR, every submission together, oldest first; a list of `Event`                                                                                                                                                             |
| POST               | `…pr/submit`                                             | Submit the PR to the queue of its base branch: `priority` (name or 0–255), `no_batch`, `profile` (a profile of that queue, else its `conditions` decide), `direct`; a PR already in a queue answers `409 pr_already_queued` at once, the refusals that need a read of the SCM (among them a queue that accepts nothing, `draining` or `switching_modes`) come later on the PR. A command or a label on a PR already in the queue waits until its item ends, then submits it again |
| POST               | `…pr/cancel`, `…pr/restart`                              | Cancel, or restart the tests of, the current submission of the PR                                                                                                                                                                              |
| GET                | `/orgs/{org}/repos/{repo}/runs/{run_id}`                 | A `Run` (format below, frozen at Lot 6): branch, SHA, all the observed check runs and statuses with the required ones marked, tested PRs, impacted targets, dependents                                                                         |
| GET                | `/orgs/{org}/stream`                                     | Server-sent events of the queues of the organization: `queue_changed` (`QueueChanged`: `queue_id` and `seq`, the last sequence number of its log) when what the pages of a queue show changed, a ping comment every 15 seconds, `retry: 1000`; the stream ends after 5 minutes, when the server stops, lost changes or the page lags, and the browser opens it again and reads everything; at most 8 streams per token or person (`429 too_many_streams`); `queues:read` |
| GET                | `/orgs/{org}/audit`                                      | Setting changes and admin actions, newest first; a list                                                                                                                                                                                        |
| GET                | `…/test-results`                                         | One row per PR and per attempt over a range (≤ 45 days): `outcome` (`passed`, `failed`, `interrupted`, `cancelled`), `cycle_conclusion`, `bisection` (`entered_bisection`, `in_bisection`), `run_id`; a list                                   |
| POST               | `/orgs/{org}/repos/{repo}/prs/{number}/impacted-targets` | Body `{head_sha, targets}` with a list or `"ALL"`; 20 MB limit; also accepts a fork workflow identifier instead of a token                                                                                                                     |
| GET                | `/metrics`                                               | Prometheus exposition, filters `repo`, `branch`                                                                                                                                                                                                |
| GET, POST, DELETE  | `/orgs/{org}/webhooks`                                   | Manage the outgoing webhook endpoints, test a delivery                                                                                                                                                                                         |

A queue (`Queue`) carries its `repo`, its `target_branch` and `is_default_branch` (the branch is the default branch
of the repository), its `state` and `mode`, `state_changed_at` (its creation before a
first change) and `state_changed_by`, the login of the person who asked for the state (`null` when CIHub decided it
or a token asked for it); the `strategy` of its last valid file; `target_head_sha`, the head of the target branch
that the log saw last; `is_suspended`, when the installation of the App is suspended or removed, or no longer lists the repository;
`has_config_file`, `config_sha` and `config_error`; its counters: `counts` (the current items in each state that is
not terminal), `merged_last_24h`, `failed_last_24h` and `failed_or_cancelled_last_24h` (the current items that reached
`merged`, `failed`, or `failed` or `cancelled`, in the 24 hours before the request, not in a calendar day: the ones
that the list of the PRs gives; a PR that failed, then came back, counts only its current item), `last_merged_at`, `live_runs`, `median_run_seconds` (the median duration of the runs that succeeded in the last 24
hours, `null` without one); the `window` with its `current`, `floor` and `ceiling`; `settings`: `batching_enabled`,
`batching_size`, `batching_max_wait_minutes`, `failure_budget_retries`, `merge_method`, `enqueue_label` (the label
that submits a PR, `null` when the labels do not submit); and `setup` (`Setup`), what the rules of the SCM missed at
its last setup read (`findings`, and `checked_at`, `null` before the first; no finding means sound rules). Every field comes from Postgres: showing a queue calls the SCM zero times.

**Vocabulary of the SCM reads.** A *sync* copies an object of the SCM into CIHub (installations, repositories,
members, a setup); its jobs run by themselves, and `POST …/sync` asks for one now (`…/sync` of a queue,
`/orgs/{org}/repos/{repo}/sync`, `/user/sync`). A *setup* is what CIHub found about a repository, the rules of its
default branch and its configuration file, with a verdict. A *check* is only a check of the CI. No route says
"check" or "readiness".

A repository (`Repo`) carries its `id`, `namespace`, `name` and `default_branch`; its `permission`, what the caller
may do on it (`read` its queues, its PRs and its setup; `write`: submit, cancel and restart a PR; `admin`: change a
queue and ask for a sync; for a person, from their permission on the repository, and for a token, from its
scopes); and `setup`, a `SetupSummary` (`state`, `checked_at`, `requested_at`). A `RepoDetail` carries the same
fields, with the whole setup in `setup`.

The setup of a repository (`RepoSetup`) carries its `state` (`SetupState`), which the server computes in this
order: `started` (the default branch has its queue, whose target branch `queue` gives; `null` otherwise),
`checking` (no read yet, or a sync asked after the last read),
`invalid_config`, `cannot_merge` (a rule that blocks the merges fails, or `target_branch_missing`),
`ready_with_warnings` (a rule fails without blocking them), `ready`; `rules`, always the four rules of the SCM that
a queue needs, in the order of the findings (`update_restriction_missing`, `app_not_exempt`,
`test_branches_not_excluded`, `up_to_date_required`), each a `RuleCheck` (`code`, `passed`, `blocks_merge` for
`app_not_exempt` and `test_branches_not_excluded`, `title`, the sentence of the rule, and `message`, what to change
when it fails), empty before the first read; `target_branch_missing`; `rules_url`, the page of the SCM that changes the rules (`null` when the SCM
has none); `config`, `null` before the first read (`ConfigFile`: `state` `none`, `valid` or `invalid`, `path`, `sha` the head that was read,
`error` with its `line` and `column`, `text` only when invalid and at most 64 KiB, `entries` the `branch` of each
entry of the file as written (none for a file that does not parse), and the `strategy` and the `settings` of the default branch, from the file or the
defaults); `checked_at`, `null` before the first read; and `requested_at`, the time of a sync while its read
waits. A queue shows the same findings in `Queue.setup`. Every field comes from Postgres: showing a setup calls the
SCM zero times. The job `read_repo_setup` reads it at the discovery of a repository, then once a day at a
moment of the day that the repository keeps (class `repair`), and at `POST …/repos/{repo}/sync` (class
`interactive`, at most once a minute), which writes `repo.sync` in the audit log; a page reads the setup again
while `requested_at` shows.

A PR (`Pr` in `cihub-api-types`) is its last submission (an item of `specs/DOMAIN.md`), with `id`, the identifier
of the item, its `number`, `repo` and `queue`. It carries `state_reason`, the code of the closed vocabulary, and
`state_description`, its sentence for a person (`Reason::describe`, `specs/DOMAIN.md`). A client shows the
sentence and decides on the code: the sentence can change without notice. It also carries the `title` of the PR at
its last read (`pr_titles`), its `url`, the page of the PR on the SCM at its last read (`queue_items.pr_url`, from
`pr.observed`; `null` before the first), its `author`, a `Person` (`login`, and `avatar_url`, `null` until a sign-in or the members of an account give
it), the `run_id` of the last run of its group, `no_batch`, `profile` (`null` for the settings of the queue), `submitted_via`,
`submitted_by`, the login of the person who submitted it (`null` for CIHub or a token), `ahead`, the admitted PRs
that merge before it (number and state), the head of the queue first, empty for a merged PR, and
`merges_in_seconds`, an estimate from the median duration of the runs and the window of its queue. An event of the
history of a PR (`Event`) carries its `type`, its `item_id` and `payload`, `actor`, the login of the person who
acted or whose request caused the decision (`null` for CIHub or a token), and `description`, its sentence for a
person, where a check or a condition is a Markdown code span. `…pr/cancel` and `…pr/restart` write
`item.cancel_requested` and `item.restart_requested` in the audit log.

**`GET /orgs/{org}/repos/{repo}/runs/{run_id}`** returns a JSON object whose fields are the columns of `test_runs` and of
`check_observations` (`specs/DATA_MODEL.md`), with the names of the closed vocabulary: `id`, `queue_id`, `group_id`,
`attempt`, `status`, `status_reason`, `test_branch`, `test_sha`, `base_sha`, `draft_pr_number`, `draft_pr_url`
(the page of the draft PR on the SCM, built by `Scm::pr_url`; `null` without a draft PR; unlike `Pr.url`, which is the page at the last read of the PR, it is built when the run is read), `tested_pr_numbers`, `head_shas` (the merged heads in order: predecessors then members),
`predecessor_pr_numbers`, `required_statuses`, `required_statuses_source`, `started_at`, `ended_at`,
`last_report_at`, `failure_reason`, `failure_causing_statuses`, `conflict_culprit` (`{kind: queue_pr, pr_number}`
or `{kind: base_branch}`), `checks` (one entry per observed check: `name`, `kind`, `required`, `status`, `conclusion`,
`details_url`, `started_at`, `completed_at`, `observed_at`), `impacted_targets` (`all` or the list, Lot 13) and `dependents` (the live runs
whose predecessors contain a tested PR: `run_id`, `pr_numbers`). An absent value is
`null`.

Errors are in `application/problem+json` with `type`, `title`, `status`, `detail` and a machine `code`. The codes are closed (`ErrorCode` in `cihub-api-types`), and each one names the cause, so that a client decides on it and never on `detail` (principle VI); each code has one status: `400` `invalid_request` (a body, a path or a query that does not parse), `limit_out_of_range`, `invalid_cursor`, `unknown_priority`, `device_code_pending`, `device_code_expired`; `401` `unauthorized`; `403` `scope_missing`, `csrf_refused`, `device_code_denied`; `404` `route_not_found`, `org_not_found`, `repo_not_found`, `queue_not_found`, `pr_not_found` (the PR was never submitted to a queue of the repository), `run_not_found` (a run of another repository too), `device_code_not_found`; `409` `pr_already_queued`, `item_ended` (a cancellation of an item that ended), `item_not_in_test` (a restart outside `testing` and `pending_failure`); `429` `rate_limited`, `too_many_streams`, `polling_too_fast`; `500` `internal`; `503` `sign_in_unavailable`, `scm_unavailable` (the SCM did not answer the check of the access of the person of a session). Rate limit: 600 requests per minute and per token or session, in each replica of the server, `Retry-After` on 429s.

The OpenAPI document is the reference of the API, written for its readers: its introduction (`crates/web/src/openapi.md`, followed by the table of every code) says how to authenticate, page, and read an error; each route has a tag (`Session`, `Organizations`, `Live updates`, `Queues`, `Pull requests`, `Runs`, `Repositories`), a short title then a description, which ends with its scope, and every response that it can give, each error with an example of each of its codes. A route declares its own errors (an `IntoResponses` of `cihub-platform::error_responses`); `cihub-web` adds those of every route of its kind: `401`, `429` and `500` everywhere, `404 org_not_found` under `/orgs/{org}`, `403 scope_missing` with a scope, `403 csrf_refused` on a method that changes a state. Every field has a description and an example; the closed enums of the domain are named schemas (`ItemState`, `Reason`, …) with their values. Its `servers` give `/api/v1`, which the client of the dashboard uses, then `{base_url}/api/v1` for a reader outside the instance. `crates/server/tests/openapi.rs` fails when a route lacks a tag, a title, a description or an error, or a field lacks a description. A submission, a cancellation, a restart, a state request and a re-read respond `202` without a body: the queue applies them at its next reconciliation. The OpenAPI 3.1 document is generated from the code (`cihub-web`, `cihub-api-types` and the routes of the modules) and published without a token on `/api/v1/openapi.json`; the TypeScript client of the dashboard is generated from it (`web/openapi.json`, `@hey-api/openapi-ts`; `mise run web:generate-check` fails when it is not up to date), and the Rust client `cihub-client` shares the types of `cihub-api-types` directly. Deferred: `GET …/test-results` (Lot 11), `impacted-targets` (Lot 13), `/metrics` and `/orgs/{org}/webhooks` (Lot 11).

Outgoing webhooks follow the [Standard Webhooks](https://www.standardwebhooks.com) specification (headers `webhook-id`, `webhook-timestamp`, `webhook-signature` in HMAC-SHA256), are retried with exponential backoff over 24 hours and disabled after 7 days of failures. The event types, the payload fields and the reasons follow CIHub's vocabulary (`specs/DOMAIN.md`), without translation to that of another product; their list is frozen at Lot 11. Per-PR events are emitted for each member of a batch.

The command line is the `cihub` binary (crate `cihub-cli`), which a person or a CI installs from the binaries of each release (`specs/DEPLOYMENT.md`). `cihub auth` and `cihub queue` only talk to the public API, through `cihub-client`; the binary holds no server code. `cihub app create` talks to GitHub alone, through `cihub-github`, as the instance does not run yet: `cihub app create --public-url <url> --name <name> [--org <login>] [--github-url <url>] [--port 3999] [--out .]` creates the GitHub App of an instance from a manifest (`specs/GITHUB.md`) on a page that it serves on `localhost`, and writes `github-app.env` (the `CIHUB_GITHUB_*` variables of the App) and `github-app.private-key.pem` in `--out`, readable by their owner only, and never over an existing file. The `cihub-server` binary talks to Postgres and is described in `specs/DEPLOYMENT.md`; `cihub-server token create --org <slug>` creates an API token of an organization, for a CI.

**Sign-in of the command line.** It follows the device authorization grant (RFC 8628), on top of the sign-in of the dashboard. `cihub auth login` asks for a code (`POST /device-codes`) and shows the address of the page `/device` of the dashboard and, on its own line, the code (`WDJB-MJHT`, 8 letters out of 20, without vowels, so that no code spells a word), valid 15 minutes. The address never carries the code. The person opens the page on any machine, signs in to the dashboard when needed, types the code in its 8 boxes, and approves it or denies it: the person types the code, so that a link sent by someone else does not sign their command line in as the person with one click. Meanwhile the command line asks `POST /device-codes/token` every `interval` seconds (5), and 5 seconds more after each `429 polling_too_fast`; once the person approved, the answer gives the session once and the code is used. A session of the command line covers every organization of its person, as a session of the dashboard, and acts as that person: a submission carries their account, and the job checks their permission on the repository. It ends after 90 days without use (each use pushes the end back, at most once an hour), or at `cihub auth logout`.

| Command                                          | Role                                                                                                   |
| ------------------------------------------------ | ------------------------------------------------------------------------------------------------------ |
| `cihub auth login [--hostname <host>]`           | signs the command line in with a device code, then keeps the session of the host                      |
| `cihub auth logout [--hostname <host>]`          | ends the session on the server, then forgets it                                                        |
| `cihub auth status [--hostname <host>] [--show-token]` | the hosts, who is signed in on each and where the token comes from; exits with 1 when a token does not work |
| `cihub auth token [--hostname <host>]`           | prints the token of the host, for a script                                                             |

The host is `--hostname`, else `CIHUB_HOST`, else the only host that the command line knows; there is no default host. The address of a host is `https://<host>`, or `http://` for a loopback host (`localhost`, `127.0.0.1`, `[::1]`, with a port or not). The command line keeps a session per host in `$XDG_CONFIG_HOME/cihub/credentials.json` (mode 0600). `CIHUB_TOKEN` takes precedence over the file: a CI sets `CIHUB_HOST` and `CIHUB_TOKEN`, with an API token of the organization.

| Command                                                  | Role                                                     | Scope          |
| -------------------------------------------------------- | -------------------------------------------------------- | -------------- |
| `cihub queue merge <pr> [-p urgent\|high\|medium\|low\|0-255] [--no-batch] [--profile <name>]` | submits a PR to the queue of its base branch | `prs:write`    |
| `cihub queue cancel <pr>`                                | takes the PR out of its queue                            | `prs:write`    |
| `cihub queue restart <pr>`                               | tests the PR again, in its queue                         | `prs:write`    |
| `cihub queue status [<pr>]`                              | the state of the queue, or of a PR in its queue          | `queues:read`  |
| `cihub queue graph`                                      | the merge graph of the queue                             | `queues:read`  |
| `cihub queue pause`                                      | stops the tests and the merges; the queue keeps its order | `queues:write` |
| `cihub queue resume`                                     | starts the queue again                                   | `queues:write` |
| `cihub queue drain`                                      | finishes the items in the queue and refuses new ones     | `queues:write` |

Every `cihub queue` command takes `--repo <owner/name>`, else the repository of the `origin` remote of git, and the queue commands (`status`, `graph`, `pause`, `resume`, `drain`) take `--branch <branch>`, else the queue of the default branch of the repository, else its only queue. The owner is the slug of the organization: an organization is the account of an installation of the App. The host is the one of `cihub auth`. With two or more queues, none of them on the default branch, and no `--branch`, the command fails, lists the
branches and does nothing; with none, it fails and says that the first `/cihub merge` on a PR of the default branch creates the queue. `cihub queue targets upload` arrives at Lot 13.

The internal interfaces are not RPC: the processes share Postgres, so the "internal API" consists of the `jobs` table, the `events` log and the `NOTIFY` channels `cihub_queue` (a queue has work), `cihub_jobs`, and `cihub_queue_changed` (`<org> <queue> <seq>`: what the pages of a queue show changed, sent in the transaction that changed it, which each web process listens to once for the streams of its pages).

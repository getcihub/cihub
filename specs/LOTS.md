# Phases and lots

Context: whoever starts a piece of work. This file splits the roadmap into lots of about one week for one
developer who drives coding agents. Each lot has a goal, a content, the interfaces it **freezes** (the following
lots build on them without changing them), its dependencies and an **automated exit criterion** that is the
definition of "done". A lot starts only when its dependencies are done and when the open questions that it
lists are settled.

**Tracking.** The status of a lot is `to do`, `in progress`, `done` or `blocked` (with the cause).
As soon as a lot starts, it receives a "Tracking" block: what is done (crates, points of the content), what remains,
and the deviations from the plan, each validated by the project lead. The status and the block are updated in the
same change as the code.

## Phases

Four phases, each deliverable on its own and completed by the use of CIHub on its own repository. The graph-based
reconciler is built from phase 1 in its general form, so that the following phases add strategies and modules, not
rewrites.

| Phase                       | Lots            | Scope                                                                                                                                                                                                                                                                                                                                                             | Exit criterion                                                                                                                                                                |
| --------------------------- | --------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1. Installable merge queue  | 0 to 9          | Foundation (tenancy, log, jobs, GitHub adapter), pure core, reconciler, complete test cycle, entry and merge conditions, adaptive window, failure budget, `sequential` and `pipelined` strategies, squash / merge / rebase / fast-forward methods, direct merge, PR surfaces, API and CLI, `docker compose` and procedure to create the App                       | The CIHub repository merges through CIHub for 2 weeks without a red merge; an outside person installs and merges a first PR in less than 30 minutes by following `INSTALL.md`; measured in lot 15 |
| 2. Throughput and visibility | 10 to 12, 16   | Dashboard, onboarding (the Setup URL of the App, the setup of a repository before its first queue), Postgres analytics, Health view, Prometheus, outgoing webhooks, `batched` strategy (batches, bisection, failure depth, optimistic merge)                                                                                                                                                                                                                 | At 20 PRs per hour on the simulator, `batched` uses 30 % fewer runs per merged PR than `pipelined`; bisection isolates a planted failure in ≤ log2(batch) runs |
| 3. Flaky tests              | 14 (advanced)   | OTLP receiver, JUnit converter and GitHub action, `ft_*` tables, detection, quarantine, `FlakeOracle` trait consumed by the queue, dedicated page                                                                                                                                                                                                                | The module is added without changing the merge queue outside the call to the trait; on a repository with a known flaky test, zero PR evicted wrongly                        |
| 4. Scale and ecosystem      | 13, 15 and next | `parallel` strategy with impacted targets, Bazel and Nx actions, ClickHouse behind `AnalyticsStore`, hardening, then stacked PRs, direct-to-queue, Slack, Terraform, GitLab behind `Scm`                                                                                                                                                                            | A pilot monorepo merges disjoint PRs in parallel; an instance holds 1,000 PRs per day with ClickHouse enabled; first outside organization in production                     |

The two highest risks are the invariants of the reconciler under concurrent GitHub events (mitigated by the pure
core and its property tests, delivered first) and the GitHub rulesets misconfigured by the users (mitigated by the
configuration checker, in phase 1).

## Overview

| Lot | Title                                                  | Phase | Depends on | Questions | Status                                   |
| --- | ------------------------------------------------------ | ----- | ---------- | --------- | ---------------------------------------- |
| 0   | Repository foundations                                 | 1     | —          | —         | done                                     |
| 1   | Pure core of the queue                                 | 1     | 0          | —         | done                                     |
| 2   | Data foundation: server, tenancy, log                  | 1     | 0          | —         | done                                     |
| 3   | Internal transport: inbox, jobs, leases                | 1     | 2          | —         | done                                     |
| 4   | GitHub adapter, simulator and App installation         | 1     | 3          | —         | done                                     |
| 5   | Reconciler and admission                               | 1     | 1, 4       | —         | done                                     |
| 6   | Test cycle                                             | 1     | 5          | —         | done                                     |
| 7   | Merge and outside changes                              | 1     | 6          | —         | done                                     |
| 8   | Queue policy                                           | 1     | 6          | —         | done                                     |
| 9   | Surfaces: PR, API, CLI                                 | 1     | 7, 8       | —         | done                                     |
| 10a | Dashboard: session, roles and frame                    | 2     | 9          | —         | done                                     |
| 10b | Dashboard: queues and the page of a queue              | 2     | 10a        | —         | done                                     |
| 10c | Dashboard: your pull requests, drawer and actions      | 2     | 10b        | —         | in progress (waits for a recording)      |
| 10d | Dashboard: live updates                                | 2     | 10b        | —         | done                                     |
| 11a | Analytics and the Health view                          | 2     | 9          | —         | in progress                              |
| 11b | Prometheus                                             | 2     | 11a        | —         | to do                                    |
| 11c | Outgoing webhooks                                      | 2     | 11a        | —         | to do                                    |
| 12  | `batched` strategy                                     | 2     | 9          | —         | done                                     |
| 13  | `parallel` strategy                                    | 4     | 12         | —         | to do                                    |
| 14  | `flakytests` module                                    | 3     | 9          | Q1, Q7, Q12 | to do                                  |
| 15  | Scale: ClickHouse, hardening                           | 4     | 11b, 13    | Q8        | to do                                    |
| 16  | Onboarding and the GitHub App                          | 2     | 10a, 10b   | —         | to do                                    |

```mermaid
flowchart LR
  L0[0 Foundations] --> L1[1 Pure core]
  L0 --> L2[2 Data foundation]
  L2 --> L3[3 Internal transport]
  L3 --> L4[4 GitHub]
  L1 --> L5[5 Reconciler]
  L4 --> L5
  L5 --> L6[6 Test cycle]
  L6 --> L7[7 Merge]
  L6 --> L8[8 Policy]
  L7 --> L9[9 Surfaces]
  L8 --> L9
  L9 --> L10a[10a Session]
  L10a --> L10b[10b Queues]
  L10b --> L10c[10c PR and actions]
  L10b --> L10d[10d Live]
  L9 --> L11a[11a Analytics]
  L11a --> L11b[11b Prometheus]
  L11a --> L11c[11c Webhooks]
  L9 --> L12[12 batched]
  L9 --> L14[14 flakytests]
  L12 --> L13[13 parallel]
  L11b --> L15[15 Scale]
  L13 --> L15
  L10b --> L16[16 Onboarding]
```

Lots 1 and 2 can advance in parallel after lot 0, as can lots 7 and 8 after lot 6, lots 10a, 11a, 12 and 14
after lot 9, lots 10c and 10d after lot 10b, and lots 11b and 11c after lot 11a.

---

## Phase 1: installable merge queue

Lots 0 to 8 are done; the git history keeps their tracking.

### Lot 9: surfaces: PR, API, CLI

- **Goal**: let humans and the CI use the queue; end of phase 1.
- **Content**: `CIHub Merge Queue (<branch>)` status, bot comment edited in place, state labels;
  `/cihub merge|cancel|restart` commands, with permission checks; REST API v1
  of `specs/API.md` (without metrics and outgoing webhooks), `problem+json` errors, rate limit, OpenAPI 3.1
  generated from the code (`cihub-web`, `cihub-api-types`); Rust client `cihub-client`; `cihub` CLI;
  `Dockerfile` of the `cihub-server` image and workflow that publishes the image to GHCR at each version; installation `docker compose` with the `cihub-migrate` service and
  installation README, with a prompt that helps write `.cihub.yaml`; configuration checker as an API endpoint, with the rulesets read through the
  `Scm` trait; block reason that names the missing required check; 5-minute
  permission cache and message of a refused submission.
- **Frozen interfaces**: API v1 published in `/api/v1/openapi.json`; PR commands; CLI commands.
- **Exit criterion**: an end-to-end test on the staging instance submits PRs on a real repository and merges
  them; the CIHub repository moves to its own queue; an outside person follows `INSTALL.md` and merges a first
  PR in less than 30 minutes (criterion of phase 1, measured by hand).
- **Tracking**:
  - Steps: 1. start of an organization; 2. PR surfaces; 3. API v1; 4. client and CLI;
    5. configuration checker; 6. image, installation `docker compose` and README; 7. exit criteria.
  - Done (October 2, 2026): step 1. An admin command reads the account
    of the installation, creates the organization (slug: the login of the account), its account and its connection,
    requests `sync_repos` and shows once a token with
    every scope; on an installation already known, it creates only a new token. Replaced on October 9, 2026 by the
    sync of the installations and `cihub server token create --org <slug>`. The
    `Scm` trait gains `installation` and `list_repos` (GitHub and simulator, contract suite). The `sync_repos` job
    creates and updates `repos` without deleting any and without taking the repository of another organization; it is
    requested by the routing of each `installation_repositories` delivery (key of the delivery), and by the admin
    command and each scheduler replica (key of the hour). Tests: against Postgres, the admin command and its rerun, the reading of
    the repositories, the hourly request and the routing; the two GitHub routes on hand-written responses.
  - Done (October 2, 2026): the GitHub responses are recorded again (64 exchanges, with the installation and
    its repositories), with the `cihub` ruleset put back in recording mode for the duration of the recording;
    #55 merged afterwards. The recorder keeps only the repository of the suite in the list of the repositories of
    the installation, because the others may be private and the cassette is public.
  - Done (October 2, 2026): step 2. `plan` gives `Desired.surfaces` (each live item, and each item
    that the reconciliation terminates, seen before its eviction); `cihub-mergequeue` turns it into the queue status,
    the comment and the state labels, writes the desired rendering in `pr_surfaces` and requests `publish_surfaces`,
    which publishes under an advisory lock of the PR; `merge_pr` publishes `success` before the merge. `/cihub cancel`
    and `/cihub restart` go through the `pr_command` job; a refused command receives a comment. `cihub-github`
    keeps a permission for 5 minutes. `pr.observed` carries `required_checks`, and a `blocked` PR names its required
    checks that fail or are pending. Tests: rendering (status, comment, labels, cut at 140 characters), surfaces in
    the core (position, end of an evicted item, named reasons), and against Postgres and the simulator a single
    comment, the status and the labels, then restart, cancellation and refusal of a command.
  - Done (October 2, 2026): step 3. Crate `cihub-api-types` (requests, responses, `ErrorCode`,
    `Problem`, OpenAPI schemas behind the `openapi` feature, `utoipa` dependency). `cihub-platform` carries the
    `problem+json` error, `ApiGrant` and the extractors; the `Module` contract gains `ModuleApi` (routes and document).
    `cihub-web` authenticates `/api/v1` by token, limits each token to 600 requests per minute and per replica,
    serves `/repos`, `/orgs/{org}/audit` and `/api/v1/openapi.json` (without token). `cihub-mergequeue` serves the
    queue routes of `specs/API.md`; `audit_log` gains `actor_token_id`, and the admin command audits its token. Tests: against
    Postgres, the authentication, the scopes, the limit and the document; and end to end on the simulator, a
    submission through the API, the item, its timeline, its run, the graph, restart, cancellation, pause, new
    queue, resync and audit log.
  - Done (October 2, 2026): step 4. `cihub-client` (the methods that the CLI uses) and `cihub-cli` (`cihub` binary,
    `clap` dependency). Tests: git remote, choice of the queue, and the client against the real server in the
    end-to-end test of the API.
  - Done (October 2, 2026): step 5. The `Scm` trait gains `check_setup` (neutral findings); GitHub reads the
    rulesets of the target branch and of the test branches, and the simulator restricts no update;
    `GET /queues/{id}/config-check`. Tests: the contract suite, a hand-written response, and the route.
  - Done (October 2, 2026): step 6. `Dockerfile` (distroless, pinned by digest), `.github/workflows/image.yaml`
    (GHCR, one native runner per architecture, then a manifest), `deploy/compose/compose.yaml` and `deploy/compose/.env.example`,
    `README.md` with the installation procedure and the help prompt for `.cihub.yaml`, `release` profile. Verified
    by the agent: image built (69.6 MB), `migrate` in the image against Postgres, `docker compose config`,
    `actionlint` and `zizmor`.
  - Done (October 2, 2026): welcome comment. `PrChanged` gains `opened`; the `welcome_pr` job posts the
    welcome comment once, with its submit box; `pr_comments` keeps the comment of the PR, shared by its queues,
    and `publish_surfaces` takes it over. The adapter translates the checked box into `SubmitBoxChecked`, which the
    connection pass submits if the comment is the one of CIHub. The box comes back for a `failed` or `cancelled`
    item. Tests: translation of the opening and of the box, and against Postgres and the simulator the welcome,
    the submission by the box, its return after a cancellation, the refusal of an account without `write` and a
    box in the comment of someone else.
  - Done (October 4, 2026): sentence of each reason. `Reason::describe` in `cihub-domain`; the status and the
    comment of a `failed` or `cancelled` PR show it instead of the code, the API `Item` gains `state_description`
    and the CLI shows it. Tests: the table (one short, unique sentence per reason), the rendering of a failure, and
    the field against Postgres.
  - Done (October 3, 2026): one binary, `cihub`. `cihub-cli` (`crates/cli/`, the former
    `crates/server/`) gives it, with `clap`: one file per command and one per role; the HTTP endpoints stay in
    `cihub-web`. The former client-only `cihub-cli` crate is removed. Groups `login`, `queue` (`--repo`, `--queue`; with two queues and no `--queue`, the command refuses and
    lists the names) and `server` (`start`, `migrate`, the admin command of step 1, `--config
    <file>`). `CIHUB_ROLES` is removed and `CIHUB_LOG` becomes `CIHUB_LOG_FILTER`; the YAML file of the server has one
    key per variable. Image: entry point `cihub`, default command `server start`. Tests: table tests of the
    configuration (file only, variables only, a variable over the file, unknown key, wrong type, missing database,
    rules of the `github` keys), the refused arguments, and against Postgres `cihub server start|migrate` and
    `cihub queue drain|status` against a running server.
  - Done (October 5, 2026): `cihub server start` takes no argument and always runs the four roles; a
    deployment scales by identical replicas. The probes give only the name and the version of the binary.
  - Done (October 14, 2026), decided by the project lead: two binaries instead of one, so that the command line
    of a CI does not carry the server (`specs/LESSONS.md`). `cihub` (`cihub-cli`) keeps `login` and `queue` and
    depends on no server crate; `cihub-server` (`crates/server/`) takes `start`, `migrate` and `token create`,
    the roles, the configuration file, the examples and the tests against Postgres. The image becomes
    `ghcr.io/getcihub/server`, entry point `cihub-server`, default command `start`; the chart and compose
    follow. Tests: the refused arguments of each binary, and against Postgres the queue commands of `cihub`
    against a `cihub-server start`.
  - Done (October 14, 2026), decided by the project lead: `cihub auth login|logout|status|token`, on the model of
    the groups of other command lines, without `refresh` (a session has no scopes of its own: the role of its person
    gives them). `cihub auth login` follows the device authorization grant (RFC 8628) on top of the sign-in of the
    dashboard: `POST /device-codes`, the page `/device` where the person types the code, then approves or denies it,
    then `POST /device-codes/token`, which gives a session of the command line (`chc_`, in `sessions`, 90 days after
    its last use). The command line acts as its person; `cihub-server token create` stays for a CI. Hosts:
    `--hostname`, `CIHUB_HOST`, else the only known host, with no default host until an instance run by the project
    exists; `CIHUB_URL` is removed. Tests: the user codes in `cihub-domain`, the device codes and the sliding end of
    a session against Postgres, the routes end to end in `cihub-web`, `cihub auth login` against
    `cihub-server start` with a code approved through the API, and the page `/device` in Vitest.
  - Done (October 4, 2026): no queue is created by hand. The first `/cihub merge` on a branch without a
    queue creates, in `submit_pr`, the queues that `.cihub.yaml` names on the branch, or the queue of the default
    branch named after it, with the file that it read; a queue without an entry has `defaults`; a queue that the
    file no longer names takes no new submission. `POST /repos/{owner}/{name}/queues`, `cihub queue create`,
    `CreateQueue` and `queue_exists` are removed; the README no longer has a step that creates a queue. Tests:
    `defaults` alone in `cihub-config`, the rule of the queues that take submissions, and against Postgres and
    the simulator the queue of the default branch, the queues of the file and their routing, a label and a
    command on another branch that create nothing.
  - Done (October 15, 2026), decided by the project lead: one queue per target branch (`specs/LESSONS.md`). Two
    queues on one branch restarted the runs of each other at each merge. `queues.name` is removed, and
    (`repo_id`, `target_branch`) is unique; a route names a queue by its branch, and the API `Queue` loses `name`.
    `route_conditions`, the `queue-name` attribute, `/cihub merge --queue=`, the `queue` of `SubmitPr` and
    `409 queue_not_accepting` are removed: a PR enters the queue of its base branch, and a draining queue refuses it
    on the PR. The CLI option `--queue` becomes `--branch`, and `cihub queue merge|cancel|restart` need no queue.
    Tests: one entry per branch in `cihub-config`, the queue of a branch that takes submissions, and against
    Postgres and the simulator the queue that the file names on a branch, the priority of a PR in the queue of its
    branch, and the CLI with two branches.
  - Done (October 15, 2026), decided by the project lead: what the named queues on one branch gave, in one train
    (`specs/CONFIGURATION.md`). The `branch` of an entry can be a glob (`release/*`), a queue for each branch that it
    matches, and an entry that names the branch wins; `defaults` takes `queue_conditions` and `merge_conditions`,
    which each queue adds to; a queue has `profiles`, chosen once at the submission by their `conditions` or by
    `--profile` (command, CLI, `profile` of the API), which set `merge_method` (but `fast_forward`),
    `merge_commit_title`, `batching` and `failure_budget` for their PRs. `item.submitted` and `queue_items` gain
    `profile`; a batch holds one profile. Tests: globs, conditions of `defaults` and profiles in `cihub-config`, the
    batch and the failure budget of a profile in `cihub-mq-core`, and against Postgres and the simulator the merge
    method of a profile, an unknown profile refused, a queue created by a glob, and a branch without a required
    check that merges nothing.
  - Decided (October 16, 2026, validated by the project lead): the model of one train per branch with globs and
    profiles; a draining queue refuses a submission of the API on the PR, not with a `409`; this work stays a
    deviation recorded in Lot 9.
  - Done (October 16, 2026): the profile of a PR shows in the comment of CIHub, in the `profile` of the API `Pr`
    and in the table of a queue in the dashboard; a submission reads the files and the review threads once for its
    conditions, its priority and its profile (`submission_choices`). Tests: one read of the files for the three
    choices, the line of the profile in the comment, the profile in the table, and the comment of a PR with a
    profile against Postgres and the simulator.
  - Done (October 16, 2026): the API `Queue` gains `is_default_branch`, and `cihub queue status|graph|pause|resume|drain`
    without `--branch` takes the queue of the default branch of the repository. The anchors and the merge keys of
    YAML are documented. Tests: the choice of the queue of the CLI, the CLI against `cihub-server start`, and a file
    with anchors in `cihub-config`.
  - Done (October 2, 2026): the GitHub responses are recorded again for `check_setup`, with the
    rules of the target branch and of the test branches.
  - Done (October 5, 2026): CIHub on its own queue, on an instance started locally behind a tunnel. The queue
    `main` of `nlecoy/cihub` merged 9 PRs (#6, #7, #11, #15, #18, #22, #26, #28, #30); `Build Success` passes on
    each commit of `main`, and the tree of each merge is the tree of its last test. Seen during this use: the
    test `two_workers_at_the_same_time_never_share_a_job` is unstable (a claim locks up to ten times its limit,
    so the other worker can get nothing); #15 did not start before a second `/cihub merge`, cause not verified.
  - Closed on October 5, 2026 by the project lead without the rest of step 7: the end-to-end test on staging,
    two weeks of CIHub on its own queue and an installation by an outside person wait for the deployment of an
    instance.
  - Deviations: deferred, `DELETE /queues/{id}`, `GET /queues/{id}/test-results`, the `?branch=`
    search and the no-op ref update of the checker.

---

## Phase 2: throughput and visibility

### Lots 10a to 10d: dashboard

The dashboard follows the mockups v3: the list of the queues of an organization, the page of a queue (counters,
settings, live graph, items), the drawer of a PR (run, checks, history), the settings (configuration, branch
protection, audit log). It is split in five lots of about one week; each lot brings its routes, its migrations
and its pages together, and starts from the routes by name of `/orgs/{org}/…`.

Decisions:

- **Q9** (decided October 10, 2026): TypeScript, React, Vite and the TanStack libraries (Router, Query, Form),
  with a client generated from the OpenAPI document. The stack and its rules are in `web/AGENTS.md`.
- The roles are `admin` and `member`, from the SCM membership. The `viewer` role and the change of a role by an
  admin come after lot 10d (see "Next").
- The mockups v5 (October 10, 2026, validated by the project lead) drop the pages of the settings (configuration,
  branch protection, audit log), the form that changes `.cihub.yaml` and the search, which go to "Next", and add
  "Your pull requests": where each PR of the person is, and when it should merge. Lot 10e disappears; what remains
  of it, the configuration check stored for the banner of a queue, goes to lot 10b.

#### Lot 10a: session, roles and frame

- **Goal**: a person signs in with the SCM and sees the organizations that the App serves for them.
- **Content**: `GET /auth/github`, `GET /auth/github/callback` and `POST /auth/sign-out`, outside `/api/v1`;
  `sessions` table (14 days, revocable) and `memberships` table, filled from the SCM membership and refreshed
  every hour (`specs/SCHEMA_PLAN.md`); same-site cookie and CSRF token on every `POST`, `PATCH` and `DELETE` of a
  session; `GET /api/v1/me` (login, organizations with their role, version of the instance, installation URL of
  the App); a session is a second authentication of every route, and its role gives the scopes (`member`:
  `queues:read`, `prs:write`; `admin`: every scope); the sign-in flow in the simulator; a server on the
  simulator for the browser tests. `web/`: Vite, the generated client, the router, the sign-in page and its
  two errors, the frame (navigation, organization switcher, user menu), the pages "not found" and "server
  error"; the build embedded by the `web` role; the dashboard in `mise run lint`, `mise run check` and the CI.
- **Frozen interfaces**: the session cookie and the CSRF token; `GET /me`; the scopes of each role.
- **Exit criterion**: a browser test on the simulator signs in and sees its organization; a `member` gets
  `403` on `PATCH` of a queue; a session of another organization gets `404` (principle VIII); a failed sign-in
  and a user without an organization see their message.
- **Tracking**:
  - Steps: 1. `sessions` and `memberships` tables; 2. sign-in through the SCM, in `cihub-github` and in the
    simulator, and the refresh of the roles; 3. session in `cihub-web` (cookie, CSRF, scopes of a role, `/auth/*`,
    `GET /me`); 4. `web/` (tools, Vite, router, generated client, lint and tests in `mise` and the CI); 5. pages and
    the build embedded in the binary; 6. server on the simulator and browser test of the exit criterion.
  - Decided (October 10, 2026): the sign-in goes through the App that exists (authorization of a user by the
    App), coded with `reqwest`, without an OAuth crate: the App lists the installations that the user reaches
    and already reads the members. A session is a row of `sessions` that keeps the hash of its token, as
    `api_tokens`, so that a sign-out revokes it; the table is global, as `users`, because a session covers
    several organizations. The client is generated by `@hey-api/openapi-ts` at a pinned version. The tools of
    the dashboard are Node and pnpm, pinned in `mise.toml`.
  - Done (October 10, 2026): step 1. Migration `sessions`: the `memberships` table (tenant, `refreshed_at` so
    that a refresh deletes the rows that it no longer sees) and the `sessions` table (global, hash of a `chs_`
    secret, deleted by the daily maintenance once expired). `cihub-domain` gains `SessionId`, `MembershipRole`
    and its scopes, and the form of a session token; `cihub-store` creates, authenticates and deletes a session,
    and records, forgets and lists the memberships of a user. Tests: `tests/postgres.rs` of `cihub-store` and the
    maintenance test of `cihub-platform`.
  - Done (October 10, 2026): step 2. The `Scm` trait gains `sign_in_url`, `sign_in` and `members` (simulator,
    GitHub adapter); the token of the person stays in the adapter, which forgets it. `members` reads the
    installation first, so that a renamed organization is never taken for the account of a person. `record_sign_in`
    (`cihub-platform`) records the person and their roles in the organizations that CIHub knows, and deletes
    none; the hourly `sync_members` job converges the members of each connection and alone deletes. New keys `github.client_id` and
    `github.client_secret`, both or none. Tests: the simulator and the contract suite (`members`), the GitHub
    adapter on answers written by hand, `tests/postgres.rs` of `cihub-platform`.
  - Done (October 10, 2026): the simulator checks PKCE: a code is refused when the verifier does not give the
    challenge of its request. `code_challenge` (`cihub-scm`) computes it, for the simulator and for `cihub-web`.
  - Waits on the project lead: a new recording by the `github` example (`record` mode), so that the recorded
    contract suite also runs `contract::members`; the fixture admin must be an owner of the account. Then
    `contract::members` moves into `contract::run`.
  - Done (October 10, 2026): step 3. A session is a second authentication of `/api/v1`: the cookie
    `__Host-cihub_session`, the role in the organization of the URL as scopes, `404` for an organization without a
    role, the header `X-CSRF-Token` (a SHA-256 of the secret of the session, which the server keeps nowhere) on a
    request that can change a state, refused with the code `csrf_refused`; one rate limit per token or session.
    `ApiGrant` names its actor, a token or a user: the audit log and the log of the queue record the user, and a
    submission carries their account, so that the job checks their permission on the repository. The OpenAPI
    document gives the scheme `session` to every route. `GET /auth/github` and its callback, with the new key
    `http.public_url`; `GET /api/v1/me` (with the page that installs the App, from the new `Scm` method
    `app_install_url`); `POST /auth/sign-out`. The chart and `deploy/compose/compose.yaml` give the three keys of the
    sign-in.
  - Changed by the mockups v5 (October 10, 2026, validated by the project lead): a person without an organization
    gets a session, and the dashboard asks them to install the App, instead of an error of the sign-in.
  - Done (October 10, 2026): step 4. `web/`: TypeScript 6 (`typescript-eslint` does not take 7 yet), React 19,
    Vite 8, TanStack Router and Query, Zod 4, the client of `@hey-api/openapi-ts` from `web/openapi.json`, which
    the example `openapi` of `cihub-cli` writes without a database; ESLint, Prettier and Vitest in `mise run lint`
    and `mise run test`; Node 24 and pnpm 12 in `mise.toml`.
  - Done (October 10, 2026), part of step 5: the sign-in page of the mockups v5 (sign-in, failure, no
    organization, another account with `prompt=select_account`), and the build embedded in the binary by
    `crates/web/build.rs`, without a dependency (decided by the project lead); `index.html` answers every path of
    the dashboard. The image builds the dashboard in a stage of its own.
  - Done (October 10, 2026): end of step 5. The frame of the mockups v5: the top bar with the pickers of the
    organization (with the role of the person, and the page that installs the App), of the repository and of the
    queue, and the menu of the person (theme, sign-out); the routes `/orgs/$org`, `/orgs/$org/repos/$repo` and
    `/orgs/$org/repos/$repo/queues/$queue`, as the API names them, whose content comes with lot 10b; `/` opens the
    organization that the person opened last. The pages "not found", "server error" and "no access" (the API
    answers `404` on an organization where the person has no role, or that does not exist; an organization that
    the App left keeps its members, and its queues say that the access is lost, lot 10b). The pages read the data of
    the server from the cache of the queries (fresh 30 seconds), and a `401` drops `/me` from it, so that the
    sign-in does not send the person back.
  - Done (October 10, 2026): step 6 and exit criterion. `crates/cli/examples/e2e.rs` serves the API, the dashboard
    and the sign-in on the simulator, with a fresh database `cihub_e2e` (`acme` and `globex`); the simulator serves
    its sign-in on a page that a browser opens, and signs in the person chosen there. `web/e2e/sign-in.spec.ts`
    (Playwright, `mise run web:e2e`, the required job `Dashboard E2E`): a person signs in and sees their organization,
    a member gets `403` on `PATCH` of a queue, a session gets `404` on another organization and its page, a refused
    sign-in and a person without an organization see their message.
  - Checked on the App of staging (October 11, 2026): GitHub shows no account picker to a GitHub App with
    `prompt=select_account`; lot 10c removes `?choose_account=true`, and the page of a person without an
    organization tells them to sign out of GitHub to use another account.

#### Lot 10b: queues and the page of a queue

- **Goal**: read the state of every queue and of the items of one queue.
- **Content**: `Queue` gains `strategy`, `state_changed_at`, `state_changed_by` (the login, for "Paused by
  @hugo"), `target_head_sha`, `access` (`ok`, `suspended`) and `has_config_file`; `GET …/queues/{queue}` gives the
  counters of `GET /orgs/{org}/queues`, the number of live runs, the window (current, floor, ceiling) and the
  summary of the settings (strategy, batching, failure budget, merge method); `Item` gains `title`, `author`,
  `run_id`, `no_batch`, `submitted_by` and `submitted_via`; the filter `state` of `GET …/prs` takes
  several states; `Graph` gains the head of the target branch and the live runs (members, predecessors, attempt,
  status); the configuration check is stored with its time, refreshed by the repair loop, and `GET …/config-check`
  reads it without a call to the SCM. Migrations: the title of the PR, kept up to date by the reads of the PRs;
  the time and the author of the last state of a queue; the stored configuration check. Pages: the queues of an
  organization and of a repository (cards, the queues that need someone first, a filter from eight queues, the
  empty states); the page of a queue (banners of pause, drain, lost access, invalid file and up-to-date rule;
  counters; settings; live graph; the tabs of the items, live, merged, failed and cancelled). The pages read again
  every 10 seconds until lot 10d.
- **Frozen interfaces**: `Queue`, `Pr`, `Graph`, `ConfigCheck` (the forms of the decision of October 11, 2026, in
  lot 10d).
- **Exit criterion**: a browser test on the simulator shows the cards of the queues and the page of a queue with
  the PRs submitted by comment, their title, author and position, and the runs of the graph; the routes give the
  same answers after a replay of the log (principle I); showing a page calls the SCM zero times (principle VII).
- **Tracking**:
  - Done (October 10, 2026): step 1. `Queue` gains its fields, read from Postgres only: the log projects in
    `queues` the time of the last state, its author (the user of the cause of `queue.transitioned`, since a
    decision acts as CIHub), the head of the target and the window; the replay rebuilds them. `GET
    …/queues/{queue}` answers `QueueDetail`. The migration of `queues` is rewritten, as before the first version:
    a development database needs `mise run db:reset`.
  - Done (October 10, 2026): step 2. `Item` gains `title`, `author`, `run_id`, `no_batch`,
    `submitted_by` and `submitted_via`; the filter `state` takes several states, separated by commas. The title
    of a PR is kept in `pr_titles` at each read of the PR by a job, outside the log.
  - Changed (October 10, 2026, validated by the project lead): the `url` of an item goes to lot 10c, with the
    drawer that links to the PR. The adapter does not read it yet, and reading it changes the recorded answers of
    the contract, which must then be recorded again.
  - Done (October 10, 2026): step 3. `Graph` gains the head of the target branch and the last run of each live
    group: a group in `pending_failure` keeps its failed run, which the page shows as failing.
  - Done (October 11, 2026): step 4. The job `check_config` keeps the findings of the configuration of the SCM
    and the time of the check in `config_checks`; the scheduler asks for it when the last check is one hour old,
    and `…/resync` asks for it too. `GET …/config-check` reads it: the routes of the queues no longer hold the `Scm`.
  - Done (October 11, 2026): step 5. The cards of the queues of an organization and of a repository (the ones
    that need someone first, a filter from eight queues); the page of a queue: the banners of a lost access, of
    an invalid file, of a pause with its author, of a drain and of each finding of the configuration check; the
    counters; the settings; the live graph (the items that wait, then the runs, the run of the head of the queue next to the branch);
    the tabs of the live items in their order, and of the items merged, failed or cancelled in the last 24 hours,
    the tab in the address. The pages read their queue again every 10 seconds. "Your pull requests" and the
    actions stay for lot 10c.
  - Done (October 11, 2026): step 6 and exit criterion. The server of the browser tests runs the worker, the
    router and the scheduler of the queues on the simulator, where `ada` submits three PRs of `acme/web` by
    comment; `web/e2e/queues.spec.ts` checks the cards and the page of the queue (titles, authors, positions,
    runs). `the_page_of_a_queue_follows_its_prs_and_survives_a_replay` checks that the page calls the SCM zero
    times and gives the same answers after a replay.

#### Lot 10c: your pull requests, drawer and actions

- **Goal**: a person follows their PRs to the merge, and acts on the queue from the dashboard.
- **Content**: `GET /orgs/{org}/prs?author={login}`: the current items of a person in every queue, those that
  merged in the last 10 minutes, with the states of the PRs ahead of each one; `GET …/prs` gains the filter
  `state_changed_since`, for the tabs of the last 24 hours; `QueueDetail` gains the median duration of the runs
  that succeeded in the last 24 hours, for the estimate "merges in about N min"; "Your pull requests" on the
  queues, their spotlight on the page of a queue, the title of the tab. `TimelineEvent` gains `actor` and
  `description`; `RunCheck` gains `status`, `started_at` and `completed_at` (migration of `check_observations`);
  `TestRun` gains `draft_pr_url`; `Item` gains `url`, read by the adapter with the PR (the recorded contract is
  recorded again); `POST …/prs` refuses at once, with `409`, a queue that accepts nothing and a PR
  already in the queue (the refusals that need a read of the SCM stay asynchronous, principle V); `…/cancel` and `…/restart` write
  `item.cancel_requested` and `item.restart_requested` in the audit log. The comment of CIHub and the status of a PR link to the
  page of the PR in the dashboard (`http.public_url`), `/orgs/$org/repos/$repo/queues/$queue/prs/$number`. Pages:
  the drawer of a PR at this address (where it is, run, required and observed checks, history); the submission,
  with TanStack Form, cancellation and restart; pause, drain and resume.
- **Frozen interfaces**: `PrList`, `Event`, `RunCheck`, `Pr` (the forms of the decision of October 11, 2026, in
  lot 10d).
- **Exit criterion**: a browser test on the simulator signs in, creates a queue by the first `/cihub merge`,
  submits a PR from the dashboard and follows it in "Your pull requests" until the merge; a member pauses
  nothing; the comment of the PR links to its page.

- **Tracking**:
  - Decided (October 11, 2026, validated by the project lead): `Item.url` read by the adapter, with a new
    recording of the contract; the address of a PR in the dashboard, `…/queues/$queue/prs/$number`, which the
    comment links to; TanStack Form for the submission; "a moment ago" is 10 minutes; the median duration of a
    run is over the runs that succeeded in the last 24 hours; the filter `state_changed_since`.
  - Done (October 11, 2026): step 1. `GET /orgs/{org}/prs?author={login}` (`OrgPrList`), the filter
    `state_changed_since` of `GET …/prs`, which the tabs of the last 24 hours now use, and
    `QueueDetail.median_run_seconds`. `OrgPr` carries `merges_in_seconds`, computed by the server from the
    median duration of the runs and the window of the queue, so that the dashboard holds no queue logic.
  - Done (October 11, 2026): part of step 2 and step 3. `TimelineEvent` gains `actor` (the user of the cause
    of a decision) and `description` (the sentences of the blocks are those of the comment); `POST …/prs`
    answers `409` for a queue that accepts nothing (`draining`, `switching_modes`) and a PR
    already in the queue; `…/cancel` and `…/restart`
    write the audit log.
  - Decided (October 11, 2026, validated by the project lead): `CheckReport` gains `status`, `started_at` and
    `completed_at` (option 1), with the new recording of the contract; `prompt=select_account` does not show the
    account picker of GitHub for a GitHub App (checked on the App of staging).
  - Done (October 11, 2026): step 4. The status of a PR links to its page in the dashboard, and its comment
    with `Open in CIHub`, from `http.public_url`, which the module of the queue receives.
  - Done (October 11, 2026): step 5. The drawer of a PR at its address (where it is, why it waits or how it
    ended, its run and checks, its history), "Your pull requests" on the queues and first on the page of a
    queue, the title of the tab, the submission (TanStack Form), the cancellation and the restart, pause, drain
    and resume for an admin; the generated client sends the CSRF token of the session.
  - Done (October 11, 2026): step 6 and exit criterion. `web/e2e/your-prs.spec.ts`: `ada` creates the queue of
    `acme/api` by the first `/cihub merge`, submits her PR from the dashboard and follows it in "Your pull
    requests" until the merge, whose comment links to its page; a member pauses nothing. The server of the
    browser tests passes each test commit of `acme/api`.
  - The project lead started lot 10d meanwhile: the rest of lot 10c waits for the recording, not for lot 10d.
  - Done (October 14, 2026, outside the content of the lot, validated by the project lead): `users` gains
    `name`, `avatar_url` and `updated_at`, from the sign-in (`GET /user` of the SCM) and, for the avatar, from
    the hourly read of the members; `User` gains `name` and `avatar_url`, and the menu of the person shows the
    avatar; `Pr.author` becomes a `Person` (`login`, `avatar_url`), whose avatar the list of the PRs of a queue shows. The email and an admin
    of the instance wait for a feature that needs them.
  - Remaining, with the new recording of the contract: `Pr.url`, `Run.draft_pr_url`, and the state and
    times of a check (`RunCheck.status`, `started_at`, `completed_at`), fields of the frozen interfaces of the
    lot; the lot is done once they are.

#### Lot 10d: live updates

- **Goal**: the pages follow the queue without a reload.
- **Content**: `GET /orgs/{org}/stream`, server-sent events built on `NOTIFY cihub_queue_changed`, that carry only
  `queue_changed` (queue, sequence number) and a ping; the dashboard invalidates the reads of the queue that
  changed and stops its polling; the motion of the mockups v5 (what changed moves, a merged PR goes into its
  branch) and a notice when a PR of the person moves.
- **Frozen interfaces**: the event `queue_changed`.
- **Exit criterion**: a change on the simulator shows on an open page in less than 2 seconds; the stream of an
  organization carries no event of another one (principle VIII); the stream closes when the server stops.
- **Tracking**:
  - Done (October 11, 2026). A channel of its own, `cihub_queue_changed`, and not `cihub_queue`, which says that
    a queue has work before it changes: the reconciliation that writes the log, the check of the configuration
    and the creation of a queue notify it in their transaction, with the last sequence number of the log. Each
    web process listens to it once (`QueueNewsFeed` of `cihub-platform`) and passes each change to the streams
    of its organization; the stream ends when the server stops, lost changes or lagged, and the browser opens it
    again and reads everything. Written without a new crate: a `tokio::io::duplex` read by `ReaderStream`. The
    dashboard reads again the queue that changed and no longer polls; what changed slides, a new head of the
    target arrives, and a notice tells the person that one of their PRs moved. After the review: the stream
    ends after 5 minutes so that a revoked grant stops hearing, 8 streams at most per token or person, a write
    bounded to 30 seconds; the listener keeps its connection after a loss and tells the pages; the dashboard
    opens a closed stream again with a growing wait, reads everything on each open, and gathers the changes of
    half a second into one read.
  - Exit criterion: `web/e2e/updates.spec.ts` (a PR submitted on the simulator shows on the open page of its
    queue in less than 2 seconds), `the_stream_carries_the_changes_of_the_organization_only` (another
    organization, end with the server), `a_reconciliation_that_writes_the_log_tells_the_pages`.
  - Decided (October 11, 2026, validated by the project lead): an action takes a segment of its own,
    `POST …/resync`, `POST …/prs/{number}/cancel` and `POST …/prs/{number}/restart`, and no longer a colon in
    the last segment; `POST …/prs:batchGet` goes, as no client reads it (`specs/LESSONS.md`).
  - Decided (October 11, 2026, validated by the project lead): the error codes name their cause
    (`queue_not_found`, `pr_already_queued`, `item_not_in_test`, …) instead of `not_found`, `forbidden` and
    `invalid_state`, whose cause was only in `detail` (principle VI). Done: the OpenAPI document reads as a
    reference: an introduction with every code, a tag, a title and a description for each route, every error
    of each route with an example, a description and an example for every field, the closed enums as named
    schemas; `crates/cli/tests/openapi.rs` keeps it so.
  - Decided (October 11, 2026, validated by the project lead), it changes the frozen interfaces of lots 10a to
    10c: one contract for the lists (`from` and `limit`; `has_more`, `count` and an opaque base64 `cursor`); a PR
    belongs to its repository (`/orgs/{org}/repos/{repo}/prs/{number}`, with `…/events`, `…/submit`, `…/cancel` and
    `…/restart`), and `GET /orgs/{org}/prs` lists PRs with filters (`author`, `@me` for the person of a session,
    `repo`, `queue`, `state`, times, `sort`), in place of `…/queues/{queue}/prs` and of the PRs of a person;
    `GET /orgs/{org}`, `GET /orgs/{org}/repos/{repo}`; `/me` becomes `/user`, `/user/orgs` and `/instance`; one
    form per resource (`Queue` holds the counters, the settings and the check of the configuration, which leaves
    `…/config-check`; `Pr` in place of `Item`, `OrgPr` and `PrItem`; `Event`, `Run`); a `202` has no body. Done
    with it: the server, `cihub-client`, the CLI, the dashboard (the person of the session is `Me` in
    `src/session.ts`; "Your pull requests" reads the live PRs of `@me` and the last ones merged) and the tests.

### Lot 16: onboarding and the GitHub App

- **Goal**: a person who installs the App goes from GitHub to the first queue of a repository without help. Before
  that queue, the dashboard says what the branch rules of the repository miss and what its `.cihub.yaml` holds
  (`GOAL.md`: simple to install, every decision explained).
- **Content**:
  - The Setup URL of the App, `GET /auth/github/installed`, with "Redirect on update". It trusts no
    `installation_id`: it starts the sign-in again, and only the installations that the token of the person lists
    count.
  - Every sign-in connects at once, as a sync would, the installations of the person that CIHub does not know yet,
    at most 10 (`specs/GITHUB.md`, "Sign-in and roles").
  - The setup of a repository: what CIHub found about its branch rules and the configuration file at the head of its
    default branch, with a state (`started`, `checking`, `ready`, `ready_with_warnings`, `cannot_merge`,
    `invalid_config`), computed by the server. The job `read_setup` reads it at the discovery of a repository, then
    once a day spread over the day (class `repair`), and at a `resync` of an admin (class `interactive`, at most
    once a minute per repository, 50 repositories for the organization). It is kept in `repo_setups`, outside the
    log.
  - `GET /orgs/{org}/setups` (with `state` and a search `q`), `GET /orgs/{org}/repos/{repo}/setup`,
    `POST /orgs/{org}/repos/{repo}/resync` and `POST /orgs/{org}/setups/resync` (`queues:write`); the `GET` routes
    read Postgres only.
  - A repository that the App no longer reaches gets `repos.removed_at`, and its queues `access.lost`, then
    `access.restored` when it comes back; `Org.access` says when every connection of an organization is suspended.
  - Pages: `/welcome` for a person without an organization (it replaces "No organization yet"), "Connecting" after
    the Setup URL, "Start your first queue" with a search and "Check again" (admins), the setup of a repository
    before its queue, the lost access of an organization and of a queue. A page reads again every 2 seconds, for 60
    seconds at most, while it waits for a read; the first `/cihub merge` still shows through `queue_changed`.
- **Frozen interfaces**: the routes and the types `RepoSetup`, `SetupState`, `SetupList`, `OrgAccess`; the table
  `repo_setups`.
- **Exit criterion**: on the simulator, a person without an organization installs the App, comes back through the
  Setup URL, sees "Connecting" then the setup of each repository (`cannot_merge`, `invalid_config` with its line,
  `ready`); a fixed rule and "Check again" turn a repository to `ready`, and a `/cihub merge` to `started` without
  a reload (`web/e2e/onboarding.spec.ts`); a Setup URL with an `installation_id` that the person does not reach
  creates nothing and gives no role; showing a setup calls the SCM zero times, and the setup of another
  organization answers `404 org_not_found` (principles VII and VIII).
- **Decisions** (October 16, 2026, validated by the project lead):
  - Vocabulary: `sync` is the internal copy of an object of the SCM; `resync` the public action that reads again
    from the SCM; `setup` what CIHub found about a repository, with a verdict; `check` stays the checks of the CI.
    So `Queue.config_check` becomes `Queue.setup`, `config_checks` becomes `queue_setups` and `check_config`
    becomes `read_setup`: a repository and its queue show the same setup, before and after the first queue.
  - The two `resync` routes take `queues:write`: a read in class `interactive` goes ahead of the merges, and a token
    of `queues:read` must not spend the budget of an installation.
  - The "request sent" state of a member waits for a check on the App of staging: GitHub does not document a return
    to the Setup URL after a request.
  - The mockups v20 give the screens; the specs win where they differ: an organization that the App left still
    opens, with a banner; the setup of a repository names the entries of the file of its default branch, and a
    queue of another branch reads the file of that branch; the buttons of the configuration come with the form of
    "Next"; the server writes every sentence.
  - The work starts from PR #143 (one queue per target branch) without waiting for its merge.
- **Tracking**: to do.

### Lots 11a to 11c: analytics, metrics and outgoing webhooks

Decisions (October 14, 2026, validated by the project lead):

- Lot 11 is split in three lots with their own exit criterion: 11a (analytics and the Health view of the
  mockups v8), 11b (Prometheus), 11c (outgoing webhooks).
- `cihub-analytics` is a new crate. The facts are four tables, `an_item_conclusions`, `an_state_durations`,
  `an_test_run_results` and `an_check_results`; the hourly aggregate (`an_hourly_queue_stats`, or the
  `measurements` table of `specs/SCHEMA_PLAN.md`) comes when a range is longer than 30 days.
- A route per view: the server computes the Health of a queue, its verdict included, and the dashboard holds no
  queue logic.
- Lot 11a does not wait for lot 10c: the duration of the checks and the CI time stay empty until the times of a
  check are recorded.

#### Lot 11a: analytics and the Health view

- **Goal**: a person sees what a queue did over 24 hours, 7 days or 30 days, and why its PRs did not merge.
- **Content**: the `notifier` role runs the subscriptions of the modules on the log `events`, the outbox: one
  cursor per queue and per subscription (`log_cursors`), advanced in the transaction of the effect, woken by
  `cihub_queue_changed`. The merge queue subscribes `cihub-analytics`, which derives the facts from the events of
  the log only: one row per concluded item, per pass of an item through a state, per ended run and per check of
  an ended run. `GET /orgs/{org}/repos/{repo}/queues/{queue}/health?range=24h|7d|30d` (`QueueHealth`): the
  verdict (a closed vocabulary), the merged and failed PRs, the time to merge (median and 90 %), each against
  the period before; the buckets of the charts (an hour for `24h`, a day otherwise, in UTC); the median time in
  each state of a merged PR; the reasons of the PRs that left without a merge; and per check, its runs, its
  failures and the times it passed on a retry. Page: the Health tab of a queue.
- **Frozen interfaces**: `QueueHealth`, the `an_*` tables, the `Module` subscriptions.
- **Exit criterion**: a subscription whose effect fails leaves its cursor where it was; the facts rebuilt from
  `events` equal those written continuously; the Health of an organization reads no fact of another one
  (principle VIII); a browser test on the simulator shows the Health of a queue with a merged PR and a failed one.
- **Tracking**:
  - To validate: the `AnalyticsStore` trait comes with its second implementation, ClickHouse (lot 15); until
    then, `cihub-analytics` gives functions on Postgres. The fact tables are not partitioned and keep everything:
    their retention comes with lot 15.
  - Done (October 14, 2026): the migration of `log_cursors` and of the four fact tables; `Module.subscriptions`
    and `run_subscriptions` in `cihub-platform`, which the `notifier` role runs; `cihub-analytics`, whose facts
    read the log only (the transitions of an item at its end; the creation, the assembly and the reports of the
    checks of a run at its end); `GET …/queues/{queue}/health` (`QueueHealth`), whose verdict the server computes,
    and whose reasons carry their conclusion, so that the page colors a failure and a cancellation apart; the
    Graph and Health panes of the page of a queue (`?pane=health&range=7d`), after the mockups v8, without the
    CI time and the duration of a check, which wait for lot 10c, and without the Config pane.
  - Exit criterion: `a_subscription_that_fails_reads_its_events_again`,
    `the_health_of_a_queue_reads_the_facts_of_its_log` (a PR merged after its check passed on a retry, a PR
    failed; the facts rebuilt from the log are the same; another organization reads none of them),
    `web/e2e/health.spec.ts`.
  - Done (October 15, 2026): an open Health reads its facts again every 30 seconds, and not at each change of its
    queue, which mostly came before the notifier wrote them (validated by the project lead).
  - Done (October 15, 2026): the Health reads the hourly aggregate (`an_hourly_conclusions`,
    `an_hourly_durations`, `an_hourly_checks`), which the notifier adds to with the facts that it writes; the
    medians and the 90th percentiles come from histograms, within 5 %; `an_check_results` gains
    `passed_on_retry`. Validated by the project lead, with three typed tables rather than `measurements`.
  - Done (October 15, 2026): a period that the person chooses, after the mockups v8-calendar: `GET …/health`
    takes `from` and `to` (the last 13 months, in UTC) instead of `range`, and cuts the period in hours, days or
    weeks after its length; the Custom choice of the Health opens a calendar of two months, its shortcuts and the
    hours, and keeps the period in the address of the page (`?from=…&to=…`).

#### Lot 11b: Prometheus

- **Content**: `/metrics` on the internal address; `GET /api/v1/metrics`; the service and product series of
  `specs/OBSERVABILITY.md`.
- **Exit criterion**: `rate()` works on the exposed counters.

#### Lot 11c: outgoing webhooks

- **Content**: Standard Webhooks outgoing webhooks with the CIHub vocabulary, a subscription of the `notifier`;
  `/orgs/{org}/webhooks` endpoints.
- **Exit criterion**: each webhook type is emitted exactly once per transition, signed, and retried after a
  failure.

### Lot 12: `batched` strategy

- **Content**: batch formation, `no_batch`, maximum wait; bisection with a cache of the results and a
  `bisection_concurrency` pool; `pending_failure_depth > 0`; optimistic merge with the strict rule; an inferred failure never evicts a PR; `batched` strategy; a load model in the
  simulator (arrivals, CI duration, broken and flaky PRs). Taken over from lot 7: measure the useless assemblies
  after the closing of the PR at the head, and remove them above 5 %.
- **Exit criterion**: `fault_injection` passes with `batched`; a planted failure is
  isolated in ≤ log2(batch size) rounds of runs after the confirmed failure of the batch; on the fixed scenario
  of the simulator (`crates/mq-core/tests/load.rs`), `batched` uses at least 30 % fewer runs per merged PR than `pipelined`, with a p50 time in queue at
  most 1.5 times as long (`mise run load`).
- **Tracking**:
  - Steps: 1. batch formation; 2. bisection (cache, `bisection_concurrency` pool); 3.
    `pending_failure_depth > 0`; 4. optimistic merge; 5. load model and cost criterion in `cihub-mq-core`; 6. useless assemblies; 7. `fault_injection` with `batched`, exit criteria.
  - Done (October 6, 2026): step 1. `plan` forms a batch from the first pending item whose predecessors are in
    groups and the pending items that follow it, up to `batching.size`, until a `no_batch` item; a batch that is
    not full waits until its oldest member has waited `batching.max_wait_minutes` since its admission
    (`Item.admitted_at`, set by `apply` from the admission), and `wake_at` gives the end of the wait. A batch
    takes one slot of the window. A member that leaves dissolves the batch, and the others restart with
    `batch_member_left`; `validity` finds a batch by its first member still in the
    queue, so that a push on the target restarts the members left after a partial merge. Tests: scenarios in
    `tests/batching.rs`, batches in the property test.
  - Done (October 6, 2026): step 2. The `bisection` rule of `plan` bisects a group of several PRs in failure
    whose predecessors merged, at once: the first half is tested in a child group of kind `bisection`,
    from the `bisection_concurrency` pool; the other half waits in the group, in `pending_failure`. A child
    that passes makes the other half split in turn, down to one PR tested alone; with
    `optimization_mode: off`, that half goes back to the queue. A child that fails or is given up sends the
    other half back to the queue. The first split of a batch closes the window by half, and the failure of a
    bisection group does not close it again. `evict` keeps a finished half while its parent is live; `check`
    accepts a member in a group in bisection and in its half. The budget restart of a bisection group draws
    from the bisection pool. Tests: five scenarios in `tests/batching.rs` (culprit last, culprit first,
    flaky failure, `optimization_mode: off`, full pool); the CI of the world (`World::drain`) moves the clock
    when a round has no run.
  - Done (October 6, 2026): a merge conflict at the assembly of a group of several PRs sends it to
    `pending_failure`, and the bisection finds the PR that conflicts. Test: a conflict in a batch evicts
    only its PR.
  - Done (October 6, 2026): the reason `test_run_failed_by_optimization` is removed.
  - Done (October 6, 2026): step 3. With `pending_failure_depth: N > 0`, a group in failure whose predecessors
    merged waits for the runs of the first `N` live groups behind it whose predecessors include its members,
    before its failure budget, its failure or its first split; a group without such a successor does not
    wait. A successor that passes releases nothing yet: that is step 4. Tests: `tests/failure_depth.rs`, and
    a depth of 0 to 2 in the property test.
  - Done (October 6, 2026): step 4. With `optimistic_merging`, the `optimistic` rule of `plan` takes a group
    that passed by its own run, and gives `tests_passed` to each live group whose live members are all in its
    predecessors: `passed_by_optimization` from `testing` (its run is interrupted), `successor_test_run_passed`
    from `pending_failure`; the members record the run of the successor. A group in bisection is not proved:
    its halves are. An evicted group still restarts the groups behind it. `evict` keeps a run that a
    live item cites as its proof. Tests: `tests/optimistic.rs`, and the option in the property test.
  - Done (October 6, 2026): step 5. `tests/load.rs` plays the scenario of the exit criterion in the world of `cihub-mq-core`:
    exponential arrivals, a CI of 20 minutes, broken and flaky PRs, three fixed seeds, `pipelined` against
    `batched`. Measured (October 6, 2026): runs per merged PR 4.18 against 1.69 (−60 %, the bar of −30 %
    holds); p50 time in queue 465 minutes against 881 (+89 %, the bar of +25 % fails). The test fails
    until a decision on the algorithm, the scenario or the bar. After three changes validated by the project lead (halves at once,
    groups behind given up, batches of 4): runs per merged PR 4.18 against 1.09 (−74 %); p50 465 minutes
    against 657 (+41 %). With 3 % of broken PRs instead of 10 %: 1.91 against 0.58 runs, p50 76 against 129
    minutes (+70 %). The project lead sets the time bar at 1.5 times: the test passes (1.41), in `mise run load`.
  - Done (October 6, 2026): step 7. `fault_injection_batched` plays the fault injection test of lot 7 with
    `strategy: batched` and checks that the seeds formed batches and bisections; it passes on 200 seeds, and
    `fault_injection` too. In the CI of both tests, a red PR fails every run that tests its head, as a member
    or as a predecessor: before, only its own run failed, and a run on top of it could prove it by optimistic
    merge. `mise run sim` plays both tests. `a_planted_failure_is_isolated_in_log2_rounds` checks the
    bisection for each batch size from 2 to 8 and each place of the failure.
  - Done (October 6, 2026): step 6. `useless_assemblies_after_a_close_stay_under_5_percent` plays the load of
    the exit criterion where 5 % of the PRs close up to an hour after their submission. An assembly is useless when its run
    stops without a result because a PR that it tested closed (`prerequisite_merge_item_cancelled`,
    `batch_member_left`); the run of the closed PR alone does not count. Measured: `pipelined` 2 out of 1,765
    assemblies, `batched` 0 out of 454: under 5 %, so nothing to remove. Limit: an assembly is instant in the
    simulated world, so the measure is a floor.
  - Done (October 7, 2026), after a manual test on `getsyncr/contracts`: a batch that is not full starts
    at once when the item right behind it is `no_batch`. Test: `a_no_batch_pr_does_not_wait_for_the_batch_ahead_of_it`.
  - Closed on October 6, 2026 by the project lead: the exit criteria pass (`mise run load`, `mise run sim`, the
    bisection test).
  - Later, outside the lot (asked by the project lead on October 6, 2026, not done): a nightly CI job for
    `mise run sim` and `mise run load`. No workflow runs them today, although `AGENTS.md` says that the CI runs
    `sim` at night.

---

## Phase 3: flaky tests

### Lot 14: `flakytests` module

- **Content**: OTLP/HTTP receiver in the `web` role; `cihub test-results push --junit` and the GitHub action;
  `ft_results` and `ft_cases` tables; the three detection signals, including bisection; quarantine;
  `cihub-flaky-core` and `cihub-flakytests`, implementation of the `FlakeOracle` trait (`cihub-contracts`) and
  restart reason (Q1); "Flaky tests" page, `/api/v1/flaky/*` API, `flaky.*` webhooks. This lot can be split in two
  (reception and storage, then detection and oracle) if it exceeds one week.
- **Exit criterion**: criterion of phase 3; with the module disabled, the queue suite passes without any
  change; the oracle answers in less than 50 ms at p99 on 1 M results.
- **Open questions**:
  - **Q1** The reason `tests_restarted_flake_suspected` (`specs/FLAKY_TESTS.md`) is not in the closed vocabulary.
  - **Q7** OpenTelemetry semantic conventions for tests: follow the proposal of the CI/CD group, or freeze
    `cihub.*` attributes as long as it is not stable.
  - **Q12** `specs/FLAKY_TESTS.md` subscribes the module to `mergequeue.run.completed`, which is not an event
    type of `specs/DOMAIN.md` (`run.transitioned`?).

---

## Phase 4: scale and ecosystem

### Lot 13: `parallel` strategy

- **Content**: `POST /orgs/{org}/repos/{repo}/prs/{number}/impacted-targets` (`targets:write` token or GitHub Actions
  run for forks, 20 MB limit); lanes by overlap; batches per lane; `parallel` strategy;
  example actions for Bazel and Nx.
- **Exit criterion**: on the simulator, PRs with disjoint targets are tested and merged in
  parallel, and an `ALL` PR serializes everything that follows it; criterion of phase 4 on a pilot monorepo.

### Lot 15: scale: ClickHouse, hardening

- **Content**: ClickHouse implementation of `AnalyticsStore` (Q8); load test at the order of magnitude of the
  standard instance; chaos exercise on staging; the two bars of phase 1 measured by hand; API
  compatibility policy and first published version. The Helm chart came before this lot.
- **Exit criterion**: an instance holds 1,000 PRs per day with ClickHouse enabled; the migration from Postgres to
  ClickHouse is a copy without any change to the application code.
- **Open questions**:
  - **Q8** Switch-over threshold to ClickHouse: volume from which the documentation recommends it.

### Next

Stacked PRs, direct-to-queue with the `cihub-sudo` App, Slack, Terraform, GitLab behind `Scm`; the `viewer` role
and the change of a role by an admin (`specs/SECURITY.md`); from the mockups v3, the pages of the settings
(configuration and its sources, branch protection with "Check again", audit log), the form that changes
`.cihub.yaml` through a PR, and the search of a PR. These lots are split when phase 4 starts.

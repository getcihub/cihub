# Lessons

What we tried or considered, and why we do not do it. Read before you propose a change in the same area. Add an
entry when an approach fails or is rejected for a reason that the code cannot show; remove an entry when its
reason no longer holds.

## Language and dependencies

### `jsonwebtoken` and `hmac` for the App JWT and the webhook signature

- **Tried / considered**: the usual crates for RS256 and HMAC-SHA256.
- **Why not**: `jsonwebtoken` offers only `aws-lc-rs` (C code) or RustCrypto, whose `rsa` crate has an advisory
  on signing (RUSTSEC-2023-0071). `ring`, already present through sqlx and rustls, does both.

### `serde_yaml` or `serde_yml` to read `.cihub.yaml`

- **Tried / considered**: the common YAML crates.
- **Why not**: `serde_yaml` is archived and puts an error in a list on the previous element; `serde_yml` has an
  advisory (RUSTSEC-2025-0068). The position of an error is part of the product: `serde-saphyr` gives it.

### A full GitHub SDK

- **Tried / considered**: a complete GitHub client crate instead of a small client on `reqwest`.
- **Why not**: every call must go through the budget of the connection (class, pause, rate headers). The few
  endpoints in use fit in a small client where we control each request.

## Tooling, CI and release

### `cargo:` backend for mise tools

- **Tried / considered**: install the tools with the `cargo:` backend of mise.
- **Why not**: it compiles each tool from source in each CI job. Use `aqua:` or `github:` (precompiled binary,
  checksum in `mise.lock`); keep `cargo:` only for a tool that publishes no binary.

### release-please with the `rust` release type

- **Tried / considered**: the `rust` type, made for Cargo projects.
- **Why not**: it does not handle a workspace whose root has no `[package]`. The `simple` type with `extra-files`
  updates `[workspace.package].version` and the `cihub-*` versions in `Cargo.lock`.

### A self-hosted runner for the CI

- **Tried / considered**: `cihub-runner`, a runner set of Actions Runner Controller in a home cluster, for the CI,
  the fault injection test and the image.
- **Why not**: the repository is public, and a self-hosted runner runs the code of the pull requests of forks.
  The standard runners hosted by GitHub are free for a public repository.

### The chart jobs only on a pull request that changes the chart

- **Tried / considered**: `Helm Lint`, `Helm Unittest` and `Smoke` in a workflow filtered on the paths of the
  chart, plus a run once a night.
- **Why not**: `Smoke` also checks the image, the migrations and the configuration, so a change of the code can
  break an installation by the chart without touching these paths. On the free runners of a public repository,
  they run on every pull request.

### The chart and the compose installation under `deploy/`

- **Tried / considered**: `deploy/charts/cihub` and `deploy/compose/compose.yaml`, next to the development
  `compose.yaml` at the root.
- **Why not**: two compose files to keep in step, and a folder that groups two tools with different users. The
  chart is in `helm/cihub`; one `compose.yaml`, with profiles and its configuration files in `.docker/`, serves
  development; production installs with the chart.

### `${VAR:?}` for the values of a compose profile

- **Tried / considered**: the server values of `compose.yaml` required with `${VAR:?message}`, and the private key
  as a bind mount `${PATH}:/run/secrets/…`.
- **Why not**: Compose reads every variable of the file before it selects the profiles. A missing value, or an
  empty source of a bind mount, stops `docker compose up postgres` too, without the values of the server. The values take a `:-`
  default, the key is a top-level `secrets:` entry, and `cihub-server` stops at startup on a missing value.
- **Would change if**: Compose skips the services of the inactive profiles when it reads the variables.

## Process and binaries

### One binary for the server and the command line

- **Tried / considered**: one `cihub` binary, with `cihub server start|migrate|token` next to `cihub login` and
  `cihub queue`.
- **Why not**: the command line installs in the CI of the users and on their machines. With the server in it,
  it carried `sqlx`, `axum`, the GitHub adapter and the queue (221 crates instead of 116), and their
  vulnerabilities. `cihub` (`cihub-cli`) and `cihub-server` are two binaries; the image carries only
  `cihub-server`.
- **Would change if**: the command line needs the code of the server.

### A selection of the roles per process

- **Tried / considered**: `CIHUB_ROLES` or a role argument to run only some of the four roles in a process.
- **Why not**: identical replicas already share the work through leases, locks and idempotency keys. A server
  that always runs the four roles is simpler to ship and to deploy.
- **Would change if**: a measured need for a role on its own appears.

### A crate per role

- **Tried / considered**: one crate for `web`, `scheduler`, `worker` and `notifier`.
- **Why not**: a role only starts loops whose code lives in the library crates. `cihub-cli` has one file per role.
- **Would change if**: the code of a role grows enough to need its own crate.

### Traits for the inbox, the jobs, the outbox and the module contract

- **Tried / considered**: a trait per platform service, and a `Module` trait.
- **Why not**: these functions write in the caller's transaction, which no other implementation could join. With
  one implementation, a trait is only indirection.
- **Would change if**: a second implementation arrives.

### A per-organization module activation table (`org_modules`)

- **Tried / considered**: enable each module per organization in a table.
- **Why not**: there is one module, and every organization has it. The table selected nothing and was removed.
- **Would change if**: a second module (flaky tests) needs activation per organization.

## Postgres

### NATS or another message broker between the roles

- **Tried / considered**: a transport for the wake-ups and the jobs.
- **Why not**: Postgres is enough (`LISTEN/NOTIFY`, `FOR UPDATE SKIP LOCKED`, leases), and it is one less service
  to run.
- **Would change if**: a measurement shows that Postgres is the limit.

### A lease alone to give a queue to one scheduler

- **Tried / considered**: the `queue_leases` row as the only guard.
- **Why not**: during the takeover of an expired lease, two replicas can reconcile at once. Each reconciliation
  also takes `pg_try_advisory_xact_lock` on the queue.

### An expression index on `payload->>'run_id'`

- **Tried / considered**: find the events of a run through the JSON payload.
- **Why not**: Postgres ignores the statistics of the expression of a partial index and reads the whole
  partition. `events` has a real `run_id` column with a partial index.

### A Postgres init script for the development role

- **Tried / considered**: an init script in the Postgres container to create the `cihub` role and database.
- **Why not**: it runs only on an empty volume, so a change never reaches an existing database. A one-shot
  `postgres-setup` service converges the role and the database at each start.

### Write `last_used_at` on each API request

- **Tried / considered**: update the token row at each authenticated call.
- **Why not**: each read request would write to the database. `last_used_at` advances at most once per minute.

### Several SCM instances in one deployment

- **Tried / considered**: an `scm_instances` table and an `scm_instance_id` in each unique key.
- **Why not**: a deployment reads one SCM configuration (one URL, one App), so the column had the same value in
  every row. A GitHub Enterprise Server gets its own deployment.

### A state filter as a bind parameter, or as an index column, under row level security

- **Tried / considered**: `state = ANY($n::text[]::item_state[])` in the lists of PRs, and indexes that carry
  `state` as a column, such as `queue_items (queue_id, state, state_changed_at)`.
- **Why not**: on a table with row level security, a condition whose operator is not `LEAKPROOF` is applied after
  the policy, never as an index condition, and its statistics are not used. The equality of a Postgres enum is not
  `LEAKPROOF`, so every `state` filter read the whole history of a queue or an organization. A state filter is
  written with literals of the closed vocabulary (`ItemState::as_str`), and the index is partial on the states:
  the predicate of a partial index matches literals, not a parameter. Measured on 300,000 items, the list of the
  queues went from 140 ms to 2 ms.

## Event log

### A separate outbox table

- **Tried / considered**: write each event and its outbox entry in the same transaction.
- **Why not**: the `events` log is already the outbox. A consumer keeps the last `seq` it handled per queue;
  `seq` has no gap and follows the commit order, because only the lease owner writes the log of a queue.

### Delete old `events` partitions without snapshots

- **Tried / considered**: drop partitions older than 90 days on a schedule.
- **Why not**: an inactive queue would lose its whole log and its `seq` would restart at 1. A partition is
  dropped only when each of its queues has a snapshot at a `seq` at least as high.

### Logins in the log

- **Tried / considered**: store the user login in the events.
- **Why not**: a rename or an anonymization would have to rewrite the log. The log keeps the user identifier;
  `users` keeps the login, and a change touches one row.

## Jobs and inboxes

### Keep processed rows in `webhook_inbox` and `queue_inbox`

- **Tried / considered**: `processed_at`, `routed_at` and `error` columns, a purge by the maintenance job, and a
  sweep that compares the SCM delivery list with `webhook_inbox`.
- **Why not**: an inbox holds only work to do. CIHub answers 2xx only after it stores a delivery, so the sweep
  asks again the deliveries with no 2xx attempt on the SCM. A replay is safe: job keys carry the delivery.

### Renew the lock of a running job

- **Tried / considered**: a heartbeat that extends `locked_until` for a long executor.
- **Why not**: the worker stops an executor after 50 s, before its 60 s lock expires, so two workers never run
  the same job. An executor accepts being stopped at each `.await`.

### Count every assembly error toward its attempts

- **Tried / considered**: one attempt budget for all errors of `assemble_test_branch`.
- **Why not**: 5xx, 429 and stops are not the fault of the assembly and would use up the budget. Only 422
  counts (5 attempts); other errors retry until 15 minutes after the run starts, and `MAX_ATTEMPTS` covers that
  backoff.

### A fixed scheduler tick

- **Tried / considered**: wait `TICK` (30 s) between two passes when no notification arrives.
- **Why not**: a `wake_at` was served up to 30 s late (a batch with `max_wait_minutes: 1` waited 1 min 30). The
  scheduler waits until the earliest future `next_tick_at`, between 1 s and `TICK`.

## Queue algorithm

### A lenient optimistic merge

- **Tried / considered**: keep the downstream groups when a predecessor is evicted.
- **Why not**: their tests included code that will not land, so the target could turn red. They restart.

### Evict a PR on a failure inferred by the bisection

- **Tried / considered**: a half that the bisection proves failed without a run fails its PR
  (`test_run_failed_by_optimization`, now removed).
- **Why not**: an inferred failure is not a proof (a flaky test can explain it). A PR is evicted only after its own
  failed run and its failure budget.

### Fail every member of a batch on a merge conflict at assembly

- **Tried / considered**: a conflict at the assembly of a group of several PRs fails all its members.
- **Why not**: only one PR conflicts. The group goes to `pending_failure` and the bisection finds it. A group of
  one PR still fails at once.

### Let a batch that cannot grow wait for `max_wait_minutes`

- **Tried / considered**: a batch that is not full always waits for more PRs.
- **Why not**: when the item right behind it is `no_batch`, the batch cannot grow and that item waits for nothing.
  Seen on a manual test. The batch starts at once.

### Expect `batched` to lower the time in queue in the simulator

- **Tried / considered**: a time bar that required `batched` to be about as fast as `pipelined`.
- **Why not**: the simulated CI has no limit of runners, so a batch saves runs but cannot save time. The time bar
  only limits the delay (p50 at most 1.5 times that of `pipelined`).

### `fast_forward` with a strategy other than `sequential`

- **Tried / considered**: allow `merge_method: fast_forward` with `pipelined` or `batched`.
- **Why not**: the assembly redoes the merges of the predecessors on `base_sha`, so the test commit of a PR does
  not descend from that of its predecessor and the target cannot fast-forward to it.

### Enable `trigger: inplace` by removing the configuration refusal

- **Tried / considered**: update the branch of the PR itself and test it there.
- **Why not**: it needs more than the setting: an exemption in the PR changes when the head is the test SHA, a
  `sha` guard of the merge on that SHA, formation only without a live predecessor, and routing of checks by SHA.

## Configuration

### Several queues on one branch

- **Tried / considered**: named queues on the same branch (`main` and `hotfix` on `main`), each with its own log
  and train, and a routing of each submission by `--queue=<name>` or `route_conditions`.
- **Why not**: both trains merge into the same branch, and neither tests on top of the other. A queue knows only
  its own merges (`own_shas`), so each merge of one is an outside push for the other: it drops all its runs with
  `manual_push_to_target_branch`, a reason that is false on the PR. The needs behind a second queue have another
  answer in one train: `priority_rules` and `--priority` for an urgent PR, `--no-batch` to test it alone,
  profiles for other settings, and the lanes of the `parallel` mode for unrelated changes. Mergify went the same
  way: it removed `partition_rules` (2025), the implicit priority between `queue_rules` (2025) and
  `routing_conditions` (2026).

### Choose a profile with `queue_conditions`, or let it change the train

- **Tried / considered**: choose the profile of a PR, as Mergify chooses a `queue_rule`, by the first profile
  whose `queue_conditions` pass; and let a profile set any setting.
- **Why not**: the submission reads the PR once, but `queue_conditions` change over time (checks, reviews): a PR
  that does not pass them yet would keep another profile. `conditions` choose; `queue_conditions` admit. A
  profile sets only the settings of a PR: `strategy`, `window` or `trigger` describe the whole train, and
  `fast_forward` moves the target onto a test commit that the other PRs of the train do not descend from.

### Conditions as strings of a small language

- **Tried / considered**: write each condition as a string, `[-][#]attribute [operator value]` (`-draft`,
  `-label = do-not-merge`, `#approved-reviews-by >= 2`, `~=` for a regex, `*=` for a glob), in a list with
  `and`, `or` and `not` blocks.
- **Why not**: it is the syntax of another product. Its symbols must be learned, two forms say the same thing
  (`-label = x` and `label != x`), its meaning on a list does not show (`!=` negates "any", `~=` keeps it),
  several values need a block or several lines, and a string needs its own parser, its quotes and its escapes.
  The dashboard writes the file through a form, which maps to keys, not to a string to build. The conditions are
  typed YAML maps (`specs/CONFIGURATION.md`): a schema can describe them for an editor or an agent, and an
  explanation shows the path and the values that fail.

## GitHub

### An App creation wizard (manifest flow)

- **Tried / considered**: a `/setup` page that creates the GitHub App and shows its secrets once.
- **Why not**: CIHub is not designed for a single SCM, the wizard needed a public URL and state cookies, and a
  manual creation from `specs/GITHUB.md` is enough.

### Webhooks as the source of truth for installations, and a bootstrap command

- **Tried / considered**: create nothing from webhooks, and create organizations with `cihub server bootstrap`.
- **Why not**: webhooks get lost. CIHub reads the installations of its App on the SCM (at start, daily, and on
  each signal) and converges `scm_connections`; `cihub-server token create` only makes a token.

### The device flow of the GitHub App for the command line

- **Tried / considered**: `cihub auth login` runs the device flow of the GitHub App, then trades the GitHub token
  for a CIHub session; or `cihub auth login --token`, with an API token that the person copies from the server.
- **Why not**: the command line would know GitHub, the App would need its device flow turned on, and CIHub would
  take an SCM token as input. CIHub has its own device codes on top of the sign-in of the dashboard, so the command
  line stays neutral. A copied API token stands for an organization, not for the person: a submission would carry
  no account, and no permission of the person would be checked.
- **Would change if**: a person must sign in where no browser reaches the dashboard at all.

### The code of the command line in the address of the page `/device`

- **Tried / considered**: `cihub auth login` shows `/device?code=WDJB-MJHT`, and the page shows the code for the
  person to compare with the terminal, then approve (`verification_uri_complete`, RFC 8628, section 3.3.1).
- **Why not**: an attacker runs `cihub auth login` and sends the link. The person has no terminal to compare with,
  and one click on *Approve* gives the attacker a session that acts as the person. The person types the code, as
  on the device page of GitHub.
- **Would change if**: a sign-in needs no typing, for example on a device without a keyboard.

### Forget the token of the person at the end of the sign-in

- **Tried / considered**: the sign-in reads the installations and the memberships of the person with their token,
  then forgets it; the role in the organization gives the access to every repository of it.
- **Why not**: a member of an organization then reads in CIHub a private repository that GitHub hides from them,
  and keeps the access that they had at the sign-in for the 14 days of the session. CIHub keeps the token of the
  person, encrypted, refreshes it, and reads with it what the person reaches (lot 16, October 17, 2026).
- **Would change if**: the token of an installation can list the repositories that a person reaches.

### Forget the permissions of a person at their sign-out

- **Tried / considered**: the sign-out of the last session forgets the token of the person and the permissions that
  it read; the next sign-in reads them again in a `sync_user` job.
- **Why not**: the dashboard opens on the organization before the job ends, and its lists show no repository
  until a reload. The permissions now outlive the sign-out for 14 days, the life of a dashboard session; the token does not,
  since the next sign-in gives a new one. Keeping the token too was rejected: it serves no person without a
  session, and its refresh token lasts months (October 18, 2026).
- **Would change if**: a list must never show a repository that the person lost on the SCM, even for the seconds
  of a sync.

### Trust a `blocked` PR state as is

- **Tried / considered**: keep a PR out of the queue while GitHub reports it `blocked`.
- **Why not**: when the branch requires the `CIHub Merge Queue (<branch>)` status, the PR is blocked by the queue
  itself and can never enter. When that is the only unmet check, the jobs report the PR `mergeable`.

### Use the asynchronous merge endpoint by default

- **Tried / considered**: `PUT /pulls/{n}/merge-async` for every merge.
- **Why not**: GitHub Enterprise Server (3.22) has only `PUT /pulls/{n}/merge`. The adapter uses the
  asynchronous endpoint only when a stacked PR gets a 403 that names it.

### Read the queue or the actor from the delivery of a comment or a label

- **Tried / considered**: route a `/cihub` command by its delivery, and attribute a label submission to a user.
- **Why not**: a comment delivery does not give the base branch, and a label delivery does not say who added the
  label. `submit_pr` reads the PR to find the queue, and a label submission has the `system` actor.

### Filter the check deliveries by name before a PR read

- **Tried / considered**: read a waiting PR only for the checks that matter.
- **Why not**: not measured yet. A CI with N checks gives up to N reads per push of a waiting PR.
- **Would change if**: a measurement shows that these reads cost too much of the budget.

## PR surfaces

### Checkboxes for `restart` and `cancel` in the PR comment

- **Tried / considered**: a box to restart or cancel, as for the submission.
- **Why not**: a click has no confirmation, both actions affect the PRs behind, and the comment of an item under
  test is often rewritten.

### A relative delay in the comment of a waiting batch

- **Tried / considered**: "closes in 4 minutes".
- **Why not**: the comment is not rewritten every minute (SCM budget), so a relative delay goes stale. The
  comment shows an absolute UTC time.

### Mention users in the history of the comment

- **Tried / considered**: write `@login` in the `By` column.
- **Why not**: each rewrite of the comment would notify the user. The login is in a code span.

### Fail a command job when its acknowledgement fails

- **Tried / considered**: treat a failed `+1` reaction as a job error.
- **Why not**: the retry would give the facts of the command twice. A failed acknowledgement is a `warn` log.

### Write a job result in a column of `queues`

- **Tried / considered**: keep the setup of a queue, what the rules of the SCM miss, in `queues.config_findings`,
  written by the job `read_setup`.
- **Why not**: the lease of the queues takes their rows with `FOR UPDATE SKIP LOCKED`. While the job holds the
  row, the scheduler skips a dirty queue, and nothing wakes it before the next tick (30 seconds). The setup has
  its own table, `queue_setups`. A job that must write `queues` calls `mark_dirty` in its transaction, as
  `load_config` does.

## API

### An action after a colon in the last segment of a route

- **Tried / considered**: `POST …/queues/{queue}:sync`, `…/prs/{number}:cancel`, `…/prs/{number}:restart` and
  `…/prs:batchGet`, the custom methods of the Google API design guide.
- **Why not**: axum routes whole segments only. A handler took every `POST` on the queue or the PR and cut the
  segment by hand, a wrong method answered `404` instead of `405`, and the paths of the `OpenAPI` document were
  not those of the router. An action is a segment of its own: `…/sync`, `…/prs/{number}/cancel`.

### An action next to the identifiers of a collection

- **Tried / considered**: `POST /orgs/{org}/repos/resync` and `POST /orgs/{org}/setups/resync`, to read again
  the setups of the repositories of an organization.
- **Why not**: a repository can be named `resync`: the action takes the place of an identifier, and hides the
  route of that repository. An action comes after the identifier of the resource that it acts on
  (`…/repos/{repo}/sync`), or under a segment that names no identifier (`/user/sync`).

### `@me` for the person of the session in a filter

- **Tried / considered**: `GET /orgs/{org}/prs?author=@me`, with `400 me_needs_session` for a token.
- **Why not**: the syntax comes from the search of another product, and the filter then reads differently for a
  session and a token. The dashboard knows the login of its person (`GET /user`) and passes it.

### Filters of a repository on a list of the organization

- **Tried / considered**: `GET /orgs/{org}/prs?repo=web&queue=main` and `GET /orgs/{org}/queues?repo=web`, with
  `400 queue_needs_repo` when `queue` came without `repo`.
- **Why not**: two routes gave one list, and the access to a repository was checked in a filter instead of the
  path. What belongs to a repository is under it (`/orgs/{org}/repos/{repo}/prs?queue=main`); a list of the
  organization only searches across its repositories.

### A counter of failures that keeps the failure of a PR that came back

- **Tried / considered**: `failed_last_24h` counted every item that reached `failed` in the 24 hours, the earlier
  submissions of a PR too, so that a failure stays visible after the PR comes back.
- **Why not**: the list of the PRs gives only the current item of each PR. The dashboard showed "Failed in 24 h 3"
  above a tab of 2 failures, and nothing on the page said which failure was missing. A counter counts what its list
  shows; the history of the PR keeps its earlier failures.

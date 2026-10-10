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

## Process and binaries

### Two binaries, and a selection of the roles per process

- **Tried / considered**: a server binary and a separate CLI; `CIHUB_ROLES` or a role argument to run only some
  of the four roles in a process.
- **Why not**: identical replicas already share the work through leases, locks and idempotency keys. One binary
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

### Route a submission with `queue_conditions`

- **Tried / considered**: pick the first queue of the branch whose `queue_conditions` pass.
- **Why not**: routing reads the PR once and an item never changes queue, but `queue_conditions` change over time
  (checks, reviews): a PR that does not pass them yet at submission would go to another queue and stay there.
  `route_conditions` route; `queue_conditions` admit.

## GitHub

### An App creation wizard (manifest flow)

- **Tried / considered**: a `/setup` page that creates the GitHub App and shows its secrets once.
- **Why not**: CIHub is not designed for a single SCM, the wizard needed a public URL and state cookies, and a
  manual creation from `specs/GITHUB.md` is enough.

### Webhooks as the source of truth for installations, and a bootstrap command

- **Tried / considered**: create nothing from webhooks, and create organizations with `cihub server bootstrap`.
- **Why not**: webhooks get lost. CIHub reads the installations of its App on the SCM (at start, daily, and on
  each signal) and converges `scm_connections`; `cihub server token create` only makes a token.

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

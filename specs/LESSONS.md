# Lessons

What we tried or considered, and why we do not do it. Read it before you propose a change in the same area. An
entry says the rejected approach, the reason, and, when known, what would change it. Add one when an approach fails
for a reason that the code cannot show; remove one when its reason no longer holds. An ID is never reused.

## Language and dependencies

- **L-DEP-1** Not `jsonwebtoken` nor `hmac`: they bring `aws-lc-rs` (C code) or the `rsa` crate
  (RUSTSEC-2023-0071). `ring`, already present through sqlx and rustls, signs the App JWT and checks webhooks.
- **L-DEP-2** Not `serde_yaml` (archived, misplaces errors in a list) nor `serde_yml` (RUSTSEC-2025-0068): the
  position of an error is part of the product, and `serde-saphyr` gives it.
- **L-DEP-3** No full GitHub SDK: every call goes through the budget of the connection, and the few endpoints fit
  in a small client on `reqwest`.

## Tooling, CI and release

- **L-CI-1** Not the `cargo:` backend of mise: it compiles each tool in each CI job. Use `aqua:` or `github:`;
  `cargo:` only for a tool that publishes no binary.
- **L-CI-2** Not the `rust` type of release-please: it does not handle a workspace root without `[package]`. The
  `simple` type with `extra-files` updates the versions.
- **L-CI-3** No self-hosted runner: the repository is public, and such a runner runs the code of forks.
- **L-CI-4** The chart jobs run on every pull request, not only on the chart paths: `Smoke` also checks the image,
  the migrations and the configuration, which any change can break.
- **L-CI-5** One `compose.yaml` with profiles serves development, and the chart (`helm/cihub`) serves production:
  a second compose file under `deploy/` drifted.
- **L-CI-6** No `${VAR:?}` nor bind-mounted key in `compose.yaml`: Compose reads every variable before it selects
  the profiles, so `docker compose up postgres` stops too. Would change if Compose skips the inactive profiles.

## Process and binaries

- **L-PROC-1** The server and the command line are two binaries: the server doubled the crates of the command
  line (221 instead of 116) that users install in their CI. Would change if the command line needs server code.
- **L-PROC-2** Every process runs the four roles, with one file per role in `crates/server/src` and no crate per
  role: replicas share the work through leases, locks and idempotency keys. Would change on a measured need.
- **L-PROC-3** No trait for the inbox, the jobs or the outbox: they write in the caller's transaction, which no
  other implementation could join. Would change if a second implementation arrives.

## Postgres

- **L-PG-1** No message broker: `LISTEN/NOTIFY`, `SKIP LOCKED` and leases are enough. Would change if a
  measurement shows that Postgres is the limit.
- **L-PG-2** A lease alone does not give a queue to one scheduler: two replicas can reconcile during a takeover.
  Each reconciliation also takes `pg_try_advisory_xact_lock`.
- **L-PG-3** No expression index on `payload->>'run_id'`: Postgres ignores its statistics in a partial index.
  `events` has a real `run_id` column.
- **L-PG-4** No init script in the Postgres container: it runs only on an empty volume. `postgres-setup` creates
  what is missing at each start.
- **L-PG-5** `last_used_at` of a token advances at most once per minute, so that a read does not write.
- **L-PG-6** Under row level security, a `state` filter uses literals (`ItemState::as_str`) and partial indexes,
  not a bind parameter nor an index column: an enum equality is not `LEAKPROOF`, so it applies after the policy
  (140 ms to 2 ms on 300,000 items).

## Event log

- **L-LOG-1** No separate outbox: `events` is the outbox, and its `seq` has no gap because only the lease owner
  writes the log of a queue.
- **L-LOG-2** A partition of `events` drops only when each of its queues has a snapshot at a `seq` as high: else
  an inactive queue loses its log and restarts at `seq` 1.
- **L-LOG-3** The log keeps the user ID, not the login, so that a rename or an anonymization touches one row.

## Jobs and inboxes

- **L-JOB-1** An inbox holds only work to do. CIHub answers 2xx only after it stores a delivery, so the sweep asks
  again for the deliveries without a 2xx attempt.
- **L-JOB-2** No heartbeat for a job lock: the worker stops an executor after 50 s, before its 60 s lock expires.
- **L-JOB-3** Only a 422 of `assemble_test_branch` counts toward its 5 attempts: 5xx, 429 and stops are not the
  fault of the assembly.
- **L-JOB-4** No fixed scheduler tick: a `wake_at` was up to 30 s late. The scheduler waits until the earliest
  `next_tick_at`.

## Queue algorithm

- **L-ALG-1** No lenient optimistic merge: the groups behind an evicted PR tested code that will not land.
- **L-ALG-2** No eviction on a failure that the bisection infers without a run: a flaky test can explain it.
- **L-ALG-3** A conflict at the assembly of a group of several PRs does not fail them all: the bisection finds it.
- **L-ALG-4** A batch that cannot grow (a `no_batch` item behind it) starts at once, without `max_wait_minutes`.
- **L-ALG-5** The simulator cannot show `batched` saving time (its CI has no runner limit): `mise run rust:load`
  compares the runs per merged PR instead.
- **L-ALG-6** No `fast_forward` with a strategy other than `sequential`: the test commit of a PR does not descend
  from that of its predecessor.
- **L-ALG-7** `trigger: inplace` needs more than removing its refusal: an exemption when the head is the test SHA,
  a `sha` guard of the merge, formation without a live predecessor, and checks routed by SHA.
- **L-ALG-8** No `paused` state for a lost access: one state for two causes loses the choice of the admin (a
  pause undone by `access.restored`, a drain turned into `running`, a resume without the access). The state keeps
  the choice of the admin, and `is_access_lost` of the snapshot stops the queue apart from it.

## Configuration

- **L-CFG-1** One queue per branch: two trains on one branch do not test on top of each other, and each merge of
  one is an outside push for the other. `priority_rules`, `--no-batch`, profiles and lanes answer in one train.
- **L-CFG-2** A profile is chosen by `conditions`, not by `queue_conditions`, which change over time; it sets only
  the settings of a PR, not of the train.
- **L-CFG-3** Conditions are typed YAML maps, not strings of a small language: that syntax belongs to another
  product, needs its own parser, and has no schema for an editor or the dashboard form.

## GitHub

- **L-GH-1** No App creation wizard: it needs a public URL and ties CIHub to one SCM. The App is created by hand
  (`specs/GITHUB.md`); the command line talks only to the API of an instance.
- **L-GH-2** Webhooks are not the source of truth for installations: they get lost. CIHub reads the installations
  of its App and converges `scm_connections`.
- **L-GH-3** No device flow of the App nor copied API token for the command line: one makes it know GitHub, the
  other stands for an organization, not a person. Would change if no browser reaches the dashboard.
- **L-GH-4** The `/device` page does not take the code in its address: an attacker sends the link, and one click
  approves their session. Would change for a device without a keyboard.
- **L-GH-5** CIHub keeps the encrypted token of the person and reads with it what the person reaches: with the role
  alone, a member reads a private repository that GitHub hides from them.
- **L-GH-6** The permissions of a person outlive the sign-out for 14 days, so that the dashboard does not open
  empty during `sync_user`; the token does not. Would change if a list must never show a lost repository.
- **L-GH-7** A `blocked` PR whose only unmet check is `CIHub Merge Queue (<branch>)` is `mergeable`: the queue
  itself blocks it.
- **L-GH-8** Not `merge-async` by default: GitHub Enterprise Server (3.22) lacks it. The adapter uses it only on a
  403 that names it.
- **L-GH-9** A comment or label delivery does not give the queue nor the actor: `submit_pr` reads the PR, and a
  label submission has the `system` actor.
- **L-GH-10** No read of the checks of every live run to show a running check: the `in_progress` action of
  `workflow_job` gives it without budget.

## PR surfaces

- **L-PR-1** No checkbox for `restart` or `cancel`: a click has no confirmation and affects the PRs behind.
- **L-PR-2** A waiting batch shows an absolute UTC time: the comment is not rewritten every minute.
- **L-PR-3** No welcome on every open PR when a queue starts: hundreds of comments at once.
- **L-PR-4** No `@login` in the comment history: each rewrite would notify.
- **L-PR-5** A failed acknowledgement of a command is a `warn` log: a retry would give its facts twice.
- **L-PR-6** A job does not write its result in a column of `queues`: the lease takes their rows with `SKIP
  LOCKED` and skips a queue that a job holds. A job that must write `queues` calls `mark_dirty`.

## API

- **L-API-1** No action after a colon (`…:sync`): axum routes whole segments. An action is a segment of its own.
- **L-API-2** No action beside the identifiers of a collection (`/repos/resync`): it hides a repository of that
  name. An action comes after the identifier that it acts on.
- **L-API-3** No `@me` in a filter: the syntax of another product, read differently for a session and a token.
- **L-API-4** What belongs to a repository is under its path, not a filter of an organization list.
- **L-API-5** A counter counts what its list shows; the history of the PR keeps its earlier failures.

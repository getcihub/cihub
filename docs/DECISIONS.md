# Decision log

The decisions in force, append-only (D164). The mentions "§4" and "§5" in the first decisions refer to the
sections of the old `AGENTS.md`: the validation rule (§4) is below; the working rules (§5) are in `AGENTS.md`.

**A decision is validated before it is applied.** An agent proposes each choice of architecture, dependency,
crate, vocabulary, public format or name to the user, or to the person who develops the feature, and waits for
their agreement before writing or applying it. An agent never adds a decision alone.

**The decision log is append-only.** Each decision has a stable identifier (`Dn`), a date and the name of the
person who validated it. A decision is never modified or deleted: to change it, a new decision is added that says
what it supersedes, and the old one gains the mention _Superseded by Dn_ (superseded-by).

**September 29, 2026, validated by Nicolas Lecoy.** The project moves from Go to Rust; the log restarts at D1.

- **D1** The code is the source of truth. The documents state the intent, the rules and the shared vocabulary,
  and can lag behind. `docs/DOMAIN.md` is the vocabulary contract: it defines each term and points to the code
  for the list of values. What is not coded yet is described in the lot that builds it (`docs/LOTS.md`), not in
  `docs/DOMAIN.md`.
- **D2** The list of dependencies is `[workspace.dependencies]` in `Cargo.toml`. A new external dependency is
  validated according to §4.
- **D3** Each lot has a status (`to do`, `in progress`, `done`, `blocked` with the cause) and, as soon as it
  starts, a "Tracking" block (done, remaining, deviations from the plan) in `docs/LOTS.md`, updated in the same
  change as the code.
- **D4** Identifiers are typed ULIDs in the `typedid` text format (`org_0627ank54ajth78mw4353dqpw5dvt5kp`), in
  `cihub-domain`.
- **D5** The log of a queue contains facts and decisions; the replay applies the events with the pure function
  `apply` and never calls `plan`; the identifiers of decisions are deterministic ULIDs. The detail is the target
  of Lot 1 (`docs/LOTS.md`).
- **D6** A single `Reason` enumeration and a table of allowed transitions, whose key is (source state, target
  state) and whose value is the list of allowed reasons, for items, groups, runs and the queue. It replaces the
  reason enumerations per target state (Lot 1).
- **D7** No variant for stacked PRs (`ConflictCulprit::StackedPr`, `stacked_pr_*`) before the stacked PRs lot.
- **D8** Two binaries. `cihub-server` (`crates/server/`) carries the four roles, all-in-one by default;
  `CIHUB_ROLES` (`api`, `scheduler`, `worker`, `notifier`, comma-separated) selects those of the process. No crate
  per role: a role starts loops whose code is in the library crates. The roles of one process share only the
  Postgres pool. `cihub` is the CLI, distributed alone, without server code.
  _The `api` role is renamed by D91._ _Superseded by D168._
- **D9** A crate is created when a lot needs it, after validation. `docs/ARCHITECTURE.md` describes the crates
  that exist and the target list. The skeleton of `cihub-server` arrives in Lot 2.
- **D10** Tasks are in `mise.toml` at the root; a task exists only when it has something to run (§5). The CI
  calls the `mise` tasks, without repeating their commands.
- **D11** Pure crates have a line coverage of at least 90%, measured by `cargo-llvm-cov` (`mise run cover`) and
  checked in CI.
- **D12** `compose.yaml` at the root starts only Postgres (`postgres:17.5-alpine`) for development, published on
  `127.0.0.1`, with a development password written in the file. _Extended by D52._
- **D13** Logs use `tracing` and `tracing-subscriber`, in JSON by default or in `pretty`, with the correlation
  keys carried by the spans; secrets are in `secrecy::SecretString`. The format and the rules are in
  `docs/OBSERVABILITY.md`.
- **D14** Migrations are explicit. `cihub-server migrate` applies them then stops; at startup, `cihub-server`
  applies nothing and stops if an embedded migration is missing from the database. A dedicated process runs them
  before the roles: `mise run migrate` in development, a `cihub-migrate` service in the installation
  `docker compose`, a `Job` as a Helm `pre-install` and `pre-upgrade` hook on Kubernetes. _Extended by D168._
- **D15** The Rust practices of §5 (errors, lints, tests, async, dependencies, code documentation).
- **D16** (Q11) The clock and randomness are forbidden everywhere by `disallowed-methods` in the root
  `clippy.toml` (`SystemTime::now`, `Instant::now`, `OffsetDateTime::now_utc`, `Ulid::generate`,
  `Generator::generate`). The only place that reads them carries a local
  `#[expect(clippy::disallowed_methods, reason = "…")]`.
- **D17** The whole toolchain is installed by `mise`: Rust (pinned version, `minimal` profile, components
  `clippy`, `rustfmt` and `llvm-tools`) and the tools of `mise.toml`. `mise.lock` pins the versions and the
  checksums per platform; the CI installs with `--locked`.
- **D18** The CI follows the organization of `home-operations/external-dns-unifi-webhook`: one job per `mise`
  task, a `Workflow Lint` job (`actionlint`, `zizmor`), and a `Build Success` job that gathers them and serves as
  the required status. Each action is pinned by the SHA of its commit, with the version as a comment
  (`actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1`).
- **D19** Versions are published by release-please (`release-please-config.json`,
  `.release-please-manifest.json`), from commit titles in the Conventional Commits format. Type `simple`: the
  `rust` type does not handle a workspace whose root has no `[package]`. release-please updates
  `[workspace.package].version` in `Cargo.toml`, the versions of the `cihub-*` crates in `Cargo.lock` and
  `CHANGELOG.md`; tags are `vX.Y.Z`. The workflow uses the project's bot GitHub App (secrets `BOT_CLIENT_ID` and
  `BOT_APP_PRIVATE_KEY`). The image and its publication workflow arrive in Lot 9.
- **D20** Extends D16: the identifier constructors (`OrgId::new()` and the others) are also forbidden by
  `disallowed-methods`; they carry the local exception and have no `impl Default`. Tests use fixed identifiers
  (`OrgId::from_ulid(Ulid(1))`).
- **D21** Renovate runs in this repository through `.github/workflows/renovate.yaml`, copied from
  `nlecoy/home-ops`, with the bot GitHub App, and can run `mise` to update `mise.lock`. Its configuration
  `.renovaterc.json5` is specific to the project (no `home-operations` preset): actions pinned by SHA with the
  version as a comment, Conventional Commits titles, groups (GitHub actions, `mise` tools, Rust toolchain), a
  3-day wait after a release, monthly lockfile updates. No automatic merge: a person merges each update.
- **D22** Extends D17: a tool in `mise.toml` uses a backend that downloads a precompiled binary (`aqua:`,
  `github:`), with its checksum in `mise.lock`. The `cargo:` backend, which compiles the tool from source in each
  CI job, is used only when the tool publishes no binary.
- **D23** The log uses the vocabulary of CIHub, not that of an SCM. Each adapter translates the values of its SCM
  into neutral facts; the core sees no GitHub-specific value. "PR" designates any change request. `Mergeability`
  is `mergeable`, `behind`, `conflict`, `blocked` or `unknown`; the merge failure reasons are `merge_rejected`,
  `direct_merge_rejected`, `merge_rate_limit_exceeded`, `merge_branch_protection_unmet`,
  `merge_already_in_progress` and `merge_repeated_server_errors`; the loss of access to the repository is an
  `access.lost` or `access.restored` fact; `CheckKind` is `check` or `status`. Trunk names stay in the outgoing
  webhooks, through a translation in Lot 11. _Partly superseded by D139: no Trunk names in the outgoing
  webhooks._
- **D24** The pure crate `cihub-mq-core` (`crates/mq-core/`) contains `apply`, the merge graph, `plan` and
  `check`. It depends on `cihub-domain` and knows neither `cihub-config` nor `cihub-conditions`.
  _Extended by D99: it depends on `cihub-config`._
- **D25** The queue settings (`QueueSettings`) are a parameter of `plan` and `check`, like the time:
  `plan(snapshot, settings, now)` and `check(snapshot, settings)`. They are not in the snapshot, because
  `cihub-domain` cannot depend on `cihub-config`. The `queue.settings_changed` fact carries only the mode before
  and after.
- **D26** `thiserror` for the errors of library crates (§5.1); `proptest` for property tests, as a development
  dependency only. `insta` arrives with the first lot that compares plans by snapshots. _Extended by D97: Lot 5._
- **D27** (Q6) Probabilistic speculation stays out of v1. The `item.submitted` fact keeps from now on the
  attributes it will need: `author`, `additions`, `deletions` and `changed_files`. Durations come from the
  timestamps of the log; they are not an attribute. _Extended by D42: `author` becomes `author_id`._
- **D28** (Q10) The queue tables are `queue_items`, `test_groups` and `test_runs`, without a module prefix.
- **D29** Lot 2 creates the crates `cihub-server` and `cihub-store`.
- **D30** Dependencies of Lot 2: `tokio`, `tokio-util`, `sqlx` (Postgres, `rustls`), `axum`, `anyhow` (binaries
  only), `tracing`, `tracing-subscriber`, `secrecy`, `sha2` and `rand`. Any other dependency is proposed
  according to §4.
- **D31** All the environment variables of the server start with `CIHUB_`. _Extended by D168._
- **D32** An API token is `cht_` followed by 20 random bytes encoded in base32 with the typedid alphabet (36
  characters in total), sent by `Authorization: Bearer`. The server keeps only the SHA-256 fingerprint of the
  token and finds it by this fingerprint. `cht` is never a typedid identifier type.
- **D33** The API is served under `/api/v1`, on the address of the instance, next to the dashboard.
- **D34** Extends D29: Lot 2 also creates the crate `cihub-telemetry` (`crates/telemetry/`), which installs the
  logs (D13), then the OpenTelemetry traces and the Prometheus metrics when they arrive. Only the binaries use
  it. In `pretty`, a log is one line: UTC time in milliseconds, level, message and fields.
- **D35** The `Module` contract and the activation of a module per organization arrive in Lot 5, with the first
  module (`cihub-mergequeue`), and not in Lot 2. _Extended by D94._
- **D36** The `events` table is partitioned by month. `cihub-server migrate` creates the partitions of the current
  month and of the next month; a loop of the `scheduler` role creates every day the partition of the next month
  if it is missing, until a job replaces it in Lot 3. No extension (`pg_partman`) and no default partition.
- **D37** Identifier columns are `text` and keep the typedid form (`org_0627…`, D4).
- **D38** A single Postgres role, the one of `CIHUB_DATABASE_URL`, owns the tables and runs the queries. Each
  table that carries `org_id` (and `orgs`) has row level security, forced also for the owner. `cihub-store` opens
  a transaction in one of two contexts: `begin_org(org_id)` (parameter `cihub.org_id`) sees one organization,
  `begin_system()` (parameter `cihub.system`) sees them all; a transaction without context sees nothing. The role
  is neither superuser nor `BYPASSRLS`: `cihub-server` refuses to start otherwise. The development `compose.yaml`
  creates this role (`cihub`) next to the `postgres` superuser.
- **D39** The `users` table is global, without `org_id` or row level security: a GitHub user belongs to several
  organizations through `memberships`. _Extended by D51._
- **D40** New closed values: `MembershipRole` (`admin`, `member`, `viewer`), `AppKind` (`main`, `sudo`),
  `ApiScope` (`queues:read`, `queues:write`, `prs:write`, `targets:write`, `metrics:read`). New identifier types:
  `installation`, `user`, `apitoken` (the token row; the secret stays `cht_…`, D32), `audit`.
  _`AppKind` and the `installation` type superseded by D49._
- **D41** A migration file is named `<YYYYMMDDHHMMSS>_<description>.sql` (format of `sqlx migrate add`).
- **D42** Extends D27: the log keeps no login. A user actor is `Actor::User { user_id }`, stored in
  `events.actor_kind` (`system`, `bot`, `user`) and `events.actor_user_id`, which points to `users`;
  `item.submitted` carries `author_id` and `target.pushed` carries `sender_id`. `users` keeps the login: a rename
  or an anonymization changes one row, never the log. Ingestion creates or updates the `users` row before writing
  the event (Lots 4 and 5). _Extended by D51._
- **D43** `events` table: `id`, `org_id`, `queue_id`, `seq`, `type` (enum `event_type`), `actor_kind`,
  `actor_user_id`, `cause_id`, `item_id`, `group_id`, `run_id`, `payload` (the fields of the type, serialized by
  `cihub-domain`) and `occurred_at` (to the microsecond); primary key (`id`, `occurred_at`). `seq` is the rank in
  the log of the queue, starting at 1: `cihub-store` computes it at write time, and the caller holds the lock of
  the queue (Lot 3). One partition per UTC month, `events_<YYYY>_<MM>`, with its own forced policy. The deletion
  of partitions older than 90 days arrives in Lot 3, with the jobs.
- **D44** The outbox, the subscriptions and the criterion "an event and its outbox entry are written in the same
  transaction" move from Lot 2 to Lot 3, with the `Outbox` trait. _Extended by D62: no trait. Superseded by
  D65._
- **D45** API tokens are in `cihub-store` (creation, authentication by fingerprint in the system context);
  `cihub-domain` carries their text form, without randomness. The secret comes from the system random generator
  (`SysRng`), forbidden elsewhere by `clippy.toml`. `last_used_at` advances at most once per minute, so that an
  API call does not write to the database.

**September 30, 2026, validated by Nicolas Lecoy.**

- **D46** The data model depends on no SCM, like the log (D23). An SCM identifier is an `external_id` column in
  `text`, unique within its SCM instance only: each uniqueness key carries `scm_instance_id`.
- **D47** Table `scm_instances` (`id`, `kind`, `base_url` unique): github.com or a GitHub Enterprise Server. It is
  global, without `org_id` or row level security, because organizations share an instance. Closed value `ScmKind`
  (`github`); identifier type `scminstance`. The migration creates no instance: the setup wizard creates it
  (Lot 4), because a GitHub App belongs to a single instance.
  _Extended by D92: `cihub-server` creates it at startup, from the App configuration (D79)._
- **D48** `orgs` no longer has an SCM-specific column or a `settings` column. Table `scm_accounts` (`id`,
  `org_id`, `scm_instance_id`, `external_id`, `path`), unique (`scm_instance_id`, `external_id`): the SCM account
  (a GitHub organization) attached to an organization, which can have several. Identifier type `scmaccount`.
- **D49** Table `scm_connections` (`id`, `org_id`, `scm_instance_id`, `scm_account_id`, `kind`, `external_id`,
  `suspended_at`, `claimed_at`), unique (`scm_instance_id`, `kind`, `external_id`): the means by which CIHub
  reaches the repositories of an account. It replaces `installations`. Closed value `ConnectionKind`
  (`github_app`, `github_sudo_app`) instead of `AppKind`; identifier type `connection` instead of
  `installation`. The budget, the quotas and the degraded modes are per connection.
- **D50** `repos`: `scm_instance_id`, `connection_id`, `external_id`, `namespace`, `name`, `default_branch`,
  unique (`scm_instance_id`, `external_id`).
- **D51** Extends D39 and D42: a `users` row is a user account of an SCM instance (`scm_instance_id`,
  `external_id`, `login`), unique (`scm_instance_id`, `external_id`). The accounts of one person are separate
  rows, without a link; a table of persons can group them later without changing the log, which points to the
  account that acted.
- **D52** Extends D12: `compose.yaml` adds a one-shot service `postgres-setup`, which brings the `cihub` role and
  the `cihub` database to the desired state at each startup (creation if they are missing, then `ALTER`). It
  replaces the initialization script, which ran only on an empty volume. `mise run db` starts Postgres then runs
  this service; `mise run db-reset` erases the data then runs `mise run db`.
- **D53** Lot 3 creates the crate `cihub-platform` (`crates/platform/`). It depends on `cihub-domain` and
  `cihub-store`; the dependency on `cihub-scm` arrives in Lot 4.
- **D54** A queue is assigned to a scheduler replica by a lease in the `queue_leases` table (owner, expiration at
  15 s), and each reconciliation additionally takes `pg_try_advisory_xact_lock` on the queue, which guarantees a
  single reconciliation at a time, even during the takeover of an expired lease.
- **D55** Worker replicas share the jobs through `SELECT … FOR UPDATE SKIP LOCKED`; `locked_until` gives the job
  of a stopped replica to another one.
- **D56** Lot 3 creates a minimal `queues` table (`id`, `org_id`, `repo_id`, `target_branch`, `dirty`,
  `next_tick_at`) and the foreign key from `events.queue_id` to `queues`. Lot 5 adds the state, the mode and the
  settings of the queue.
- **D57** Extends D36 and D43: Lot 3 replaces the scheduler loop with a job that creates the `events` partitions.
  The deletion of partitions older than 90 days arrives in Lot 5, with the snapshots: without a snapshot, an
  inactive queue would lose its log and its `seq` would restart at 1. _Extended by D67._
- **D58** Extends D49: `jobs` and `webhook_inbox` carry `connection_id`, which carries the quotas and the budget.
- **D59** Closed value `JobClass` (`interactive`, `merge`, `speculate`, `repair`), in `cihub-domain` and as a
  Postgres enum.
- **D60** An `idempotency_key` has the form `<job type>:<subject>:<version>` (for example `merge_pr:item_…:1`)
  and is unique in `jobs`. The exact grammar is proposed with the first job type. _Extended by D68._
- **D61** Lot 3 adds no external dependency as long as `sqlx` is enough (`PgListener` for `LISTEN/NOTIFY`).
- **D62** The inbox, the jobs and the outbox have no trait: they are functions of `cihub-platform` on a Postgres
  connection, and their signatures are the frozen interfaces of Lot 3. Marking a delivery as processed, writing
  an event and its outbox entry, or emitting the jobs of a reconciliation happens in the caller's transaction,
  which no other implementation could join. A trait will arrive with a second implementation, not before.
- **D63** No NATS in v1: Postgres is the only transport between the roles. Another transport dependency requires
  a measurement and a decision.

- **D64** The scheduler loop, which assembles the leases, the wake-up and the transaction of a queue, arrives in
  Lot 5 in `cihub-mergequeue`, with the reconciliation. Lot 3 delivers its parts in `cihub-platform`.
- **D65** Supersedes D44: the outbox, the subscriptions and the criterion "an event and its outbox entry are
  written in the same transaction" move to Lot 11, the first lot that consumes them (outgoing webhooks,
  ClickHouse).
- **D66** An executor is the code of a job type; the worker has a registry, from job type to its executor. An
  executor answers: done, to redo later (with the error), or definitive failure (with the error). Job types and
  their executors are neutral: an executor calls the SCM through the `Scm` trait (Lot 4), according to the
  connection of the job; SCM-specific code stays in its adapter. The `Module` contract (Lot 5) declares its
  executors in this registry.
- **D67** Extends D36 and D57: a daily `maintenance` job creates the `events` partitions of the month and of the
  next month, deletes the deliveries processed more than 7 days ago and the jobs finished more than 3 days ago.
  Each scheduler replica requests it every hour with the key of the day (`maintenance:<YYYY-MM-DD>:1`): the key
  admits only one job per day, which a single worker runs. It replaces the daily partition loop.
- **D68** Extends D60: in an `idempotency_key`, the type is in `snake_case`, the subject contains no `:`, and the
  version is an integer starting at 1.
- **D69** The worker stops an executor after `JOB_TIMEOUT` (50 s), shorter than `JOB_LOCK` (60 s), and puts its
  job back in the wait state with the backoff. The lock of a job is not renewed. An executor accepts being
  stopped at each `.await` (principle III).
- **D70** A 429 pauses the jobs of a connection: a `paused_until` column on `scm_connections`, which
  `claim_jobs` respects. The SCM adapter writes the pause (Lot 4); `Outcome` does not change.
- **D71** An executor receives the SCM through its closure (`Arc<dyn Scm>` captured at registration): the
  signature of `Executors::register` does not change.
- **D72** (Q5) The `actions: write` permission is a checkbox of the wizard, unchecked by default. Without it,
  CIHub cancels no CI and restarts no job. _Extended by D92: an optional permission, checked by hand._
- **D73** Lot 4 creates three crates: `cihub-scm` (`Scm` trait, neutral types, and the GitHub simulator in a
  module behind the `sim` feature, used only as a development dependency), `cihub-github` and `cihub-http`. No
  `cihub-github-sim` crate. `cihub-http` depends on `cihub-api-types` from Lot 9 on.
  _`cihub-http` is renamed by D91._
- **D74** Dependencies of Lot 4: `reqwest` (`rustls`), `jsonwebtoken` (App JWT) and `hmac` (webhook signature).
  The recorded GitHub responses are JSON files replayed by a test `axum` server, without a cassette library.
  _Partly superseded by D80._
- **D75** Lot 4 is not split. It stops at the delivery stored, verified and routed to its queue, and at the
  translation of a delivery into a `ScmEvent`, a neutral event defined in `cihub-scm` (PR changed, push, check
  changed, comment, access lost or restored, installation created). Lot 4 writes no fact in the log: in Lot 5,
  the scheduler loop reads the inbox of a queue, translates the `ScmEvent`s into facts and marks the delivery
  processed in the same transaction (D43, D62, D64).
- **D76** The `Scm` trait has one implementation per SCM instance. Each method receives the connection, the
  repository and a `JobClass` (D59), which decides what the budget lets through. Lot 4 freezes the methods of
  Lots 5 to 7: reads (grouped state of at most 50 PRs, one PR, head of a ref, checks of a SHA, required checks,
  content of a file, permission of a user) and writes (create, move or delete a ref, merge a commit into a ref,
  merge a PR, publish a status, create or edit a comment, add or remove a label, open or close a draft PR). The
  errors are a neutral `ScmError` enumeration. The trait is used as `Arc<dyn Scm>` (D71), without the
  `async-trait` crate.
- **D77** The budget and the mode of a connection are columns of `scm_connections`: `mode`, `mode_reason`,
  `rate_remaining`, `rate_limit`, `rate_reset_at`, `paused_until` (D70) and `consecutive_failures`. Closed value
  `ConnectionMode` (`normal`, `throttled`, `scm_unavailable`, `paused`). A process keeps the rate limit headers
  in memory and writes them at most every 5 s; a 429 or a mode change is written immediately. Installation
  tokens stay in memory.
- **D78** Extends D23: the whole vocabulary of CIHub is neutral (states, modes, reasons, facts, columns,
  configuration keys, routes, metrics). An SCM name appears only in its adapter (`cihub-github`), its environment
  variables and `docs/GITHUB.md`, and in the values that name an SCM (`ScmKind`, `ConnectionKind`). The
  `github_unavailable` mode becomes `scm_unavailable`.
- **D79** The secrets of the GitHub App come from the environment: `CIHUB_GITHUB_URL` (`https://github.com` by
  default), `CIHUB_GITHUB_APP_ID`, `CIHUB_GITHUB_PRIVATE_KEY` (or `CIHUB_GITHUB_PRIVATE_KEY_FILE`) and
  `CIHUB_GITHUB_WEBHOOK_SECRET`. The wizard creates the App through the manifest flow and shows its secrets and
  the block to copy only once; CIHub writes them nowhere. At startup, `cihub-server` creates the `scm_instances`
  row of `CIHUB_GITHUB_URL` if it is missing (D47). The `cihub-sudo` App and its variables arrive with the
  direct-to-queue merge. _Extended by D92: no wizard, the App is created by hand._
- **D80** Supersedes `jsonwebtoken` and `hmac` in D74: `ring`, already present through sqlx, signs the App JWT in
  RS256 and computes the HMAC-SHA256 of the webhooks; `cihub-github` assembles the JWT and reads the PEM key with
  `base64`. `reqwest` uses `rustls-no-provider` with the `ring` provider of rustls. A single crypto library,
  without C code: `jsonwebtoken` offers only `aws-lc-rs` or RustCrypto, whose `rsa` crate has a security advisory
  on signing (RUSTSEC-2023-0071).
- **D81** `cargo deny` accepts the `CDLA-Permissive-2.0` license for the single crate `webpki-root-certs`: the
  list of Mozilla root authorities, which `rustls-platform-verifier` embeds for `reqwest`. It is a data license,
  permissive; its redistribution requires the license text in the notices of the image (Lot 9).
- **D82** The public address of the server is `CIHUB_HTTP_ADDR` (`0.0.0.0:8080` by default): it serves the
  webhooks, the wizard and, later, the API. `CIHUB_INTERNAL_ADDR` keeps `/healthz` and `/readyz`.
  `CIHUB_PUBLIC_URL` is the URL of the instance as seen from the Internet; the wizard needs it for the App
  manifest. _Extended by D92: no wizard, so no `CIHUB_PUBLIC_URL`._
- **D83** GitHub webhooks arrive on `POST /webhooks/github`, outside `/api/v1`. The route belongs to the adapter
  (D78).
- **D84** The webhook handler verifies the signature, stores the delivery without routing it and responds. A loop
  of the `api` role reads the unrouted deliveries, translates them into `ScmDelivery`, routes them, writes the
  routed row and marks the queue (`mark_dirty`). _Extended by D91: the `web` role._
- **D85** Routing is partial: it fills `org_id` and `connection_id` as soon as the connection is known, and
  `queue_id` only when a single queue is certain (PR with its base branch, push on a target branch, check of a
  test branch). Lot 5 reads the deliveries of its queue and those of its connection without a queue.
- **D86** A delivery whose connection is not known (`installation.created` before any organization) stays
  unrouted, visible from the system context only. Lot 4 creates no `orgs`, `scm_connections` or `repos` row from
  a webhook; its tests create these rows directly.
- **D87** The configuration checker moves to Lot 9, with the API, the known repositories and a method of the
  `Scm` trait that reads the rulesets.
- **D88** Extends D76: the `Scm` trait gains two methods at the instance level, without a connection: read the
  recent deliveries and redeliver a delivery. A `sweep_deliveries` job of class `repair`, requested by each
  scheduler replica every 5 minutes with the key of the window
  (`sweep_deliveries:<scm_instance_id>-<YYYYMMDDTHHMM>:1`), redelivers the deliveries absent from the inbox.
- **D89** The wizard (`/setup`) responds only when no App is configured; the server therefore starts without the
  GitHub variables. It links the return from GitHub to the browser that started the flow by a random `state`
  parameter, kept in an `HttpOnly` and `SameSite=Lax` cookie, and refuses a return whose `state` differs. The
  secrets page carries `Cache-Control: no-store`. _Superseded by D92._
- **D90** The recorded GitHub responses (Lot 4) come from the `github` example in `--record` mode: it prepares
  the test repository (branches, file, mergeable, conflicting and draft PRs), waits for the approval of a second
  account, then plays the contract suite and writes each response in `crates/github/tests/recorded/`, without a
  secret.
- **D91** Extends D8 and D73: the `api` role becomes `web`, and the `cihub-http` crate becomes `cihub-web`
  (`crates/web/`). The role serves the public address: the API, the dashboard, the webhooks and the wizard; a
  narrow name like `api`, or `server`, already the name of the binary, would be confusing. The routes stay in a
  library, so that its tests call the router and the modules declare their routes there (D8).

**October 1, 2026, validated by Nicolas Lecoy.**

- **D92** Supersedes D89, and extends D47, D72, D79 and D82. No App creation wizard: CIHub is not designed for a
  single SCM, and a manual creation is enough. The operator creates the GitHub App according to `docs/GITHUB.md`
  (permissions, events, webhook URL) and provides `CIHUB_GITHUB_APP_ID`, the private key
  (`CIHUB_GITHUB_PRIVATE_KEY` or `CIHUB_GITHUB_PRIVATE_KEY_FILE`) and `CIHUB_GITHUB_WEBHOOK_SECRET`. Without
  them, `cihub-server` stops at startup with a message that names them; `cihub-server migrate` does not need
  them. `CIHUB_PUBLIC_URL` disappears. Several SCMs will come later, each with its variables.
- **D93** Lot 5 creates the crate `cihub-mergequeue` (`crates/mergequeue/`). It depends on `cihub-domain`,
  `cihub-mq-core`, `cihub-config`, `cihub-platform`, `cihub-scm` and `cihub-store`; `cihub-contracts` (Lot 14)
  and `cihub-analytics` (Lot 11) arrive with their lots.
- **D94** Extends D35: the `Module` contract of `cihub-platform` is a struct, not a trait (D62): the name of the
  module, its executors for `Executors::register` and its loops per role. The routes arrive in Lot 9, the
  subscriptions in Lot 11. An `org_modules` table (`org_id`, `module`, `enabled_at`), with row level security,
  enables a module for an organization; without a row, the organization does not have the module.
- **D95** Migrations of Lot 5: `queues` gains the state, the mode and the settings (D56); the tables
  `queue_items`, `test_groups` and `test_runs` follow `docs/DATA_MODEL.md`, with the identifier lists in `text[]`
  (D37); a `queue_snapshots` table (`org_id`, `queue_id`, `seq`, `payload jsonb`, `created_at`) keeps a snapshot
  every 500 events.
- **D96** The `refresh_prs` job reads the state of at most 50 PRs of a queue through the grouped read of the
  `Scm` trait; its result comes back as `pr.observed` facts. Its key is `refresh_prs:<queue_id>-<YYYYMMDDTHHMM>:1`;
  its class is `interactive` after a command or a submission, `repair` for the 1 to 15 minute polling.
  _Extended by D101._
- **D97** Extends D26: `insta` arrives in Lot 5, as a development dependency only, for the snapshots of plans and
  projections.
- **D98** Lot 5 publishes neither the queue status nor the comments on the PR (Lot 9); it only excludes the queue
  status from the required checks read for mergeability.
- **D99** Extends D24: `cihub-mq-core` depends on `cihub-config` for `QueueSettings` (D25), not on
  `cihub-conditions`.
- **D100** A `queue_inbox` table receives the facts that the jobs, then the API, return to a queue: the identifier
  of the fact, its actor, its type and its payload. The depositor marks the queue; the scheduler applies these
  facts under the lock of the queue and marks them processed in the transaction of the reconciliation. The
  scheduler is the only one that writes the log.
- **D101** Extends D96: the key of `refresh_prs` names its cause. After a delivery, it carries its identifier
  (`refresh_prs:<queue_id>-<delivery_id>:1`, class `interactive`); the polling keeps the key of its window
  (`refresh_prs:<queue_id>-<YYYYMMDDTHHMM>:1`, class `repair`). _Extended by D170._
- **D102** The `submit_pr` job (class `interactive`, key `submit_pr:<delivery_id>:1`) submits a PR after a command
  or a label: it checks the permission of the author of a command (`write`, `admin` for `--direct`), reads the
  PR and deposits `item.submitted` and `pr.observed` in `queue_inbox`.
- **D103** A scheduler pass reads the deliveries of a connection without a queue (access lost or restored),
  deposits their fact in the `queue_inbox` of each queue of the connection, marks these queues and marks the
  delivery processed, in one transaction. _Extended by D170._
- **D104** Extends the interfaces of Lot 1: `Item` gains `submitted_at` and `Observed` gains `observed_at`, filled
  by `apply` with the time of the event; `plan` cancels a `not_ready` item after `not_ready_timeout_hours`
  (`not_ready_timeout_expired`, without a cause); `Desired` gains `observe`, the reads that are due (`Read`: the
  PR and its read point), and `Plan` gains `wake_at`, the next wake-up of the queue. The read points depend only
  on the age of the item: 1, 3, 7 and 15 minutes after the submission, then every 15 minutes, and every minute
  as long as the mergeability is `unknown`. A read that changes nothing is not in the log; the point gives the
  window of the polling key (D101).
- **D105** The queue status exception is in the jobs that read a PR: when the only unsatisfied required check is
  the `CIHub Merge Queue (<branch>)` status, the job reports the PR `mergeable`. `plan` does not change.
- **D106** Extends D57: the `maintenance` job deletes an `events` partition older than 90 days only when each
  queue that has events in it has a snapshot with a `seq` at least equal. A queue also takes a snapshot when its
  oldest event after the last snapshot is more than 30 days old; an hourly pass of the scheduler wakes the queues
  in this case. `seq` follows the last event or the last snapshot, and never restarts at 1. The full replay
  (`rebuild`) requires a whole log; after a deletion, a queue loads from its snapshot.
- **D107** Extends D75: `ScmEvent::PrChanged` carries `label_added`, the label that an action added, so that a
  submission by label does not require reading each PR that changes.
- **D108** Extends D102 and D103: a comment does not give the base branch of the PR; the command goes through the
  connection deliveries pass, and `submit_pr` finds the queue by the base branch of the PR it reads. A submission
  by label has the `system` actor, because the delivery does not say who added the label.
- **D109** Extends D101: beyond 50 PRs at the same read point, the polling key adds the rank of its part
  (`refresh_prs:<queue_id>-<YYYYMMDDTHHMM>-2:1`).
- **D110** Extends D95: no `queues.settings` column before Lot 8; in Lot 5, each queue has the default settings.
  _Extended by D134: no `queues.settings` column, the settings come from the file._
- **D111** Extends D94: a `Module` always has a scheduler loop.
- **D112** The `undetermined` reason of a blocked PR does not name the missing required check; Lot 9 refines it,
  with the PR surfaces.
- **D113** A submission stays possible when the queue is `paused` (`docs/RELIABILITY.md`): it waits for the
  resumption of the queue to create its item.
- **D114** The `inplace` mode updates the branch of the PR by a merge of the target (`update_method: merge`);
  `rebase` arrives with a later lot, and `UpdateMethod` has only `merge` until then. _Extended by D119._
- **D115** Extends D33 and D94: the format of `GET /api/v1/test-runs/{run_id}` is frozen in `docs/API.md` in
  Lot 6; the route is served in Lot 9, with the others.
- **D116** Extends D72: Lot 6 cancels no workflow of a superseded run; its reports are stored then ignored. The
  method of the `Scm` trait arrives with the optional `actions: write` permission.
- **D117** Two jobs of the test cycle: `assemble_test_branch:<run_id>:1` (class `speculate`), which the scheduler
  requests for each `assembling` run of the desired state, and `cleanup_test_branch:<run_id>:1` (class
  `repair`), which it requests when it writes the decision that ends a run, to be executed after
  `keep_test_branches_minutes`; this job reads `test_runs`, closes the draft PR and deletes the test branch and
  the temporary branch.
- **D118** A check report for the test SHA of a run that is no longer live is stored in `check_observations`
  without a fact in the log; the replay (`rebuild`) keeps these rows, and the table has no foreign key to
  `test_runs`.
- **D119** Extends D114: the `inplace` mode leaves Lot 7 and arrives with a later lot, with its own decision
  (exemption in `pr_changes` when the observed head is the test SHA of the live run, `sha` guard of the merge on
  this SHA, formation only without a live predecessor, `head_branch` in `PrSnapshot`, routing of the checks by
  SHA). Until then, `cihub-config` refuses `trigger: inplace`.
- **D120** (Q2) `merge_method` is `squash` by default for each strategy; `fast_forward` is an option.
  `cihub-config` accepts it only with the `sequential` strategy, as long as the assembly redoes the merges of the
  predecessors on `base_sha`: the test commit of a PR then does not descend from the test commit of its
  predecessor. `fast_forward` advances the target, without forcing, to the test SHA of the successful run, and
  not to the head of the PR; a refusal because the target moved is a merge failure. _Extended by D124._
- **D121** (Q4) A push on the head of a queued PR cancels its item (`pr_pushed_to`); the PR must be resubmitted.
  The `resubmit_on_push` setting, which resubmits the item at the end of the queue, arrives in Lot 8 with
  `queues.settings`. _Extended by D134: the setting comes from the file._
- **D122** (Q13) A direct merge failure gives the same reason as a merge failure of a run, from `pending`:
  `head_changed` gives `pr_updated_at_merge_time`, `rate_limit_exceeded`, `branch_protection_unmet`,
  `merge_already_in_progress` and `repeated_server_errors` give `merge_<error>`, and `merge_rejected` gives
  `direct_merge_rejected`. The transition table gains these reasons for `pending` → `failed`.
- **D123** The `merge_pr:<item_id>:1` job (class `merge`) merges each PR of `Desired.merges`. Before acting, it
  rereads `queue_items`: an item that is no longer `tests_passed` ends the job without a fact. It returns
  `merge.succeeded` or `merge.failed` in `queue_inbox`. `HeadChanged` gives `head_changed`, `Forbidden` gives
  `branch_protection_unmet`, `MergeRejected` and `Validation` give `merge_rejected`, immediately; `Unavailable`
  and `MergeInProgress` redo the job with the backoff, then give `repeated_server_errors` or
  `merge_already_in_progress` after 6 attempts; `RateLimited` and `Throttled` redo the job as long as the end of
  the pause is less than 15 minutes away, otherwise give `rate_limit_exceeded`.
- **D124** Extends D120: `Merge` (`Desired`) and `PrMerge` (`Scm` trait) carry `tested_sha`, the test SHA of the
  run that proved the item. `fast_forward` advances the target to this SHA. A refusal because it is not a
  fast-forward is a transient error, like `Unavailable` (D123): the fact of the external push restarts the group;
  after 6 attempts, it gives `merge_rejected`.
- **D125** The direct merge on main (`direct_merge_mode`) moves to Lot 8, with the required statuses in the
  settings and the injection of the protections.
- **D126** `PrSnapshot` gains `title` and `body`, read in the same read, for `merge_commit_title` and the
  `merge-commit-title:` line of the PR body. The recorded GitHub responses (D90) are recorded again.
- **D127** The repair loop: `Desired.observe` rereads each live PR every 15 minutes (not only the `not_ready`
  items); the `refresh_checks:<run_id>-<YYYYMMDDTHHMM>:1` job (class `repair`) rereads the checks of the test SHA
  of an `in_progress` run without a report for 10 minutes, then every 10 minutes, returns `check.observed`
  facts, and recreates at the test SHA a test branch that is absent; the
  `refresh_target:<queue_id>-<YYYYMMDDTHHMM>:1` job (class `repair`) rereads the head of the target every 5
  minutes and returns `target.pushed` without `before`.
- **D128** Lot 7 delivers the `resync` function of `cihub-mergequeue`: it requests, in class `interactive`, the
  reread of each live PR, of the checks of each live run and of the head of the target. The route
  `POST /api/v1/queues/{id}:resync` arrives in Lot 9.
- **D129** The `fault_injection` test (`crates/mergequeue/tests/postgres.rs`) plays one seed per organization and
  per queue, in a single database; `CIHUB_SIM_SEEDS` gives the number of seeds. The simulator gains random
  failures drawn from a seed (5xx, 429); the test stops and restarts the worker and the scheduler at random
  instants. `mise run sim` plays 1,000 seeds, `mise run test-integration` plays 10; the CI runs `sim` once per
  night, in a separate job.
- **D130** A 422 from the assembly because a head is already in the target fails the run immediately
  (`start_test_run_merge_conflict`). The `maintenance` job also deletes the unrouted deliveries older than 7
  days.

**October 2, 2026, validated by Nicolas Lecoy.**

- **D131** At startup, each scheduler replica marks the queues that have a live item, in the organizations that
  have the module: a plan written by an older version may have scheduled no wake-up.
- **D132** The `merge_pr` job reads the PR before merging it; an already merged PR gives `merge.failed` with the
  error `pr_already_merged`, without a merge call, and `plan` makes it `merged` with the reason `manual_merge`:
  the downstream restarts and tests itself on the real target. A merge by the queue followed by a stop before its
  fact also gives `manual_merge`, which costs one more test but stays safe.
- **D133** Only validation errors (422) count toward the 5 attempts of the assembly; another transient error
  (5xx, 429, stop) redoes the job until 15 minutes after the start of the run, then gives `retry_exhausted`.
  `MAX_ATTEMPTS` goes from 8 to 11, so that the backoff covers these 15 minutes.
- **D134** The `.cihub.yaml` file (or `.cihub.yml`) is the only source of the settings of a queue: the dashboard
  writes no setting, and there is neither a `ui` source in `cihub-config` nor a `ui_override` value of the
  required statuses. The dashboard helps write the file through a form that opens it as a PR on the repository,
  never as a direct commit on the target (Lot 10); a help prompt arrives with the installation README (Lot 9).
  The state of the queue (`pause`, `resume`, `drain`) is not a setting and stays in the dashboard, the API and
  the CLI. An invalid file fails the run that reads it at `base_sha` (`start_test_run_config_*`); for the other
  settings, the queue keeps the last valid file and shows the error.
- **D135** `queues` gains `name`, unique per repository (`repo_id`, `name`), instead of the uniqueness by
  (`repo_id`, `target_branch`): several queues can target the same branch. The routing of a delivery gives a
  queue only when the branch has only one (D85).
- **D136** The `load_config:<queue_id>-<sha>:1` job (class `repair`) reads the configuration file on the head of
  the target, which the scheduler requests when this head is not the one of the last read. It writes nothing if
  the target has moved since. `queues` gains `config_text` (the last valid file, `NULL` without a file),
  `config_sha` (the head read) and `config_error` (why the file read is not valid). Each reconciliation and each
  job resolves the settings of its queue from `config_text`; the assembly always reads the required statuses at
  `base_sha`.
- **D137** When the mode of the settings differs from the requested mode of the queue, the reconciliation writes
  the `queue.settings_changed` fact (actor `system`) before `plan`.
- **D138** Lot 8 creates no queue from the file: the admin command of Lot 9 creates them (Q14). An entry of the
  file is linked to its queue by its branch and its name; a queue without an entry has the default settings,
  without the `defaults` of the file. _Extended by D154: the API and the CLI create the queues._ _Superseded by D172._
- **D139** CIHub integrates with no other product: it studies the competition but takes none of its interfaces
  for compatibility. No command alias (`/trunk`), no names or format of another product in the outgoing
  webhooks, the API or the test branches, no translation of the reasons to another vocabulary. The specification
  documents describe CIHub alone; the comparisons are in `docs/COMPETITION.md`. Supersedes the last sentence of
  D23.
- **D140** Extends D139: CIHub provides no converter from the configuration of another product, and `GOAL.md`
  loses its objective "an easy migration". The attributes of the conditions have their own names, without a
  documented equivalence with another product. The help prompt (D134) helps write `.cihub.yaml`.
- **D141** The speculation window starts from the floor. It gains 1 when a group outside bisection passes its
  tests, and is divided by 2 (rounded down) when the test failure of a group is confirmed (`failed` after the
  failure budget, or bisection). A restart by the budget, a restart, a conflict, a configuration error or a merge
  failure do not change it. The `queue.window_changed` is written in the same step as the decision of the group,
  and keeps its `{before, after}` form. The window stays between the floor and the ceiling, and comes back within
  these bounds when the settings change; a reduced window interrupts no run in progress. The `throttled` mode
  does not change the core: the `speculate` jobs already wait for the budget. The exit criterion is tested in
  `cihub-mq-core`, on a simulated CI with a fixed seed.
- **D142** The jobs that read a PR (`submit_pr`, `refresh_prs`) evaluate the `queue_conditions` of its queue,
  read from `config_text`; the core stays without `cihub-conditions` (D99). `pr.observed` gains
  `unmet_conditions`, the text of the top-level conditions that are not satisfied, as the file writes it, and
  `BlockReason` gains `queue_conditions_unmet { conditions }`. The conditions count only before admission, for a
  `not_ready` item: a condition that becomes false does not remove an admitted item.
- **D143** `PrSnapshot` gains what the same read gives: `head_branch`, `from_fork`, `created_at`, the logins of
  the reviews that approve (`approved_by`) and that request changes (`changes_requested_by`), and the requested
  reviews (`review_requested`). A method of the `Scm` trait reads the changed files and the unresolved review
  threads of a PR, only when a condition of the queue uses them. The recorded GitHub responses (D90) are recorded
  again.
- **D144** `item.submitted` gains `sender_permission`, the permission of the author of the command read at
  submission (`NULL` for a label); a read of the PR evaluates `sender` and `sender-permission` with it.
- **D145** Neutral vocabulary of the conditions (D78, D139): `github-review-decision` becomes `review-decision`,
  and `sender-permission` has the levels `none` < `read` < `write` < `admin` of `RepoPermission`.
- **D146** In this lot, `merge_conditions` accepts only top-level `check-success = <name>` conditions; their names
  are added to the required statuses of the run. Another form is a configuration error, with its position.
- **D147** `branch_protection_injection`: `both` (default) adds the checks required by the rules of the branch to
  the `queue_conditions` (as `check-success`, except the queue status, D105) and to the required statuses of the
  test commit; `merge` adds them only to the test commit; `none` adds them nowhere.
- **D148** `submit_pr` selects the queue named by `/cihub merge --queue <name>`, otherwise the first queue of the
  branch, in the order of the file, whose `queue_conditions` pass, otherwise the first queue of the branch. An
  item never changes queue; a delivery of a PR goes to the queue of its live item. `submit_pr` applies
  `priority_rules`: the first true rule gives the priority, and `--priority` of the command wins.
- **D149** Extends D148: the option is written `/cihub merge --queue=<name>`, like `--priority=<value>`; an
  option with a value is always written `--option=<value>`.
- **D150** Extends D135: a push on a branch that has several queues is routed to no queue; the connection
  deliveries pass then asks each queue of the branch to reread its head (`refresh_target`, class
  `interactive`). As long as a `merge_pr` job of one of these queues waits or runs, the delivery waits for the
  next pass, like a delivery routed to a single queue (deviation 4 of Lot 7).
- **D151** A submission that arrives when the queue is `draining` or `switching_modes` creates an item directly
  `cancelled`, with the new reason `queue_draining` or `queue_switching_modes`; the transition table allows
  "nothing → `cancelled`" for these two reasons. This item never becomes the live item of the PR: a live item of
  the same PR stays so. In `paused`, the submission waits for the resumption (D113).
- **D152** New setting `resubmit_on_push` (disabled by default). When it is active and a job that reads a PR sees
  a new head on a live item, it deposits a new `item.submitted` with the same priority, the same source and the
  same options; `plan` cancels the old item (`pr_pushed_to`) and creates the new one, which starts again as
  `not_ready` at the end of the queue. A submission by label is redone only if the PR still has the label.
  Extends D121.
- **D153** The direct merge on the target (D125) arrives in Lot 8, disabled by default: `direct_merge_mode` is
  `off` by default and in the strategy table, and `always` is an explicit choice of the file. With `always`,
  `plan` proposes a direct merge only once per item, when the item is `pending`, at the head, in a `running`
  queue without a live group; it forms no group for it as long as the attempt has not answered. The `merge_pr`
  job checks before acting that the required statuses (the same as for a test commit, D146, D147) are green on
  the head of the PR, that the head of the target is an ancestor of the head of the PR (new `is_ancestor` method
  of the `Scm` trait) and that the PR is mergeable; otherwise it returns `merge.failed` with the new
  `JobError::DirectMergeUnavailable`, a fact that marks the item and lets it follow the normal path, without a
  decision. The other errors of the direct merge keep the reasons of D122. The recorded GitHub responses (D90)
  are recorded again. With `squash`, `merge_commit` and `rebase`, a push on the target between the check and the
  merge is not detected; `fast_forward` does not have this window.
- **D154** (Q14) `cihub-server admin bootstrap --installation <id>` bootstraps an organization without the
  dashboard. The command stays after Lot 10: it serves installations without a dashboard, the recovery of an
  organization without an admin or a token, and the end-to-end tests. It creates the organization, its SCM
  account and its connection from the installation, requests the synchronization of the repositories, and shows
  only once an API token of the organization. No admin command creates a queue: queues are created by
  `POST /api/v1/repos/{owner}/{name}/queues` or by `cihub queue create`, which is added to the CLI commands. No
  MCP server in Lot 9. Extends D138. _Extended by D168._ _Extended by D172._
- **D155** Extends D154 and D86. The `Scm` trait gains two methods: `installation`, at the instance level, which
  returns the account of an installation (identifier and login), and `list_repos`, which returns the repositories
  of a connection. `bootstrap` gives the organization the login of the account as slug, enables the `mergequeue`
  module (`org_modules`) and creates a token named `bootstrap` with the five scopes, without `created_by`; on an
  already bootstrapped installation, it creates nothing more and shows a new token. The `sync_repos` job (class
  `repair`) creates and updates `repos`; it deletes no repository. As for `refresh_prs` (D101), its key names its
  cause: an `installation_repositories` delivery requests `sync_repos:<connection_id>-<delivery_id>:1`, and
  `bootstrap` and the hourly request of each scheduler replica request
  `sync_repos:<connection_id>-<YYYYMMDDTHH>:1`. The recorded GitHub responses (D90) are recorded again.
- **D156** The PR surfaces (Lot 9). `Desired` gains `surfaces`: for each live item and each item that the
  reconciliation ends (`plan` keeps it before `apply` evicts it), its state, its reason, its position, the
  numbers of its predecessors, its test branch, its failing checks, its block reasons and the labels of the PR.
  `cihub-mergequeue` turns them into the `CIHub Merge Queue (<branch>)` status, the comment (in English) and the
  labels `cihub:queued`, `cihub:testing` and `cihub:failed`. A `pr_surfaces` table keeps, without a fact in the
  log (like D118), the desired rendering and its fingerprint, written by the reconciliation, and what is
  published on the SCM (published fingerprint; the comment is in `pr_comments`, D163). The job
  `publish_surfaces:<queue_id>-<pr_number>-<hash>:1` (class `interactive`) publishes the most recent desired
  rendering; an older job never replaces the result of a more recent one; each reconciliation re-requests the job
  of the rows of its queue whose published fingerprint differs from the desired fingerprint. A dead job keeps its
  key: the last state of an ended item may then never be published. A stop between the creation of the comment
  and the write of its identifier can give a second comment. `merge_pr` publishes the `success` status before the
  merge call. `comments_enabled` and `status_check_enabled` stop the comment and the status; without
  `state_labels_enabled`, the queue adds no label and only updates the state labels that the PR already carries.
  The status has no details URL before Lot 10 (no public address since D92); the comment has no restart
  checkbox: it says to write `/cihub restart` for an item under test, and names the test branch without a link;
  for an ended item, it shows the submission checkbox of D163. A single job at a time writes the comment of a PR
  (advisory lock). _Extended by D173._
- **D157** The `/cihub cancel` and `/cihub restart` commands require `write` and go through the connection
  deliveries pass, like `merge` (D108). The `pr_command:<delivery_id>:1` job (class `interactive`) checks the
  permission, finds the live item of the PR and deposits `item.cancel_requested` or `item.restart_requested` in
  `queue_inbox` (reasons `user_requested`, `tests_restarted_by_user`). `cihub-github` keeps the permission of a
  user on a repository 5 minutes in memory, per connection. A refused command receives a comment from the job
  that says why: permission too low, commands disabled, no queue for the branch, or no live item.
  _Extended by D166._ _Extended by D173._
- **D158** (D112) `pr.observed` gains `required_checks`, the checks that the rules of the branch require, without
  the queue status (D105). For a `blocked` PR, `plan` gives `failing_required_check { check }` for a failing
  required check, `awaiting_required_check { check, status }` for a required check that has not finished, and
  `undetermined` only when no required check explains it. No new reason.
- **D159** The API v1 layer (Lot 9). The `cihub-api-types` crate (`crates/api-types/`) carries the requests, the
  responses, the error codes and the `problem+json` body; its `openapi` feature, which only the server enables,
  derives the schemas with `utoipa` (new dependency, OpenAPI 3.1). The `Module` contract gains its routes (an
  `axum` `Router`) and their OpenAPI document (D94); `cihub-platform` carries what the routes of the modules
  share: the error that renders as `problem+json`, the organization and the scopes of the token of the request,
  and the check of a scope. `cihub-web` authenticates each request of `/api/v1` by `Authorization: Bearer`,
  limits each token to 600 requests per minute (one-minute window, in memory and per replica of the `web` role,
  without a dependency), caps the bodies at 25 MB, and serves `/api/v1/openapi.json`, gathered from the modules.
- **D160** The routes of Lot 9. Read (`queues:read`): `GET /repos`, `GET /repos/{owner}/{name}/queues`,
  `GET /queues/{id}`, `GET /queues/{id}/items`, `GET /queues/{id}/prs/{pr_number}` (`?include=timeline`),
  `POST /queues/{id}/prs:batchGet`, `GET /queues/{id}/graph`, `GET /test-runs/{run_id}` (format of D115). PR
  writes (`prs:write`): `POST /queues/{id}/items` (a submission, rendered by the `submit_pr` job with the `api`
  source: response `202`), `POST /items/{id}/cancel` (`admin_requested`) and `POST /items/{id}/restart`
  (`tests_restarted_by_api_request`). Queue writes (`queues:write`): `POST /repos/{owner}/{name}/queues`
  (`target_branch` and `name` only, D134), `PATCH /queues/{id}` (`state` only: `running`, `paused`,
  `draining`), `POST /queues/{id}:resync` (D128) and `GET /orgs/{org}/audit`. An action of a token has the
  `system` actor in the log; `audit_log` gains `actor_token_id`, the token that acted. Deferred: `DELETE
  /queues/{id}` (the log of a queue is append-only: its deletion requires a decision on its retention),
  `GET /queues/{id}/test-results` (Lot 11, with the fact tables), the `?branch=` search (the projection does not
  keep the head branch), `impacted-targets` (Lot 13), `/metrics` and `/orgs/{org}/webhooks` (Lot 11).
  _Extended by D172._
- **D161** The CLI (Lot 9). `cihub-client` (`crates/client/`) is the typed client of the API, on `reqwest` and
  `rustls`; `cihub-cli` (`crates/cli/`) gives the `cihub` binary, with `clap` (new dependency). `cihub login`
  keeps the address and the token in `$XDG_CONFIG_HOME/cihub/credentials.json` (or `~/.config/cihub/`), in mode
  0600; `CIHUB_URL` and `CIHUB_TOKEN` take precedence. The repository comes from the `origin` remote of git; the
  queue comes from `--queue <name>`, or from the only queue of the repository. Commands: `login`,
  `merge <pr> [-p <priority>] [--no-batch]`, `cancel <pr>`, `restart <pr>`, `status [<pr>]`,
  `queue create <branch> [--name <name>]`, `queue pause|resume|drain`, `queue graph`. `targets upload` arrives
  in Lot 13. _Extended by D168._ _Extended by D172._
- **D162** The configuration checker (D87). The `Scm` trait gains `check_setup(repo, branche)`, which returns
  neutral findings from a closed vocabulary: `update_restriction_missing` (no rule restricts the updates of the
  branch), `app_not_exempt` (the App is not exempt from this restriction), `test_branches_not_excluded` (a rule
  also covers `cihub-temp/**` or `cihub-merge/**`) and `up_to_date_required` ("up-to-date branches" required).
  The GitHub adapter reads the rulesets of the branch; the no-op ref update of `docs/GITHUB.md` awaits a
  decision. `GET /queues/{id}/config-check` (`queues:read`) returns the findings. The recorded GitHub responses
  (D90) are recorded again.
- **D163** The welcome comment and its submission checkbox. When a PR is opened, reopened or goes from draft to
  ready (`ScmEvent::PrChanged` gains `opened`) on a branch that has an active queue, with `comments_enabled` and
  `commands_enabled`, the `welcome_pr:<delivery_id>:1` job (class `interactive`) posts once per PR a comment
  that says the queue manages the branch, with a submission checkbox (marker `<!-- cihub:submit -->`) and the
  `/cihub merge` command. The comment belongs to the PR: a `pr_comments` table (`org_id`, `repo_id`, `pr_number`,
  `comment_external_id`) replaces `pr_surfaces.comment_external_id`, and this same comment becomes the status
  comment of the queue. The adapter translates an edit of a comment where the checkbox goes from empty to
  checked, made by an author other than the bot, into `ScmEvent::SubmitBoxChecked`; the connection deliveries
  pass checks that the comment is the one of `pr_comments`, then requests `submit_pr` as for `/cihub merge`
  (`write` permission of the author, source `comment`). The checkbox comes back for a `failed` or `cancelled`
  item, and does not appear as long as the PR is in the queue. No checkbox for `restart` or `cancel`. Takes no
  name or marker from another product (D139). _Extended by D173._

- **D164** `AGENTS.md` stays under 150 lines and contains only what applies to each session: the working
  rules, the ten principles, the rules that the tools do not enforce, the commands and the map of the documents.
  The decision log and the open questions are in `docs/DECISIONS.md`, append-only, with the same identifiers;
  the `AGENTS.md Dn` references in the code and the documents become `DECISIONS.md Dn`. What `GOAL.md`,
  `docs/ARCHITECTURE.md`, `Cargo.toml`, `clippy.toml` and `deny.toml` already say is not repeated. Everything in the
  repository is in English, the documents included; an agent replies to a person in the language they write in.
- **D165** Extends D3: `docs/LOTS.md` keeps the phases, the table, the current lot and the lots to do; the
  tracking of a finished lot moves to `docs/LOTS_DONE.md` when the next lot starts.

**October 3, 2026, validated by Nicolas Lecoy.**

- **D166** Extends D157: a `/cihub merge`, `/cihub cancel` or `/cihub restart` command that its job accepts
  receives an acknowledgement on its comment, after the job gives its facts to the queue. `ScmEvent::Commented`
  gains `comment_external_id`, the payloads of `submit_pr` and `pr_command` gain `comment_id` (`None` for a
  label, the checkbox or the API), and the `Scm` trait gains `acknowledge_comment(repo, comment_id)`, which the
  GitHub adapter makes a `+1` reaction (permission Issues: write, already granted). A failed acknowledgement is
  logged at `warn` and does not fail the job, so that a retry does not give the facts twice. No reaction at the
  reception of the command; a refused command keeps only its comment (D157). A separate evolution, outside the
  plan of a lot.

**October 4, 2026, validated by Nicolas Lecoy.**

- **D167** Each `Reason` has a sentence for a person, `Reason::describe()` in `cihub-domain`: in English, a free
  rendering that can change without a decision; the code (`as_str`) stays the stable contract. The status and
  the comment of the PR show the sentence instead of the code, with the detail that the surface already has
  (failing checks). The API `Item` gains `state_description`, computed from the same table, and the CLI shows
  it.

**October 3, 2026, validated by Nicolas Lecoy.**

- **D168** One binary, `cihub`. Supersedes D8; extends D14, D31, D154 and D161. `crates/server/` (`cihub-server`)
  gives the `cihub` binary, with `clap`; `crates/cli/` is removed, and the CLI carries the server code. The
  commands are in three groups: `cihub login`; `cihub queue merge|cancel|restart|status|graph|create|pause|
  resume|drain`, with the options `--repo <owner/name>` (the `origin` remote of git by default) and
  `--queue <name>` (the only queue of the repository by default; with two or more, the command fails and lists
  the names); `cihub server start [<role>...]`, `cihub server migrate` and
  `cihub server bootstrap --installation <id>`. The four roles (D91) are arguments of `cihub server start`, all
  four without an argument; `CIHUB_ROLES` is removed. `cihub server` reads a YAML file named by
  `--config <file>` (`serde-saphyr`, no default path), whose keys are `database.url`, `http.addr`,
  `internal.addr`, `log.filter`, `log.format`, `github.url`, `github.app_id`, `github.private_key`,
  `github.private_key_file` and `github.webhook_secret`. The variable of a key is `CIHUB_` followed by its path
  in capitals, with `_` as separator; the precedence is variable, then file, then default; an unknown key or an
  invalid value stops the process. `CIHUB_LOG` becomes `CIHUB_LOG_FILTER`. `cihub queue targets upload`
  replaces `cihub targets upload` (Lot 13). The PR commands `/cihub merge|cancel|restart` do not change. _Extended by D169._ _Extended by D170._ _Extended by D172._
- **D169** Extends D168. The `cihub` binary is given by `cihub-cli` (`crates/cli/`, the former `crates/server/`):
  it defines every command, one file per command, and starts the components. The HTTP endpoints stay in
  `cihub-web`, whose name follows the `web` role. No crate per role: `cihub-cli` has one file per role
  (`server/web.rs`, `server/scheduler.rs`, `server/worker.rs`, `server/notifier.rs`), which starts the loops whose
  code is in the library crates (`cihub-web`, `cihub-mergequeue`, `cihub-platform`); a role gets its own crate only
  by a decision, when its code grows.

**October 5, 2026, validated by Nicolas Lecoy.**

- **D170** Extends D168: `cihub server start` takes no argument and always runs the four roles in one process; the
  `ROLE` argument is removed. A deployment scales by identical replicas; the leases, the locks and the idempotency
  keys already share the work between them. A role stays an internal unit: one task and one `role` span in the
  logs. The probes give only the name and the version of the binary. A selection of the roles can come back as an
  addition when a measured need appears.
- **D171** The `merge_pr` job keeps `PUT /pulls/{n}/merge`, the only merge endpoint of GitHub Enterprise Server
  (3.22 has no asynchronous merge). When GitHub refuses a stacked PR there with a 403 that names its asynchronous
  endpoint, the GitHub adapter asks `PUT /pulls/{n}/merge-async` with `merge_action: direct_merge` and the same
  `sha` guard: `merged` gives the commit, `pending` or `409` give `MergeInProgress` (the job retries with its
  backoff), `failed` or `400` give `MergeRejected`. The `Scm` trait does not change, and the job does not keep the
  identifier of the request.

**October 4, 2026, validated by Nicolas Lecoy.**

- **D172** No queue is created by hand. Supersedes D138; extends D154, D160, D161 and D168. A command
  (`/cihub merge` or the box of D163) on a PR whose base branch has no queue that takes submissions creates the
  queues of that branch in the `submit_pr` job, then routes the submission (D148): the queues that the
  configuration file names on the branch, read on its head; without such an entry, on the default branch of
  the repository, a queue named after the branch; on another branch, none, and the command is refused with a
  comment. A created queue keeps the file that it was read from (`config_text`, `config_sha`, `config_error`),
  so that the routing of the first submission reads its conditions. Nothing is created when the organization
  does not have the module, nor by a label, which is a setting of a queue. An entry of the file is linked to
  its queue by its branch and its name; a queue without an entry has the `defaults` of the file. A queue takes
  new submissions while its file names it, or while its file names no queue of its branch and it is the queue
  of the default branch named after it; a queue that the file no longer names finishes its items and keeps its
  log. `POST /api/v1/repos/{owner}/{name}/queues`, `cihub queue create`, `CreateQueue` and the error code
  `queue_exists` are removed; the `write` permission of the author of the command is enough, the creation of a
  queue no longer needs an admin. The welcome comment (D163) does not change: it needs a queue.

**October 5, 2026, validated by Nicolas Lecoy.**

- **D170** Extends D101 and D103: a check run or a commit status that concludes (`CheckReport.conclusion` is
  set) on the head of a live `not_ready` item asks the queue of that item to read the PR at once. The connection
  deliveries pass requests `refresh_prs`, class `interactive`, keyed on the delivery
  (`refresh_prs:<queue_id>-<delivery_id>:1`), so that a replayed delivery requests nothing more. A check that has not
  concluded, or a check on another head, requests nothing. The read points of the plan do not change: they are
  the fallback when the delivery is lost (principle V). No filter on the names of the checks: a CI with N checks
  gives up to N reads per push of a waiting PR, to measure before a filter. A separate evolution, outside the plan
  of a lot.
- **D173** The layout of the comments of CIHub on the PR. Extends D156, D157 and D163. The comment has no title:
  it starts with a state line, an icon, the state in bold, then its reason or its position (`⏳ **Waiting**`,
  `🕓 **Queued**`, `🧪 **Testing**`, `⚠️ **Tests failed**`, `🚀 **Tests passed**`, `✅ **Merged**`, `❌ **Failed**`,
  `⚪ **Cancelled**`); then the block reasons in a list, the failing checks, the predecessors and the test branch;
  then the commands in small print (`<sub>`). The welcome comment says that CIHub manages merges into the branch,
  with the box "Add this PR to the merge queue"; the box of an ended item says "Submit again". The answer to a
  refused command quotes the command (`> /cihub restart`) and gives the cause after `Refused:`. The commit status
  does not change. Still no checkbox for `restart` or `cancel`: a click has no confirmation, both affect the PRs
  behind, and the comment of an item under test is often rewritten. A separate evolution, outside the plan of a
  lot. _Extended by D174._
- **D174** Extends D173: the comment of CIHub on the PR names no test branch and no run identifier. It says
  where the test runs, as a link: the draft PR of the run (`Tested on top of #5 in #42.`), or its test commit with
  `trigger: push`; nothing before the assembly. The dashboard (Lot 10) will replace it with a link to the page of
  the run, with the details URL of the status (D156).

## Open questions

Each question carries an identifier; `docs/LOTS.md` indicates the lot that needs the answer. A settled question
becomes a decision above and leaves this list.

- **Q1** The reason `tests_restarted_flake_suspected` (`docs/FLAKY_TESTS.md`) is not in the closed vocabulary.
  _(Lot 14)_
- **Q3** Optimistic merge after the eviction of a predecessor: strict rule (restart the downstream) by default,
  the lenient one remaining an option. _(Lot 12)_
- **Q7** OpenTelemetry semantic conventions for tests: follow the proposal of the CI/CD group, or freeze
  `cihub.*` attributes as long as it is not stable. _(Lot 14)_
- **Q8** Switch-over threshold to ClickHouse: volume from which the documentation recommends it. _(Lot 15)_
- **Q9** Dashboard stack: React with the generated TypeScript client is the hypothesis; to be confirmed.
  _(Lot 10)_
- **Q12** `docs/FLAKY_TESTS.md` subscribes the module to `mergequeue.run.completed`, which is not an event type
  of `docs/DOMAIN.md` (`run.transitioned` ?). _(Lot 14)_

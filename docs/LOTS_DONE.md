# Finished lots

The tracking of the finished lots, moved out of `docs/LOTS.md` (D165). The lots in progress and to do are in `docs/LOTS.md`; the decisions are in `docs/DECISIONS.md`.

### Lot 0: foundations of the repository

- **Goal**: a repository where each following lot adds only business code.
- **Content**: Cargo workspace (`Cargo.toml`, `[workspace.package]`, `[workspace.dependencies]`,
  `[workspace.lints]`); Rust and tools installed by `mise`, with `mise.lock` (`DECISIONS.md` D17); `clippy.toml`; `deny.toml` (licenses, security advisories,
  `bans` rules that keep the pure crates and the CLI free of server code); tasks of `mise.toml`;
  `compose.yaml` with Postgres (`DECISIONS.md` D12); GitHub Actions CI with one job per `mise` task and a
  `Build Success` job (`DECISIONS.md` D18); `LICENSE` (Apache-2.0) and `.gitignore`.
- **Frozen interfaces**: names of the crates (`cihub-*`, `publish = false`); names of the `mise` tasks; lints of the
  workspace.
- **Exit criterion**: `mise install --locked` installs the whole toolchain; `mise run check`, `mise run cover` and
  `mise run lint-workflows` pass locally and in CI; `mise run db` starts a healthy Postgres.
- **Tracking**:
  - Done: workspace; Rust 1.98.1 and tools by `mise`, with `mise.lock`; lints of the workspace and
    ban of the clock and of randomness (D16); `deny.toml`; tasks `fmt`, `lint`, `test`, `cover`, `check`,
    `lint-workflows` and `db`; `compose.yaml`; CI with the jobs `Rust Lint`, `Rust Test`, `Rust Coverage`,
    `Workflow Lint` and `Build Success`, actions pinned by SHA. Verified locally: `mise install --locked`,
    `mise run check`, `mise run cover` (91.4% of the lines of the pure crates), `mise run lint-workflows` and
    `mise run db`.
    `rust-toolchain.toml` removed: `mise.toml` pins Rust. release-please (`DECISIONS.md` D19). Renovate
    (`DECISIONS.md` D21), configuration validated by `renovate-config-validator` 44.118.1. CI, release-please and
    Renovate pass on GitHub.
  - Later: the repository moves from `nlecoy/cihub` to `getcihub/cihub`. `Cargo.toml` and the documents already use
    `getcihub`; the workflows read the repository name from `github.repository` and do not change. The secrets must
    be recreated and the bot GitHub App installed on the new repository.
  - Deviations: the skeleton of `cihub-server` moves to lot 2 (`DECISIONS.md` D9).

### Lot 1: pure core of the queue

- **Goal**: all the decision logic of the queue, without I/O, tested before a single GitHub call exists.
- **Content**: in `cihub-domain`, the entities `Item`, `TestGroup`, `TestRun`, `Event` and the whole closed
  vocabulary, with the tables of allowed transitions (`DECISIONS.md` D6); the crate `cihub-mq-core`, which implements
  `apply` (replay, `DECISIONS.md` D5); the order by `position` and the insertion by `priority_value`, including the
  `urgent` preemption; the merge graph with target sets (`ALL` in simple mode, overlap in parallel mode); the
  validity of a run (predecessors and `base_sha` unchanged); `plan` for the `single` groups; the consistency
  rules of `docs/RELIABILITY.md` written in `check`. The detailed target is below.
- **Frozen interfaces**: the entities and the `Event` envelope (with its payloads); the closed vocabulary; the
  signatures of `apply`, `plan` and `check`, and their types (queue snapshot, plan, violation).
- **Exit criterion**: table tests for each row of the transition tables; a test proves that replaying the produced
  events with `apply` gives back the same state; a property test (`proptest`) that generates random queues
  (submissions, successes, failures, cancellations, external pushes) and checks `check` after each step;
  coverage of the pure crates ≥ 90% (`mise run cover`); no forbidden dependency (`cargo deny`).
- **Tracking**:
  - Done (`cihub-domain`): typed ULID identifiers (`DECISIONS.md` D4); closed vocabulary, including a single
    `Reason` enumeration and the transition tables of the items, groups, runs and the queue (D6:
    `Lifecycle`, `check_transition`), `GroupState`, `Mergeability`, `JobError`, `RequiredStatusesSource`,
    `ActorKind` with `bot`, `QueueControl`, `PrState`, `ReviewDecision` (moved from `cihub-conditions`);
    `ConflictCulprit::StackedPr` removed (D7); `Event` envelope with `cause_id`, facts and decisions
    (`EventKind`, `EventType`); the identifier constructors are forbidden outside their exception (D20).
  - Done (`cihub-mq-core`): `Snapshot`; `apply` (replay, requests, removal of impossible requests, exit of the
    concluded entities); order by `position`, insertion by priority, `urgent` preemption, renumbering;
    merge graph by target sets; validity of the runs (predecessors and base); `plan` for the
    `single` groups: admission by mergeability, formation in a fixed window (`window.floor`), end of
    run (success, failure, timeout), pending failure at depth 0 with the failure budget, merge in the
    order of the graph, cancellations, PR changes, external push, manual merge, pause, draining,
    suspension, mode change; decision identifiers without randomness; `check` (rules of
    `docs/RELIABILITY.md`). Tests: transition tables, replayed scenarios, property test (20,000 cases
    verified locally). `mise run cover` also covers `cihub-mq-core`. Example `simulate`, which plays a
    scenario line by line and prints the decisions (`crates/mq-core/examples/`).
  - Validated deviations (Nicolas Lecoy, September 29, 2026): crate `cihub-mq-core` (`DECISIONS.md` D24); settings as a
    parameter of `plan` and `check` (D25); dependencies `thiserror` and `proptest`, no `insta` in this lot (D26);
    `item.submitted` carries `author_id`, `additions`, `deletions` and `changed_files` (D27, D42). Neutral vocabulary of the
    log (`DECISIONS.md` D23): `Mergeability` (`mergeable`, `behind`, `conflict`, `blocked`, `unknown`, which
    also covers the `DRAFT` value of GitHub, absent before), merge failure reasons `merge_*` instead of
    `github_api_*`, facts `access.lost` and `access.restored` instead of `installation.*`, `CheckKind`
    `check` instead of `check_run`.
  - Deviations in the reading of the tables, validated (Nicolas Lecoy, September 29, 2026) and written in the tables
    below: a `dissolved` group accepts the reasons of return to `pending` without `merge_item_now_ready`, the
    `cancelled` reasons without `not_ready_timeout_expired`, and the merge failure reasons without
    `direct_merge_rejected`. A direct merge failure always gives `direct_merge_rejected`, the only reason allowed
    from `pending` (`DECISIONS.md` Q13, settled by D122 in lot 7). A decision caused by the clock (timeout) or
    by a mode change has no `cause_id`. A PR seen merged while the queue requests its merge waits for
    the result of the merge job.
  - Remaining for the following lots: entry conditions and exception of the queue status (lot 5), adaptive
    window (lot 8), pending failure depth beyond 0 and optimistic merge (lot 12), batches and bisection
    (lot 12), direct merge (lot 7), `not_ready_timeout_hours` (lot 5), serialization of the `Snapshot` for
    the snapshot (lot 5).

#### Target of lot 1: log, facts and decisions

The log of a queue contains two kinds of events (`DECISIONS.md` D5; definitions in `docs/DOMAIN.md`). A
fact changes only the observed state; a decision changes only the state of the queue (states, positions, groups,
runs, window). `plan` reads the state and produces only decisions. The replay applies the events in
order with `apply` and never calls `plan`: a fix of `plan` does not change the past decisions. A
rule that can change with the code (the exception of the queue status, the translation of an error into a reason)
therefore produces a recorded decision, never a computation in `apply`. A fact that does not change the observed state
is not written.

Each decision carries the identifier of the fact that caused it (`cause_id`), except a decision caused by the clock
(timeout) or by a mode change, which has no source fact. A request (submission, cancellation,
restart, job result, request on the queue, suspension of the connection) waits for a decision. `apply` does not
write a request that cannot receive a decision (cancellation of a finished item, result of a replaced
run, submission when the queue is not `running`) or that repeats a pending request; the decision that
cites a request as its cause removes it, and a request that another decision makes impossible is removed too. The
creation of an item reads its submission in the request it cites. The clock is not an event: it is a
parameter of `plan`, and an exceeded delay becomes a decision with its reason. The raw webhook stays in the inbox;
the desired state (statuses, comments, refs) becomes jobs.

Facts:

| Type                                                 | Source                                                       | Payload                                                                                                                                              |
| ---------------------------------------------------- | ------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| `item.submitted`                                     | command, label, API, dashboard, CLI                          | PR number, head SHA, priority, `no_batch`, `direct`, `submitted_via`, attributes of D27                                                              |
| `item.cancel_requested`                              | `/cihub cancel`, API, CLI                                    | `by_admin`; gives `admin_requested` or `user_requested`                                                                                              |
| `item.restart_requested`                             | `/cihub restart`, API, CLI                                   | `by_api`; gives `tests_restarted_by_user` or `tests_restarted_by_api_request`                                                                        |
| `pr.observed`                                        | webhook `pull_request` or `pull_request_review`, tick, resync | full state read: open, closed or merged, head SHA, base branch, draft, labels, mergeability, review decision, checks of the PR                       |
| `target.pushed`                                      | webhook `push` on the target branch, tick                    | SHA `before` and `sender_id` (empty for a read by the tick), SHA `after`                                                                             |
| `check.observed`                                     | check or commit status, tick                                 | test SHA, name, kind, state, conclusion, URL; only for a SHA that a live run waits for                                                               |
| `targets.received`                                   | `POST …/impacted-targets`                                    | head SHA, targets or `ALL`                                                                                                                           |
| `test_branch.assembled`                              | assembly job                                                 | `run_id`, test SHA, draft PR number, required statuses and their source                                                                              |
| `test_branch.assembly_failed`                        | assembly job                                                 | `run_id`, job error, `conflict_culprit`                                                                                                              |
| `merge.succeeded`                                    | merge job                                                    | `item_id`, `merged_sha`                                                                                                                              |
| `merge.failed`                                       | merge job                                                    | `item_id`, direct merge or not, job error                                                                                                            |
| `queue.control_requested`                            | API, CLI                                                     | `pause`, `resume` or `drain`                                                                                                                         |
| `queue.settings_changed`                             | API, dashboard                                               | settings before and after                                                                                                                            |
| `access.lost`, `access.restored`                     | webhook `installation` (GitHub)                              | —                                                                                                                                                    |

A single `pr.observed` fact carries the full state of the PR, whatever the GitHub action: the webhook, the tick and
the resync produce the same event, a duplicate changes nothing, and `plan` is what finds the difference (push
on the head, closing, merge outside the queue, base change, conversion to draft, label removed).

Decisions:

| Type                     | Effect                                                                                                           | Payload                                                           |
| ------------------------ | ---------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| `item.transitioned`      | the item moves from one state to another, with a reason; the creation is a transition without a start state     | position at admission, `verified_by_run_id`, `merged_sha`         |
| `item.readiness_changed` | new mergeability and new block reasons                                                                           | the full list                                                     |
| `queue.renumbered`       | new positions (interval exhausted, `urgent` preemption)                                                          | `item_id` → position                                              |
| `group.formed`           | new group, in `testing`                                                                                          | kind, `item_ids`, predecessors, `base_sha`, parent                |
| `group.transitioned`     | the group moves from one state to another, with a reason                                                         | states before and after                                           |
| `run.transitioned`       | the run moves from one status to another, with a reason; the creation is a transition without a start status     | attempt, test branch; at the end, `failure_causing_statuses`      |
| `queue.transitioned`     | the queue moves from one state to another, with a reason                                                         | states before and after                                           |
| `queue.window_changed`   | new speculation window                                                                                           | values before and after                                           |

Actor of an event: `user` (with its identifier, `DECISIONS.md` D42), `bot` (the account of the App), `system` (CIHub: decisions, tick,
resync). Request of an operator on the queue: `pause`, `resume`, `drain`.

Choices of the core:

- The snapshot keeps, with each observation, the identifier of the fact that produced it, and the requests that
  wait for a decision; the concluded entities leave the snapshot as soon as no live group cites them.
- The decision identifiers are ULIDs computed without a random source: the 48 time bits come from the
  `now` parameter of `plan`, the other 80 bits from a hash of the queue, of the number of applied events and of an
  index. The same snapshot always gives the same identifiers.
- `plan` returns an error when one of its decisions is refused by the tables: it is a bug of the core.
- A push on the target that arrives late (its `after` or its `before` is a SHA older than the known head) changes
  nothing: the tick rereads the real head.
- An `urgent` item goes ahead of all the groups, including `pending_failure` and `tests_passed`, which return
  to `pending` with `skip_the_line_interrupted`.
- A PR seen merged while the queue has requested its merge is not a `manual_merge`: the queue waits for the result
  of the merge job.

#### Target of lot 1: transitions and reasons

A single `Reason` enumeration (`DECISIONS.md` D6). The values of the `failed` reasons carry the prefix of their
family: the recorded value is `start_test_run_merge_conflict`, not `merge_conflict`.

- Admission and test: `submitted`, `merge_item_now_ready`, `test_run_started`, `failure_budget_retry`.
- Return to `pending`: `merge_item_now_ready`, `manual_push_to_target_branch`, `prerequisite_merge_item_failed`,
  `prerequisite_merge_item_cancelled`, `prerequisite_merge_item_manually_merged`, `tests_restarted_by_user`,
  `tests_restarted_by_api_request`, `skip_the_line_interrupted`, `bisection_required`, `bisection_test_run_passed`.
- `tests_passed`: `test_run_passed`, `successor_test_run_passed`, `passed_by_optimization`.
- `merged`: `merge_graph_merge`, `merge_graph_direct_merge`, `manual_merge`.
- `cancelled`: `user_requested`, `admin_requested`, `pr_closed`, `pr_pushed_to`, `pr_base_branch_changed`,
  `pr_converted_to_draft`, `label_removed`, `not_ready_timeout_expired`.
- `start_test_run_*`: `start_test_run_invalid_event`, `start_test_run_merge_conflict`,
  `start_test_run_config_not_found`, `start_test_run_config_parsing_failure`, `start_test_run_config_bad_version`,
  `start_test_run_config_missing_required_statuses`, `start_test_run_config_bad_required_statuses`,
  `start_test_run_no_required_statuses`, `start_test_run_retry_exhausted`, `start_test_run_draft_prs_not_allowed`,
  `start_test_run_permissions_block_test_branch`.
- `test_run_*`: `test_run_required_status_failed`, `test_run_timeout`, `test_run_failed_by_optimization`.
- Merge failure: `merge_rejected`, `direct_merge_rejected`, `merge_rate_limit_exceeded`,
  `merge_branch_protection_unmet`, `merge_already_in_progress`, `merge_repeated_server_errors`,
  `pr_updated_at_merge_time`.
- Run: `test_branch_assembled`.
- Queue: `access_lost`, `access_restored`, `mode_changed`, `mode_switch_completed` (and
  `admin_requested`).

Transitions of an item:

| From                   | To                | Allowed reasons                                                                                                                                                                                                                                                                               |
| ---------------------- | ----------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| —                      | `not_ready`       | `submitted`                                                                                                                                                                                                                                                                                   |
| `not_ready`            | `pending`         | `merge_item_now_ready`                                                                                                                                                                                                                                                                        |
| `pending`              | `testing`         | `test_run_started`                                                                                                                                                                                                                                                                            |
| `pending`              | `merged`          | `merge_graph_direct_merge`                                                                                                                                                                                                                                                                    |
| `pending`              | `failed`          | `direct_merge_rejected`, `merge_rate_limit_exceeded`, `merge_branch_protection_unmet`, `merge_already_in_progress`, `merge_repeated_server_errors`, `pr_updated_at_merge_time` (`DECISIONS.md` D122)                                                                                       |
| `testing`              | `tests_passed`    | `test_run_passed`, `passed_by_optimization`                                                                                                                                                                                                                                                   |
| `testing`              | `pending_failure` | `test_run_required_status_failed`, `test_run_timeout`                                                                                                                                                                                                                                         |
| `testing`              | `failed`          | all the `start_test_run_*` reasons, `test_run_failed_by_optimization`                                                                                                                                                                                                                         |
| `testing`              | `pending`         | `manual_push_to_target_branch`, `prerequisite_merge_item_failed`, `prerequisite_merge_item_cancelled`, `prerequisite_merge_item_manually_merged`, `tests_restarted_by_user`, `tests_restarted_by_api_request`, `skip_the_line_interrupted`                                                    |
| `pending_failure`      | `pending`         | `skip_the_line_interrupted`, `manual_push_to_target_branch`, `prerequisite_merge_item_failed`, `prerequisite_merge_item_cancelled`, `prerequisite_merge_item_manually_merged`, `tests_restarted_by_user`, `tests_restarted_by_api_request`, `bisection_required`, `bisection_test_run_passed` |
| `pending_failure`      | `testing`         | `failure_budget_retry`                                                                                                                                                                                                                                                                        |
| `pending_failure`      | `tests_passed`    | `successor_test_run_passed`                                                                                                                                                                                                                                                                   |
| `pending_failure`      | `failed`          | `test_run_required_status_failed`, `test_run_timeout`, `test_run_failed_by_optimization`                                                                                                                                                                                                      |
| `tests_passed`         | `merged`          | `merge_graph_merge`                                                                                                                                                                                                                                                                           |
| `tests_passed`         | `pending`         | `skip_the_line_interrupted`, `manual_push_to_target_branch`, `prerequisite_merge_item_failed`, `prerequisite_merge_item_cancelled`, `prerequisite_merge_item_manually_merged`                                                                                                                 |
| `tests_passed`         | `failed`          | `merge_rejected`, `merge_rate_limit_exceeded`, `merge_branch_protection_unmet`, `merge_already_in_progress`, `merge_repeated_server_errors`, `pr_updated_at_merge_time`                                                                                 |
| `not_ready`            | `cancelled`       | `not_ready_timeout_expired`, and the reasons of the next row                                                                                                                                                                                                                                  |
| any non-terminal state | `cancelled`       | `user_requested`, `admin_requested`, `pr_closed`, `pr_pushed_to`, `pr_base_branch_changed`, `pr_converted_to_draft`, `label_removed`                                                                                                                                                          |
| any non-terminal state | `merged`          | `manual_merge`                                                                                                                                                                                                                                                                                |

States of a group (`GroupState`): `testing`, `pending_failure`, `tests_passed`, `merged`, `failed`, `bisected`,
`dissolved`; the last four are terminal. A batch never reaches `failed`: it becomes `bisected`. A
conflict at the assembly of a batch abandons the group (`dissolved`); the culprit PR goes to `failed` and the other
members return to `pending`. A group transition carries the reason that its items receive.

| From                   | To                | Allowed reasons                                                                                                                                                                                                                                |
| ---------------------- | ----------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| —                      | `testing`         | `test_run_started`                                                                                                                                                                                                                             |
| `testing`              | `tests_passed`    | `test_run_passed`, `passed_by_optimization`                                                                                                                                                                                                    |
| `testing`              | `pending_failure` | `test_run_required_status_failed`, `test_run_timeout`                                                                                                                                                                                          |
| `testing`              | `failed`          | all the `start_test_run_*` reasons, `test_run_failed_by_optimization`                                                                                                                                                                          |
| `pending_failure`      | `testing`         | `failure_budget_retry`                                                                                                                                                                                                                         |
| `pending_failure`      | `tests_passed`    | `successor_test_run_passed`                                                                                                                                                                                                                    |
| `pending_failure`      | `failed`          | `test_run_required_status_failed`, `test_run_timeout`, `test_run_failed_by_optimization`                                                                                                                                                       |
| `pending_failure`      | `bisected`        | `bisection_required`                                                                                                                                                                                                                           |
| `tests_passed`         | `merged`          | `merge_graph_merge`                                                                                                                                                                                                                            |
| `tests_passed`         | `failed`          | the reasons of `tests_passed` → `failed` of an item                                                                                                                                                                                            |
| any non-terminal state | `dissolved`       | the reason given to the item that causes the abandonment: reasons of return to `pending` except `merge_item_now_ready`, `cancelled` reasons except `not_ready_timeout_expired`, `manual_merge`, `start_test_run_merge_conflict` for a batch, merge failure reasons except `direct_merge_rejected` for a batch member |

Transitions of a run:

| From                        | To            | Allowed reasons                                                                                  |
| --------------------------- | ------------- | ------------------------------------------------------------------------------------------------ |
| —                           | `assembling`  | `test_run_started`, `failure_budget_retry`                                                       |
| `assembling`                | `in_progress` | `test_branch_assembled`                                                                          |
| `assembling`                | `failed`      | all the `start_test_run_*` reasons                                                               |
| `in_progress`               | `succeeded`   | `test_run_passed`                                                                                |
| `in_progress`               | `failed`      | `test_run_required_status_failed`                                                                |
| `in_progress`               | `timed_out`   | `test_run_timeout`                                                                               |
| `assembling`, `in_progress` | `interrupted` | `passed_by_optimization`, and the `dissolved` reasons of a group except the `cancelled` reasons  |
| `assembling`, `in_progress` | `cancelled`   | the `cancelled` reasons                                                                          |

Transitions of the queue:

| From                                     | To                | Allowed reasons                               |
| ---------------------------------------- | ----------------- | --------------------------------------------- |
| `running`, `draining`, `switching_modes` | `paused`          | `admin_requested`, `access_lost`   |
| `paused`                                 | `running`         | `admin_requested`, `access_restored` |
| `running`                                | `draining`        | `admin_requested`                             |
| `draining`                               | `running`         | `admin_requested`                             |
| `running`                                | `switching_modes` | `mode_changed`                                |
| `switching_modes`                        | `running`         | `mode_switch_completed`                       |

Other closed values:

- Mergeability of an item (`Mergeability`), translated by the SCM adapter (`DECISIONS.md` D23): `mergeable`,
  `behind`, `conflict`, `blocked`, `unknown`.
- Source of the required statuses (`RequiredStatusesSource`), in order of precedence: `repo_config`,
  `branch_protection` (`DECISIONS.md` D134).
- Job errors (`JobError`): a job reports an error, never a reason; `plan` is what chooses the reason.
  Assembly errors: `invalid_event`, `merge_conflict`, `config_not_found`, `config_parsing_failure`,
  `config_bad_version`, `config_missing_required_statuses`, `config_bad_required_statuses`,
  `no_required_statuses`, `retry_exhausted`, `draft_prs_not_allowed`, `permissions_block_test_branch`; each
  gives `start_test_run_<error>`. Merge errors: `merge_rejected`, `rate_limit_exceeded`,
  `branch_protection_unmet`, `merge_already_in_progress`, `repeated_server_errors`, `head_changed`;
  `head_changed` (409 on the `sha` guard) gives `pr_updated_at_merge_time`, `merge_rejected` gives
  `direct_merge_rejected` for a direct merge (`DECISIONS.md` D122), `pr_already_merged` gives `merged` with
  `manual_merge` (D132), and the others give `merge_rejected` or `merge_<error>`.

### Lot 2: data foundation: server, tenancy, log

- **Goal**: the tables shared by all the modules, with the isolation by organization from the start, and
  the binary that serves them.
- **Content**:
  - `cihub-server` (`crates/server/`, `DECISIONS.md` D8): `CIHUB_*` configuration, including `CIHUB_ROLES` and the
    connection string; logs (`DECISIONS.md` D13); one loop per role under a `CancellationToken`, which waits for
    the cancellation as long as its lot is not done; clean shutdown on `SIGTERM`; `/healthz` and `/readyz` on a
    separate internal address; `migrate` subcommand and schema check at startup (`DECISIONS.md` D14).
  - `cihub-store`: `sqlx` pool, transactions, setting of `cihub.org_id` for the RLS, embedded migrations
    (`migrations/`), queries checked at compile time (`.sqlx/`, `cargo sqlx prepare --check`), test that compares
    the Postgres enums to the enumerations of `cihub-domain`.
  - Tables `scm_instances`, `orgs`, `scm_accounts`, `scm_connections`, `repos`, `users`, `memberships`,
    `api_tokens`, `audit_log`; RLS with
    `cihub.org_id`; `events` table partitioned by month and serialization of `Event` (lot 1); tokens hashed with
    SHA-256 and scopes. The event schema keeps the attributes of D27. The outbox moves to lot 3 (`DECISIONS.md`
    D44).
  - Tasks `migrate`, `test-integration` and `dev`; Postgres service in the CI.
- **Frozen interfaces**: schema of the `events` table; RLS policy; format of the tokens; `CIHUB_*` variables of the
  server.
- **Exit criterion**: an integration test against a real Postgres proves that a connection set on
  organization A neither reads nor writes any row of B, for each table; the partitions of the next month are
  created by a job;
  `cihub-server migrate` run twice changes nothing the second time; `cihub-server` refuses to start when
  a migration is missing; `cihub-server` answers `200` on `/healthz` and `/readyz`, all-in-one and with one role
  per process.
- **Tracking**:
  - Done (step 1, skeleton of `cihub-server`): configuration `CIHUB_ROLES`, `CIHUB_INTERNAL_ADDR`, `CIHUB_LOG`
    and `CIHUB_LOG_FORMAT`, an invalid value stops the startup; logs in the JSON format of
    `tracing-subscriber` (spans in the `spans` list), or `pretty`; a `role` span and one task per role, which
    waits for the cancellation; shutdown on
    `SIGTERM` and `SIGINT`, and shutdown of all the tasks when one task stops on its own; `/healthz` and
    `/readyz` on the internal address, which give the name, the version and the roles. Verified by tests that
    launch the binary all-in-one and with a single role.
  - Done (step 2, `cihub-store` and migrations): `sqlx` pool (TLS by `rustls` and the system certificates);
    embedded migrations from `migrations/`, still empty; `cihub-server migrate`; at startup, stop if an
    embedded migration is missing in the database; `CIHUB_DATABASE_URL` as a `SecretString`; `/readyz` answers `503` when
    Postgres does not answer within 2 s; tasks `migrate`, `test-integration` and `dev`, and job `Rust Integration Test`
    in the CI. The tests against Postgres are in the `tests/postgres.rs` files; `mise run test` excludes them.
    Verified by tests on test migrations (`crates/store/tests/migrations/`) and on the binary.
  - Done (step 3, tenancy and RLS): migration `20260929160000_tenancy.sql` with `scm_instances`, `orgs`,
    `scm_accounts`, `scm_connections`, `repos`, `users`, `memberships`, `api_tokens` and `audit_log`, the enums
    `membership_role`, `scm_kind`, `connection_kind` and `api_scope`, and the `tenant` policy forced on each
    table that carries `org_id` (`DECISIONS.md` D37 to D41, D46 to D51);
    `begin_org` and `begin_system` in `cihub-store`; refusal to start when the Postgres role bypasses the RLS.
    Tests against Postgres: an organization does not read, update, delete or write any row of another,
    for each table; without a context, a transaction sees nothing; a context does not stay on the connection
    after the transaction; each table that carries `org_id` has its policy; the Postgres enums are those of
    `cihub-domain`; an `external_id` is unique in its SCM instance only; the binary refuses to
    start when a migration is missing.
  - Done (step 4, log): migration `20260929170000_events.sql` (`events` table, enums `event_type` and
    `actor_kind`, function `cihub_create_events_partition`); `append_events`, `load_events` and
    `create_event_partitions` in `cihub-store`; `cihub-server migrate` creates the partitions of the month and of the next
    month, and the `scheduler` role checks them at startup then every day (`DECISIONS.md` D36, D43). The logins
    leave the log: `Actor::User { user_id }`, `author_id`, `sender_id` (D42). Tests against Postgres: the
    log reads back identically and in `seq` order; another organization does not see it; a
    partition isolates the organizations; the partitions cover the month and the next one, and an event without a
    partition is refused.
  - Done (step 5, API tokens): `cht_…` form in `cihub-domain` (`format_api_token`, `is_api_token`);
    `new_api_token_secret`, `create_api_token` and `authenticate` in `cihub-store` (`DECISIONS.md` D45). Tests:
    only the fingerprint is stored; a token gives its organization and its scopes, then nothing after its
    expiration; an unknown or malformed token gives nothing; `last_used_at` advances at most once per minute.
  - Exit criteria: isolation of each table, partitions, `migrate` twice, refusal to start without a
    migration, `/healthz` and `/readyz` all-in-one and with one role, verified by `mise run test-integration`.
    The partitions are created by a loop of the scheduler, not by a job (D36). The outbox criterion moves to
    lot 3 (D44). The `Rust Integration Test` job passes on GitHub since September 30, 2026.
  - Remaining for the following lots: `sqlx-cli` and `.sqlx/` arrive with the first query checked at
    compile time. The partitions older than 90 days are deleted in lot 3: each queue must keep a
    more recent snapshot (lot 5). The refusal of a superuser has no automatic test
    (it needs the credentials of the superuser); it is verified by hand.
  - Validated deviations (Nicolas Lecoy, September 29, 2026): the crate `cihub-telemetry` arrives in this lot and carries
    the logs (`DECISIONS.md` D34). The default internal address is `127.0.0.1:8081`. The `Module` contract moves to
    lot 5 (D35). The partitions are created by `migrate` and by a loop of the scheduler (D36). `deny.toml` allows
    `axum` in `cihub-server` (for the internal address), `sqlx` in `cihub-server` (tests only), `tokio`
    in the external crates that depend on it, and the `Zlib` license (`foldhash`, by `sqlx`). The CI starts
    Postgres by `mise run db`, not by a GitHub Actions service. The development `compose.yaml` creates the
    `cihub` role without superuser rights (D38).
  - Validated deviations (Nicolas Lecoy, September 30, 2026): the tenancy model no longer depends on GitHub (D46 to
    D51). The migration `20260929160000_tenancy.sql` is rewritten, because no version is published: a
    development database that has already applied it must be recreated (`mise run db-reset`, D52).

### Lot 3: internal transport: inbox, jobs, leases

- **Goal**: the "internal API" between the roles, resistant to crashes and duplicates.
- **Content**: in `cihub-platform`, `webhook_inbox` with deduplication by delivery; jobs (`idempotency_key`,
  backoff, dead letter queue, `locked_until`, quotas per connection, priority class); worker loop and registry
  of the executors (`DECISIONS.md` D66); `queue_leases` and advisory lock per queue (D54); `queues.dirty`,
  `NOTIFY cihub_queue` and `cihub_jobs`; 30 s tick; daily `maintenance` job: `events` partitions and
  purges (D67). The scheduler loop moves to lot 5 (D64) and the outbox to lot 11 (D65).
- **Frozen interfaces**: signatures of the functions of the inbox, the jobs, the leases and the wakeup (D62); registry of the
  executors (D66); format of `idempotency_key` (D68); priority classes.
- **Exit criterion**: integration tests: a worker killed during a job sees the job taken over and executed only
  once; a duplicate delivery has no effect; two schedulers on the same queue never hold the same queue at the
  same time; a `NOTIFY` wakes up a scheduler in less than 1 s, and the tick wakes it up when `NOTIFY` is
  lost.
- **Tracking**:
  - Done (step 1, leases): crate `cihub-platform` (`DECISIONS.md` D53); migration `20260930090000_queues.sql` with
    a minimal `queues`, `queue_leases` and the foreign key of `events.queue_id` (D54, D56); `acquire_leases`,
    `release_leases` and `lock_queue`. Tests against Postgres: a lease goes to a single owner until its
    expiration; two owners at the same time never share a queue; a released lease is free right
    away; a single transaction at a time holds the lock of a queue.
  - Done (step 2, inbox): migration `20260930100000_webhook_inbox.sql`; `store_delivery`,
    `pending_deliveries` and `mark_processed`. A delivery has the key (`scm_instance_id`, `delivery_id`), because its
    identifier comes from the SCM (D46); the organization, the connection and the queue stay empty as long as the ingestion
    cannot route it, and only the system context sees it. Tests against Postgres: a copy of a delivery
    changes nothing; the same identifier on another instance is another delivery; the deliveries of a queue
    arrive in reception order until their processing; an unrouted delivery is visible only to the
    system, and a routed delivery requires its organization.
  - Done (step 3, jobs): migration `20260930110000_jobs.sql`; closed values `JobClass` and `JobState`;
    `enqueue_job`, `claim_jobs`, `complete_job` and `fail_job`. A job has its `idempotency_key` as key (D60); the
    worker keeps it for `JOB_LOCK` (60 s); a failure puts it back in waiting after 2, 4, 8… seconds (5 minutes at most),
    then `dead` after 8 attempts, or `failed` when a new attempt would fix nothing; a connection has at
    most 100 jobs in progress. Tests against Postgres: a second request of a job changes nothing; the due jobs
    arrive by class then by time; the job of a stopped worker is taken over after `JOB_LOCK`, the late answer
    of the first worker changes nothing, and the job is completed only once; two workers at the same time never
    share a job; a failing job waits for its delay then dies after its last attempt; a
    connection receives no more jobs beyond its quota.
  - Done (step 4, wakeup): `mark_dirty` marks a queue and sends `NOTIFY cihub_queue` in the transaction of
    the caller; `store_delivery` calls it for a new delivery of a queue; `begin_reconciliation` opens
    the transaction of a queue, with its lock, and resets `dirty` to false from the start, so that a write during
    the reconciliation marks the queue again after the commit; `schedule_tick` sets the next pass;
    `QueueWakeup` waits for a notification, the tick (`TICK`, 30 s) or the shutdown. Tests against Postgres: a new
    delivery wakes up a scheduler in less than 1 s, and a copy does not wake it up; the tick finds a queue whose
    notification is lost; a write during a reconciliation waits for the commit then marks the queue
    again.
  - Done (step 5, worker and maintenance): `Executors` registry and `run_worker` loop: it takes the due jobs,
    executes them together (16 at most), records their result (done, retry, definitive failure) and writes one
    line per job; it wakes up on `NOTIFY cihub_jobs`, sent by `enqueue_job`, or every 5 s. Job
    `maintenance`: `events` partitions of the month and the next one, deletion of the deliveries processed more than
    7 days ago and of the jobs finished more than 3 days ago (`finished_at` column). In `cihub-server`, the
    `scheduler` role requests the maintenance of the day at startup then every hour, and the `worker` role starts the
    loop; the daily partition loop disappears. Tests against Postgres: a worker executes each job
    with its executor, and a type without an executor fails; a new job wakes up a worker in less than 1 s; the
    maintenance has only one job per day and deletes only the rows that are too old; the all-in-one binary
    executes the maintenance of the day.
  - Done (example): `cargo run -p cihub-platform --example transport` plays the lot on a temporary database
    (`cihub_transport_demo`, deleted at the end) and prints the leases, the lock, the deliveries and the wakeup, a
    write during a reconciliation, a stopped worker, the recoveries, the quota and the maintenance.
  - Done (review of the frozen interfaces): the worker stops an executor after `JOB_TIMEOUT` (50 s), shorter than
    `JOB_LOCK`, and puts its job back in waiting (D69). Tests: an executor that is too slow is put back in waiting, a
    fast executor keeps its answer. `Outcome` and `Executors::register` do not change (D70, D71).
  - Remaining, moved to lot 4: the reading of the unrouted deliveries arrives with the ingestion; the pause of the jobs of a
    connection on a 429 (`scm_connections.paused_until`, respected by `claim_jobs`) arrives with the budget (D70).
  - Validated deviations (Nicolas Lecoy, September 30, 2026): the deletion of the partitions moves to lot 5 (D57). No
    trait for the inbox, the jobs and the outbox (D62), and no NATS in v1 (D63). The scheduler loop moves to
    lot 5 (D64) and the outbox to lot 11 (D65); a daily `maintenance` job replaces the partition loop
    (D67). A timeout per executor replaces the renewal of the lock (D69).

### Lot 4: GitHub adapter, simulator and installation of the App

- **Goal**: talk to GitHub through a single interface, and be able to test the whole queue without GitHub.
- **Content**: `Scm` trait (`cihub-scm`); `cihub-github` (App authentication, cached installation tokens,
  REST client and grouped GraphQL for the mergeability of up to 50 PRs, reading of the protections and rulesets, tracking of the
  budget by the headers, degraded modes `normal`, `throttled`, `scm_unavailable`); ingestion in the
  `web` role (`cihub-web`: HMAC, storage, acknowledgment in less than 2 s); translation of the webhooks into facts of the
  domain; in `cihub-scm`, behind the `sim` feature (`DECISIONS.md` D73), a GitHub simulator (refs, merges,
  PRs, checks, protections) that can inject 5xx, 429, lost, duplicated and out-of-order webhooks; creation
  of the App by hand according to `docs/GITHUB.md` (`DECISIONS.md` D92); configuration checker as an API endpoint (the interface arrives in lot 10);
  sweep of `GET /app/hook/deliveries`.
- **Frozen interfaces**: `Scm` trait; domain facts derived from GitHub; permissions of the App (`docs/GITHUB.md`).
- **Exit criterion**: a single contract suite passes against the simulator and against recorded
  GitHub responses; a 429 pauses the jobs of the connection until the reset; an App created by hand
  according to `docs/GITHUB.md` on a test organization, the server started with its variables receives a signed
  webhook (documented manual test, the only non-automated test of the lot).
- **Tracking**:
  - Done: the decisions of the lot (`DECISIONS.md` D72 to D81); crate `cihub-scm` with the `Scm` trait, its
    neutral types, `ScmError` and `ScmEvent` (D73, D75, D76). Simulator (`cihub_scm::sim`, `sim` feature): repositories
    with commits, branches, three-way merges and conflicts, PRs, checks, comments, labels, rules and
    permissions; log of the calls with their class; call faults (`Fault`); deliveries lost, duplicated
    or out of order by a seed (`WebhookFaults`). Contract suite (`cihub_scm::contract`): reads,
    branches, merges into a branch, statuses, comments, labels, draft PRs and merge guarded by the head.
    Tests: the simulator passes the suite; a fault fails its calls then stops; a suspended connection loses
    its access; each change sends its delivery; a PR that is behind waits when the rules require it; the
    delivery faults are reproducible.
  - Done (budget and pause): closed value `ConnectionMode`; migration `20260930120000_connection_budget.sql`
    (columns of D77 on `scm_connections`); `pause_connection`, and `claim_jobs` gives no job of a
    paused connection (D70). Test against Postgres: a paused connection receives no job until the end of its
    pause, and a shorter pause does not shorten it. `ScmError::Throttled`: the budget keeps its calls for
    the classes before `speculate`.
  - Done (`cihub-github`): App key (PKCS#1 or PKCS#8) and RS256 JWT by `ring`; HMAC-SHA256 signature of the
    webhooks; installation tokens in memory, renewed 5 minutes before their end; budget per connection in
    memory (`throttled` under 20%, back to `normal` after 5 minutes above 40%, `scm_unavailable` after 3
    failures with a probe every 30 s, pause until the reset on a primary or secondary limit); `Scm`
    trait in GraphQL for the reads of PRs and checks (50 PRs per request) and in REST for the rest. Unit
    tests: budget, classification of the responses, JWT, signature, mergeability and checks of GitHub.
  - Done (example): `cargo run -p cihub-github --example github` calls a real GitHub with the App of
    the environment (D79) and prints each response and the budget of the connection: installations of the App, reads
    of the `Scm` trait, and a few limited writes (`cihub-temp/example`, a status, a comment, a label).
  - Done (budgets in the database): `record_budgets` writes the budget of a connection on `scm_connections` (mode, reason,
    the limit that has the smallest remaining share, pause, failures), without shortening a longer pause; it replaces
    `pause_connection`. The `run_budget_writer` loop writes the changed budgets every 5 s, and right away
    when `GithubApp` signals a pause or a mode change; a failed write is redone at the next one.
    `cihub-server` reads the variables of the App (D79), stops without them (D92), and starts the loop in the
    `worker` role. Tests against Postgres: a budget is written on its connection; an urgent budget is written in less than
    1 s, before the next period. Unit tests: a pause or a mode change is urgent.
  - Done (recorded responses, part 1): format of the recordings (`tests/support/cassette.rs`) and fake GitHub
    that replays them in order (`tests/support/replay.rs`, `axum` as a development dependency, D74); disposable RSA
    key `tests/test-only-key.pem`, which opens nothing. The `github` example gains `prepare-base`, `prepare-pr` and
    `record`: it prepares the test repository, then plays the contract suite through a proxy that records each
    exchange without the token (D90). Test: a 429 pauses the connection until the reset, signals it right away,
    and no call leaves before the reset.
  - Done (recorded responses, part 2): first recording `tests/recorded/contract.json` (57 exchanges,
    September 30, 2026) on the public repository `getsyncr/contracts`: the rulesets do not apply to a private repository
    of a GitHub Free organization. The contract suite passes on the simulator and on this recording.
  - Done (ingestion): crate `cihub-web` (D73, D91) and public address `CIHUB_HTTP_ADDR` of the `web` role (D82).
    `POST /webhooks/github` (D83): `cihub_github::receive` verifies the signature and reads the headers; the delivery
    is stored without routing, visible only to the system context, then the route answers `202` (a copy too), `401`
    on a wrong signature, `400` on a missing header, `503` when the storage fails or exceeds 2 s (D84).
    At startup, `cihub-server` creates the `scm_instances` row of `CIHUB_GITHUB_URL` if it is missing
    (`ensure_scm_instance`, D79); `CIHUB_GITHUB_WEBHOOK_SECRET` is required with the App. Tests against Postgres: a
    signed delivery is stored once, unrouted; a wrong signature or a missing header is refused
    without storing anything; an instance is created once per address; the binary stores a signed webhook.
  - Done (translation and routing): `cihub_github::translate` translates a delivery into `ScmDelivery` (PR and review,
    comment created on a PR, push on a branch, check run, status, protection rule, repository, installation,
    repositories of the installation; the rest is ignored). Migration `20260930130000_inbox_routing.sql`:
    `routed_at` column. An unrouted delivery sends `NOTIFY cihub_inbox`; the `run_router` loop of the `web` role
    translates it, finds the connection, and the queue when a single one is certain (PR by its base branch, push by its
    branch), writes the routed row and marks the queue (D84, D85). An unknown connection leaves the delivery without
    an organization (D86); an ignored or unreadable delivery is routed as processed. Tests: translation of each
    event; against Postgres, each routing case in less than 1 s and the queues marked; the binary routes a
    received webhook.
  - Done (sweep): `RecentDelivery`, and the `recent_deliveries` and `redeliver` methods of the `Scm` trait, at the
    level of the instance (D88); the simulator keeps each delivery attempt; `cihub-github` calls
    `GET /app/hook/deliveries` page by page with the JWT of the App, down to an attempt older than 10
    minutes. Job `sweep_deliveries` (class `repair`, key of the 5-minute window), requested by the scheduler
    every 5 minutes and executed by the worker. Tests: the simulator lists a
    lost delivery and resends it with its identifier; the GitHub pages stop at the first attempt that is
    too old; against Postgres, the sweep requests again only the deliveries absent from the inbox.
  - Done (configuration of the App, D92): no assistant; `docs/GITHUB.md` describes the creation of the App by hand.
    `cihub-server` stops at startup without the variables of the App, with a message that names them;
    `cihub-server migrate` does not need them. The tests of the binary use the test key and a GitHub
    address where nothing listens. Test against Postgres: without an App, the server stops with this message and `migrate`
    succeeds.
  - Done (manual test of the exit criterion, October 1, 2026): an App created by hand according to `docs/GITHUB.md`,
    the server started with its variables behind a public URL. A `pull_request` webhook signed by GitHub is
    stored then routed without an organization (D86); a `workflow_run` is routed as processed. With an organization, its
    connection, the repository and the queue created by hand, a new delivery of the PR is routed to its connection and
    its queue, and the queue is marked.
  - Remaining, moved to lot 5: an end-to-end test of the chain "429 from GitHub, pause in the database, no job", with
    the first executor that calls the SCM through a connection; each of its links has a test.
  - Validated deviations (Nicolas Lecoy, September 30, 2026): the lot writes no fact in the log; it delivers the
    routed delivery and its translation into `ScmEvent`, and lot 5 writes the facts (D75). The secrets of the App
    come from the environment (D79). The simulator is a module of `cihub-scm` (D73). The configuration
    checker moves to lot 9 (D87). No organization is created from a webhook (D86). Validated deviation
    (Nicolas Lecoy, October 1, 2026): no assistant nor manifest flow, the App is created by hand (D92).

### Lot 5: reconciler and admission

- **Goal**: connect the pure core to the log and to GitHub: a submitted PR becomes `pending` or stays
  `not_ready` with a reason.
- **Content**: `Module` contract and activation of a module per organization (`DECISIONS.md` D35); in
  `cihub-mergequeue`, projection `queue_items`, `test_groups`, `test_runs` from the log,
  snapshot every 500 events, replay; reading of the inbox of a queue, translation of the `ScmEvent` into facts and
  delivery marked processed in the same transaction (`DECISIONS.md` D75); scheduler loop, with the leases, the wakeup and the transaction
  of a queue from lot 3 (`DECISIONS.md` D64); convergence of the plan against the observed state and emission of the jobs; submission by command and by label;
  mergeability, `block_reasons[]`, exception of the status of the queue itself; rechecks driven by the
  events and progressive polling from 1 to 15 minutes; `not_ready_timeout_hours`; deletion of the
  `events` partitions older than 90 days, once the snapshots are in place (`DECISIONS.md` D57); end-to-end
  test "429 from the SCM, pause in the database, no job", postponed from lot 4.
- **Frozen interfaces**: `Module` contract; tables `queue_items`, `test_groups`, `test_runs` (`docs/DATA_MODEL.md`,
  `DECISIONS.md` D28); job types of the queue.
- **Exit criterion**: on the simulator, 20 PRs submitted in various states reach `pending` or
  `not_ready` with the right reason; replaying the log rebuilds exactly the same projections; a scheduler
  killed in the middle of a reconciliation leaves no partial write.
- **Tracking**:
  - Done: the decisions of the lot (`DECISIONS.md` D93 to D99). Migration `20261001090000_merge_queue.sql`: closed
    value `ModuleName` and table `org_modules`; `queues.state` and `queues.mode`; tables `queue_items`,
    `test_groups`, `test_runs` and `queue_snapshots`, with row level security; Postgres enums of the states,
    the reasons and the other values of the log, compared to `cihub-domain`. The `Snapshot` of `cihub-mq-core`
    serializes to JSON (remainder of lot 1). `append_events` returns the `seq` of each event and `load_events` reads
    after a `seq`. Crate `cihub-mergequeue`: `QueueLog` reads the last snapshot of a queue then the
    events that follow, writes the events that `apply` accepts with their projections, and keeps a
    snapshot every 500 events; `rebuild` replays the whole log. Each projected transition is guarded
    by the start state, with a check of the number of rows. Tests: the replay of a log read from a
    JSON snapshot in the middle gives the same `Snapshot` (property test); against Postgres, 110 PRs submitted,
    tested, merged, closed or failed (more than 500 events) give the same projections and the same
    snapshots after `rebuild`, and a fact that changes nothing is not written.
  - Done (reconciliation, D100 to D105): `Module` contract of `cihub-platform` (name, executors, scheduler
    loop) and `module_enabled`; a queue of an organization without the module does nothing. Table `queue_inbox`
    in deposit order; `deposit_fact`, `pending_facts`, `mark_facts_processed` and `ensure_user` in
    `cihub-store`. In `cihub-mq-core`: `Item::submitted_at`, `Observed::observed_at`, rule
    `not_ready_timeout_expired`, `Desired::observe` (`Read`s at fixed points after the submission) and
    `Plan::wake_at`. In `cihub-mergequeue`: `Reconciler` (one transaction per queue: deliveries, facts of
    `queue_inbox`, `plan`, decisions, jobs, `next_tick_at`), the pass of the deliveries of a connection without a queue
    (access lost or restored, commands), `run_scheduler`, the `/cihub merge` command,
    the `refresh_prs` and `submit_pr` jobs with the exception of the queue status. `cihub-server` starts the loop
    in the `scheduler` role and the executors in the `worker` role. A delivery of a connection without a queue
    wakes up the schedulers. `ScmEvent::PrChanged` carries `label_added`, the label that an action has set. Tests:
    table of the commands, of the exception and of the keys; in the core, timeout, read points and wakeup;
    against Postgres and the simulator, with the real loops of the scheduler and the worker: 20 PRs submitted by
    command or by label in various states reach `pending` or `not_ready` with the right block
    reason, a command without the `write` right submits nothing, the replay gives back the same projections, and a
    lost access pauses the queue; a reconciliation stopped before its commit writes nothing, and the next one
    does all the work once; a 429 from GitHub during a job pauses the connection in the database, and no
    job of the connection leaves before the reset (test of lot 4).
  - Done (partitions and pause, D106, D113): the `maintenance` job deletes an `events` partition 90 days
    after the end of its month, when a snapshot covers each of its events; a queue takes a snapshot
    when its oldest event after the last snapshot is older than 30 days, and an hourly pass of the
    scheduler wakes it up for that; `seq` follows the last snapshot; `rebuild` refuses a log that is no longer
    whole, and the loading refuses a log with a gap. A submission waits for the end of a pause of the queue.
    Tests: in the core, a submission during a pause creates its item at the resume; against Postgres, a
    partition without a snapshot stays, then leaves after the snapshot, the queue loads from it, its `seq`
    continues, and `rebuild` refuses; table of the partition names.
  - Done (manual test, October 1, 2026, on `getsyncr/contracts`): submission by command, `not_ready` on a
    missing required check then `pending` after the removal of the rule, read points at 1 and 3 minutes, push
    on a PR in the queue (`pr_pushed_to`), new submission then closing (`pr_closed`). Not done by hand:
    label on a PR outside the queue, access lost, hard stop, conflict, permission refused; the automatic
    tests cover them.
  - Remaining, moved to lot 9: `cancel` and `restart` commands, cache of the permissions, message of a refused
    submission, block reason that names the required check (D112). A check on the head of a `not_ready` PR is read
    only by the polling: it arrives through the connection, without a queue.
  - Deviations from `docs/DATA_MODEL.md`, updated in the same change: the projections keep only what
    the log contains. `queue_items` has neither `pr_title` nor `pr_base_branch`, and keeps `pr_author_id` instead
    of a login (D42); `seq` is the rank of the admission in the log, not a Postgres sequence; the columns
    of `test_runs` of lot 6 arrive with it. A queue starts in `single` mode: a mode comes only from the fact
    `queue.settings_changed`, so that the replay does not depend on the settings of the day.
  - Validated deviations (Nicolas Lecoy, October 1, 2026): no `queues.settings` column before lot 8 (D110);
    `ScmEvent::PrChanged` gains `label_added` (D107); a command goes through the connection and `submit_pr` finds
    its queue, and a submission by label has the `system` actor (D108); beyond 50 PRs, a read of the polling
    adds its rank to the key (D109); a `Module` always has a scheduler loop (D111).

### Lot 6: test cycle

- **Goal**: test a PR on top of its predecessors and conclude the run.
- **Content**: `assemble_test_branch` job (`cihub-temp` ref, merges API, `cihub-merge` ref, deletion of the temporary
  ref, `conflict_culprit`); modes `draft_pr`, `push` and `inplace`; resolution of the required statuses (UI,
  repository file, protection) and their source; `check_observations`; success, failure, timeout; late reports
  ignored; pending failure at depth 0; restart of the downstream; deletion of the test branches after
  `keep_test_branches_minutes`.
- **Frozen interfaces**: format of the branch names; body of the draft PR; `GET /api/v1/test-runs/{run_id}`.
- **Exit criterion**: on the simulator, a queue of 10 PRs of which 2 fail tests each PR on top of
  its predecessors, concludes each run with the right reason and restarts the downstream; a conflict identifies the
  right culprit PR; a check that never reports gives `test_run_timeout`.
- **Tracking**:
  - Done: the decisions of the lot (`DECISIONS.md` D114 to D118). Migration `20261001120000_test_cycle.sql`: enums
    `check_kind` and `check_conclusion`, columns `base_sha`, `head_shas`, `tested_pr_numbers` and `last_report_at`
    of `test_runs`, table `check_observations` with row level security. In `cihub-domain`, the names of the
    test branches (`test_branch`, `temp_branch`, `run_of_test_branch`); in `cihub-mq-core`, the desired state
    carries the heads with their PR number (`TestBranch { predecessors, members }`, `Head`), and a
    `check.observed` is accepted for an `assembling` run whose assembly fact has given the test
    SHA. In `cihub-platform`, a check is routed to its queue by the name of its test branch, or by its test
    SHA once the branch is deleted. In `cihub-mergequeue`: `assemble_test_branch` job (required statuses
    from the dashboard, `.cihub.yaml` or `.cihub.yml` read at the `base_sha`, then the protection of the target
    without the status of the queue; temporary branch at the `base_sha`, merges of the predecessors then of the members,
    test branch, deletion of the temporary one; draft PR in `draft_pr` mode, nothing in `push` mode;
    `conflict_culprit`; errors `permissions_block_test_branch`, `draft_prs_not_allowed`,
    `no_required_statuses`, `config_*`, `retry_exhausted` after 5 attempts); `cleanup_test_branch` job; the
    scheduler writes the facts of `queue_inbox` before those of the deliveries, translates a check into `check.observed`,
    defers a delivery that arrived before the assembly fact of its run, stores a late report, requests
    the assembly of the `assembling` runs and the cleanup of the finished runs; projections of
    `check_observations` and of the new columns; `submit_pr` creates the accounts in its own transaction
    (deadlock with the reconciliation on `users`). Tests: tables of the keys, of the body of the draft PR and of the
    configuration file; on the simulator, without Postgres, the assembly and its errors (conflict with the base,
    with a predecessor, between predecessors, protection, drafts refused, sources of the required statuses);
    against Postgres and the simulator, with the real loops: 10 PRs of which 2 red CIs, a conflict and a silent check
    (the test plays the merge job of lot 7); a protection that refuses the test branch.
  - Done (manual test, October 1, 2026, on `getsyncr/contracts`, with a `ci` workflow on `pull_request` and
    `required_statuses: [ci]` in `.cihub.yaml`): nominal path (assembly, draft PR, green `ci`,
    `tests_passed`, cleanup in less than 30 s); two runs in parallel, each on top of its
    predecessors; red CI, restart of the budget then `test_run_required_status_failed`; conflict at the assembly
    with `conflict_culprit` on the PR in front; closing of a PR in the queue (`pr_closed`, downstream dissolved then cancelled);
    resubmission of a failed PR. Not done by hand: late report, hard stop, timeout; the automatic
    tests cover them. Observed in passing: (a) a PR labeled before its own merge had a
    run formed by the push of that merge, whose assembly received a 422 from GitHub (head already in `main`),
    treated as transient then abandoned when `manual_merge` replaced the run; an immediate abandonment
    would be more correct (lot 7, with the external changes); (b) the closing of a PR at the head dissolves and
    reassembles the groups behind it before their own closings arrive: two assemblies
    for nothing, to be measured in lot 7; (c) the deliveries of an installation without a connection (D86) are never
    deleted by the maintenance, which deletes only the processed deliveries.
  - Remaining, moved to the following lots: the `inplace` mode, with a later lot (`DECISIONS.md` D119); the cancellation
    of the workflows (D116); the reread of the head of the target and of the checks of a silent run, the existence
    of the refs and the 422 above (lot 7); the route `GET /api/v1/test-runs/{run_id}` (lot 9, D115); the column
    `deadline_at` with `queues.settings` (lot 8); the retention of the deliveries without a connection (lot 7 or 15);
    an automatic test of `start_test_run_retry_exhausted` (the backoff of the jobs makes it slow).
  - Validated deviations (Nicolas Lecoy, October 1, 2026): D114 to D119. The simulator did not gain a targeted fault
    on a SHA: its three-way merge gives a conflict from the content of the files. And the following
    deviations: (1) `TestBranch` of the desired state carries `predecessors` and `members` (`Head`s with the
    PR number and the SHA) instead of `head_shas`, so that the job names the culprit PR and the tested PRs;
    (2) the cleanup is not in the desired state, because a finished run leaves the snapshot: the scheduler
    requests it when it writes the decision that finishes it (D117); (3) the reconciliation writes the facts of
    `queue_inbox` before those of the deliveries, and defers a check delivery that arrived before the assembly
    fact of its run (it is not marked processed); (4) `conflict_culprit` says what the
    member conflicts with: `base_branch` when it does not merge on the `base_sha` alone, otherwise the first PR
    in front of it that does not merge on `base_sha` plus the member (a second pass of merges, in the case of the
    conflict only); a conflict between two predecessors makes the job retry, because the run of the second one
    decides; (5) `check_observations` has no `status` column (the code has only `conclusion`, `NULL` as long
    as the check has not concluded); (6) `test_runs.deadline_at` is not projected: it depends on
    `testing_timeout_minutes`, which a projection of the log does not know before `queues.settings` (lot 8);
    (7) an assembly failure fails the item without a restart of the budget, as the tables of lot 1
    say (`testing` → `failed` for `start_test_run_*`); (8) the `inplace` mode is not delivered: it changes the
    head of the PR, which the core reads as a push (`pr_pushed_to`), and the routing of its checks has no
    test branch; its design needs a decision: exemption in `pr_changes` when the observed head is
    the test SHA of the live run, `sha` guard of the merge on this SHA, formation only without a live
    predecessor, `head_branch` in `PrSnapshot` (contract of the `Scm` trait and recorded cassette), routing of the checks
    by SHA; it leaves lot 7 and `cihub-config` refuses `trigger: inplace` until then (D119).

### Lot 7: merge and external changes

- **Goal**: merge in order and stay correct when the world changes around the queue.
- **Content**: merge step (`squash`, `merge_commit`, `rebase`, `fast_forward`, `sha` guard,
  `merge-commit-title`, retries on 5xx, 405, 409); `merged_sha`; external push on the
  target; manual merge; push, base change, draft, closing, label removed; repair loop
  (live PRs, silent checks, head of the target, existence of the refs); `POST /queues/{id}:resync`; `sim`
  task. Decisions of the lot: `cihub-config` refuses `trigger: inplace` (`DECISIONS.md` D119) and `fast_forward` outside
  `sequential`, and `fast_forward` advances the target on the test SHA, in `cihub-github` and the simulator
  (D120, D124); a push on a PR in the queue cancels it (D121); precise reasons for a direct merge failure from
  `pending` (D122); `merge_pr` job (D123); title and body in `PrSnapshot` (D126); repair jobs (D127);
  `resync` without its route (D128); fault injection test (D129). The direct merge on main moves to lot 8
  (D125). Taken over from lot 6 (D130): immediate abandonment when the assembly receives a 422 because a head is already
  in the target; measurement of the useless assemblies after the closing of the PR at the head; retention of the deliveries
  without a connection.
- **Frozen interfaces**: `merged`, `cancelled` and merge failure reasons (`merge_rejected`…).
- **Exit criterion**: the fault injection test of `docs/RELIABILITY.md` (5xx, 429, lost, duplicated and
  out-of-order webhooks, processes killed at random instants) passes on 1,000 seeds: each green PR is
  merged exactly once, in the order of the graph, and each red PR carries a reason.
- **Tracking**:
  - Done: the decisions of the lot (`DECISIONS.md` D119 to D130). `cihub-config` refuses `trigger: inplace` and
    `fast_forward` outside `sequential`; the transition table accepts the precise reasons of a direct merge
    failure from `pending`. In `cihub-scm`, `PrSnapshot` gains `title` and `body`, `PrMerge` gains `tested_sha`,
    `ScmError` gains `NotFastForward`; `cihub-github` and the simulator advance the target on the test SHA in
    `fast_forward`; the simulator gains `edit_pr` and random faults drawn from a seed (`RandomFaults`).
    In `cihub-mq-core`, `Merge` carries the run and the test SHA, `Desired` gains `check_reads` and `target_read`,
    `observe` rereads each live PR, and `wake_at` follows these reads. In `cihub-mergequeue`: `merge_pr` job
    (commit title, attempts, mapping of the errors), `refresh_checks` (recreates the test branch)
    and `refresh_target` jobs, `resync` function, abandonment of the assembly when a head is already in the target.
    `maintenance` deletes the unrouted deliveries older than 7 days. `mise run sim` task and nightly workflow
    `.github/workflows/sim.yaml`. Tests: tables of the title, the errors and the keys; core (reads of the PRs, the
    checks and the head); against Postgres and the simulator, the merge (recovery after 5xx, `head_changed`), the
    lost webhooks repaired (report, test branch deleted, external push), `resync`, and `fault_injection`.
  - Exit criterion: `mise run sim` passes on 1,000 seeds on October 2, 2026, with D131 to D133 (1,530 s).
    On October 1, before D133, it failed at seed 412.
  - Done (October 2, 2026, D131 to D133): wakeup of the live queues at the startup of the scheduler; the `merge_pr` job
    reads the PR before merging it, and an already merged PR gives `pr_already_merged`, which `plan` translates into
    `manual_merge`; the assembly counts only the 422s in its 5 attempts and waits for the other transient
    errors until 15 minutes after the start of the run; `MAX_ATTEMPTS` goes to 11. Tests: core
    (`pr_already_merged` restarts the downstream), table of `give_up`, and against Postgres the wakeup at startup and a
    PR merged by hand during its test.
  - Done (manual test, October 1 and 2, 2026, on `getsyncr/contracts`): merge at startup of a
    `tests_passed` item of lot 6 (#16, squash by the App, title of the PR); three PRs in pipeline merged in order,
    with the title of the `merge-commit-title:` line; red CI after the restart of the budget (#17); PR in conflict
    with the target kept in `not_ready` (`merge_conflict`, #15); push on a PR in test (`pr_pushed_to`);
    server stopped during the tests, merge by hand during the stop, then resume without loss. This test
    found the defects fixed by D131 (a queue without a wakeup after the startup) and D132 (the merge job
    credited itself with a merge done by hand).
  - Done (manual test, October 2, 2026, PR #45): test branch deleted 1 s after the transition to
    `in_progress`; 10 minutes after the last report, `refresh_checks` recreated it at the exact test SHA.
    Observed limit: GitHub closes the draft PR when its branch disappears, and the recreation does not reopen
    it; in `draft_pr` mode, the CI therefore does not run again and the run stays silent until its timeout.
    Reopening the draft PR after the recreation needs a decision, postponed to later (validated on
    October 2, 2026).
  - Done (October 2, 2026): the GitHub responses are recorded again (D126, 57 exchanges, with `title`
    and `body`), with the `cihub` ruleset set for the duration of the recording to the mode "one approval and the
    `cihub-contract-ci` status", then returned to its test state. The PRs of the suite are recreated (#47 merged, #48 and #49
    closed, their branches kept for a next recording).
  - Remaining, moved to the following lots: the useless assemblies after the closing of the PR at the head (lot 12);
    reopening the draft PR after the recreation of a test branch (decision postponed); the direct
    merge on main (lot 8, D125); the route of `resync` (lot 9, D128).
  - Validated deviations (Nicolas Lecoy, October 2, 2026): (1) the key of the merge is `merge_pr:<item_id>-<run_id>:1`, not `merge_pr:<item_id>:1`: an
    item proven again by another run (after a restart) requests a new job, and the job
    checks that `queue_items.verified_by_run_id` is still this run; (2) `Merge` also carries `run_id`;
    (3) `target.pushed.sender_id` is optional, empty for a read by the tick; (4) as long as a `merge_pr` job of
    the queue waits or runs, a push delivery on the target whose SHA is not a known merge waits, and
    `refresh_target` does not read the head: otherwise the push of the merge of the queue, arrived before its fact, restarts
    the whole queue; (5) new variant `ScmError::NotFastForward`; (6) the head of the target is read only
    when the queue has work, and every minute when it does not know it: before this lot, a queue without a push
    on its target formed no group; (7) for D130, the abandonment comes from a head "already contained" at the merge
    of a member, or from a 422 "No commits between" at the opening of the draft PR; (8) rates of the fault
    injection test: 1% of 5xx and 1% of 429 per call, 10% of deliveries lost, 10% sent twice,
    order changed, a stop of the processes in 15% of the rounds, 5 PRs of which 25% red; the submissions
    arrive without loss (the sweep of lot 4 covers them); a stop cancels the tasks and closes the pool of the
    process, so that Postgres aborts its transactions as at the end of a real process.
  - Observed before D133: a green PR could fail with `start_test_run_retry_exhausted`, because the assembly
    counted each 5xx, 429 and stop in its 5 attempts; with 2% of faults per call, about one seed
    out of 400.

### Lot 8: queue policy

- **Goal**: make the queue configurable without touching the core.
- **Content**: `.cihub.yaml` or `.cihub.yml` file read on the target at the `base_sha`, the only source of the settings
  (`DECISIONS.md` D134), and effective source of each setting (default, strategy, file); conditions language
  (`queue_conditions`, `merge_conditions`, injection of the protections, routing between queues of the same branch,
  `priority_rules`);
  strategies as tables of default values; adaptive speculation window; failure budget; priorities;
  states `paused`, `draining`, `switching_modes`.
- **Frozen interfaces**: schema of the configuration file (version 1); grammar of the conditions; names of the
  settings (`docs/CONFIGURATION.md`).
- **Exit criterion**: table tests of the grammar of the conditions (parsing and evaluation); on
  the simulator, a `pipelined` queue under 20% of random failures contracts its window to the floor then
  reopens it; a PR cannot change its own conditions (dedicated test); each strategy is a table without
  code in `cihub-mq-core`.
- **Tracking**:
  - Done: `cihub-conditions` (grammar `[-] attribute [op value]` with `and`, `or`, `not` blocks, attributes of
    `docs/ALGORITHMS.md`, evaluation on a PR context, errors with position);
    `cihub-config` (model of the file version 1, choice between `.cihub.yaml` and `.cihub.yml`, table of the strategies,
    merge of the sources with the source of each setting, ranges, errors with line and column, failure reason of
    each error).
  - Done (October 2, 2026, D134): the file is the only source of the settings; `cihub-config` loses the `ui`
    source, and `RequiredStatusesSource` loses `ui_override` (code, Postgres enum, documents).
  - Done (October 2, 2026, D135 to D138): `queues` gains `name`, unique per repository, and the routing of a delivery
    gives a queue only when its branch has only one; the `load_config` job reads the file on each new
    head of the target and keeps the last valid file (`config_text`, `config_sha`, `config_error`); each
    reconciliation and each job resolves the settings of its queue (end of the global settings of the module);
    `commands_enabled` is checked by `submit_pr` on the queue found; a mode change of the file writes
    `queue.settings_changed`. Tests: table of `RepoConfig::settings`, required statuses of a queue without an entry,
    routing with two queues on a branch, and against Postgres and the simulator, a queue that follows its file
    (valid file, refused, then mode change).
  - Done (October 2, 2026, D141): adaptive window in `plan` (+1 per group that passes, ÷2 per confirmed test
    failure, bounds of the settings). Exit criterion: `crates/mq-core/tests/window.rs`, a `pipelined`
    queue with 21 red PRs out of 100 (fixed seed) goes down to the floor several times, then 40 green PRs
    reopen it up to the ceiling; a lowered ceiling brings the window back within its bounds. The failure budget was already
    in `plan` (restarts, no restart for a conflict or a configuration error); its extra restart when
    `FlakeOracle` recognizes the failing tests arrives in lot 14.
  - Done (October 2, 2026, D142 to D145): `queue_conditions`. `cihub-conditions` gains `Conditions::unmet` and the
    neutral vocabulary (`review-decision`, permissions `none` to `admin`); `Permission` moves into
    `cihub-domain` in place of `RepoPermission` and of the `Permission` of `cihub-conditions`. `PrSnapshot` gains the
    head branch, the fork, the creation date and the reviews; the `Scm` trait gains `pr_files` and
    `unresolved_threads` (GitHub and simulator). `submit_pr` and `refresh_prs` evaluate the conditions of the queue
    for an item not yet admitted; `pr.observed` carries `unmet_conditions`, `item.submitted` carries
    `sender_permission` (column `queue_items.sender_permission`, enum `permission`), and `plan` blocks the item
    with `queue_conditions_unmet`. Tests: `unmet` and the vocabulary, the contract suite on the simulator, an
    item blocked then admitted in the core, and against Postgres and the simulator a PR that waits for its label and
    another for its files.
  - Done (October 2, 2026): the GitHub responses are recorded again (D143, 59 exchanges, with the reviews,
    the files and the review threads), with the `cihub` ruleset set for the duration of the recording to the mode "one
    approval and the `cihub-contract-ci` status", then returned to its test state. #48 and #49 are reopened, #51
    merged by the suite.
  - Done (October 2, 2026, D146, D147): `merge_conditions` accepts only top-level `check-success = <name>`
    (configuration error otherwise), and their names are added to the required statuses of the run;
    `branch_protection_injection` decides whether the checks required by the rules enter the required statuses of the test
    SHA (`both`, `merge`) and the `queue_conditions` of the PR (`both`). Tests: validation of the file,
    required statuses according to the file, the `merge_conditions` and the injection, conditions injected at admission, and
    against Postgres the block reasons of a failing required check.
  - Done (October 2, 2026, D148): routing between the queues of a branch and `priority_rules`. `/cihub merge` gains
    `--queue=<name>`, and its priority becomes optional; `submit_pr` chooses the named queue, otherwise the first
    queue of the branch, in the order of the file, whose conditions pass, otherwise the first one, then applies the
    first true priority rule when the command does not give one. When a branch has several queues, the pass
    of the deliveries of the connection sends a PR change to the queue of its live item, or a submission
    by label, and a push on the branch requests `refresh_target` (class `interactive`) from each of its queues
    (D150).
    Tests: the command, and against Postgres and the simulator two queues on `main` (routing by condition, by
    `--queue`, priority of a rule and of the command, push read by both queues).
  - Exit criterion (October 2, 2026): the table tests of `cihub-conditions` (parsing and evaluation); the
    window of a `pipelined` queue under 20% of failures goes down to the floor then back up to the ceiling
    (`crates/mq-core/tests/window.rs`, D141); a PR cannot change its own conditions
    (`a_pr_cannot_change_its_own_conditions`: a PR that removes the `queue_conditions` and changes the required
    statuses in its own file waits for the conditions of the target, and its run requires the statuses of the
    target); each strategy is a table of `cihub-config`, without code in `cihub-mq-core`. `mise run sim`
    passes on 1,000 seeds at each step (1, 2, 3a, 3c, 4 and 5).
  - Remaining, moved to later: `schedule` attribute (postponed: the time
    zones need a database embedded in the pure core).
  - Deviations: started before lots 2 to 6, because the two crates are pure and depend only on `cihub-domain`.
    `cihub-mergequeue` depends on `cihub-conditions`, in addition to the crates of D93, to evaluate the conditions in
    its jobs (D142).
  - Done (October 2, 2026, D149, D150): the option is written `--queue=<name>`; a push on a branch with several
    queues waits for the next pass as long as a merge of one of its queues is in flight, then requests `refresh_target` from
    each one. Test: against Postgres, a push that waits for the end of a merge, then reaches both queues.
  - Done (October 2, 2026, D151, D152): a submission in `draining` or in `switching_modes` creates a
    `cancelled` item (reasons `queue_draining` and `queue_switching_modes`, transition "nothing → `cancelled`"), which never
    becomes the live item of its PR; the projection rebuilds it from its `item.submitted`. Setting
    `resubmit_on_push`: `refresh_prs` deposits a new `item.submitted` for the new head of a PR in the queue,
    with the same priority, the same source and the same options, and `plan` creates the new item after
    the cancellation of the old one (`pr_pushed_to`). Tests: in the core, the refusal while draining and a resubmission at
    the end of the queue; against Postgres, both.
  - Done (October 2, 2026, D153): direct merge, disabled by default and in the table of the strategies. `plan` proposes
    a direct merge (`Merge` without a run) for the first `pending` item of a queue without a live group, once,
    and does not form a group for it until then; the `merge_pr` job checks the required statuses on the head of the
    PR (computation shared with the assembly), the mergeability and that the PR contains the head of the target (new
    `is_ancestor` method of the `Scm` trait, GitHub and simulator); otherwise it returns `direct_merge_unavailable`, a fact
    that marks the item. Tests: in the core, a direct merge then an item that goes through the tests, and the default
    `off`; against Postgres and the simulator, an up-to-date PR merged without a test branch and a PR that is behind tested.
    The GitHub responses are recorded again (62 exchanges, with `compare`); the `cihub` ruleset is set
    to recording mode for the duration of the recording, then returned to its test state; #53 merged by the suite.
  - Observed: before D151, `apply` ignored a submission in `draining` or in `switching_modes`, without a trace in the
    log.


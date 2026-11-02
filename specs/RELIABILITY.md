# Reliability and failure handling

This spec gives the promises of the queue under failure, and the rules that keep them: what each failure costs and
who catches it up. It does not give the numbers of each retry or delay; they are constants in the code that each
section names. The mechanisms are the event log, idempotent convergence, the repair loop, the scheduled SCM budget
and explicit degraded modes.

## Promises

- **REL-1** A PR merges only after a test run that passed on a commit whose tree equals what the target branch
  will contain (principle IV).
- **REL-2** No intent is lost: a submission, a command or a check result survives a crash, a lost webhook, an SCM
  outage and concurrent operators, and is at worst delayed (principle V).
- **REL-3** GitHub is the source of truth for the git state, Postgres for the queue state. When they disagree, the
  queue believes GitHub and records the transition with its reason (`manual_merge`, `pr_closed`).

## Ingestion and deliveries

Code: `crates/platform/src/inbox.rs` (`MAX_DELIVERY_ATTEMPTS`), `crates/platform/src/sweep.rs`
(`SWEEP_DELIVERIES`, `SWEEP_LOOKBACK`), `crates/mergequeue/src/scheduler.rs`

- **REL-4** The ingestion stores a delivery before any processing and answers 2xx only after it stored it.
  GitHub does not retry a failed delivery: the `sweep_deliveries` job lists the recent attempts and redelivers each
  delivery that no attempt delivered.
- **REL-5** A delivery is unique by its `X-GitHub-Delivery` while it waits; once processed it is deleted, and its
  jobs are keyed by its identifier, so a delivery processed again asks each job at most once.
- **REL-6** The pass of the deliveries reads each one in its own savepoint. A delivery that always fails is set
  aside after `MAX_DELIVERY_ATTEMPTS`, and the deliveries of the other organizations go on.
- **REL-7** Deliveries of one queue are processed in order. A report for a SHA that no run expects is stored and
  ignored (`specs/DOMAIN.md`, DOM-17, for a late report of a check).

## Jobs and idempotency

Code: `crates/platform/src/jobs.rs` (`claim_jobs`, `JOB_LOCK`, `MAX_ATTEMPTS`), `crates/platform/src/lease.rs`

- **REL-8** A crash during a reconciliation rolls back its transaction; the next holder of the lease redoes the
  same work, and no partial write exists.
- **REL-9** A job of a stopped worker is taken back when its lock expires. Each executor reads the real state
  first (does the branch exist, is the PR merged, does the comment exist) and skips what is already done.
- **REL-10** A Postgres failover fails the reconciliation in flight, which is replayed; jobs survive.

## SCM errors at assembly and merge

Code: `crates/mergequeue/src/assembly.rs` (`ASSEMBLY_ATTEMPTS`, `ASSEMBLY_WAIT`), `crates/mergequeue/src/merge.rs`
(`MERGE_ATTEMPTS`, `MERGE_RATE_LIMIT_WAIT`)

- **REL-11** A 5xx at merge is retried with the backoff of the jobs, then gives `merge_repeated_server_errors`.
- **REL-12** Before it gives up on "merge already in progress", the merge job reads the PR: merged is a success,
  because the merge can have succeeded. A 403 that names the asynchronous merge endpoint merges again through it.
- **REL-13** A refused `sha` guard gives `pr_updated_at_merge_time`; a PR found merged before the call becomes
  `merged` with `manual_merge`.
- **REL-14** A rate limit pauses every job of the connection until its reset. A run waits instead of failing, up to
  a bound, then gives `merge_rate_limit_exceeded` or `start_test_run_retry_exhausted`.
- **REL-15** At assembly, a validation error rereads the ref and goes on if it already matches; a transient error
  (5xx, rate limit, shutdown) does not count as an attempt. A 403 at ref creation gives
  `start_test_run_permissions_block_test_branch`.

## External drift

Code: `crates/mq-core/src/apply.rs` (`target_pushed`), `crates/mergequeue/src/repair.rs`

- **REL-16** An outside push on the target invalidates every run (`specs/ALGORITHMS.md`, ALG-37). A test branch
  deleted by hand is created again at its test SHA by the read of the checks.

## Degraded modes

Code: `crates/github/src/budget.rs` (`Budget::admit`, `Budget::observe`, the thresholds), `crates/domain/src/tenancy.rs`
(`ConnectionMode`)

- **REL-17** A connection is `throttled` when its budget is low or a secondary rate limit hit it. Only the calls of
  class `speculate` wait; the commands, the statuses and the merges go on. The window does not change.
- **REL-18** A connection is `scm_unavailable` after consecutive failures: only one probe call goes out at an
  interval, and the jobs accumulate with their class. A call that answers ends the mode.
- **REL-19** A mode applies to one connection and is shown with its cause. A Postgres outage stops every process,
  and the recovery is the replay of the inbox and of the expired leases.

## Repair loop

Code: `crates/mq-core/src/plan/reads.rs` (`wake_at`), `crates/mergequeue/src/repair.rs` (`REFRESH_CHECKS`,
`REFRESH_TARGET`, `sync_queue`), `crates/mergequeue/src/jobs/refresh.rs` (`REFRESH_PRS`),
`crates/mergequeue/src/config.rs` (`READ_SETUP_EVERY`)

- **REL-20** The plan requests the due reads of the live PRs, of the checks of a silent run and of the head of the
  target (`specs/ALGORITHMS.md`, ALG-43, ALG-44). Each read gives its facts back through `queue_inbox`. A lost
  webhook costs at most one read interval.
- **REL-21** While a merge job of the queue waits or runs, or its fact waits in `queue_inbox`, a push on the target
  that is not a known merge waits for the next reconciliation, and the target is not read. The push of a queue
  merge can arrive before the fact of the merge.
- **REL-22** A check that concludes on the head of a `not_ready` item requests at once a read of its PR, in class
  `interactive`; the periodic read stays the fallback when that delivery is lost.
- **REL-23** At each new head of the target, `load_config` reads the configuration file. A rejected file leaves the
  queue on the last valid file.
- **REL-24** The setup of each queue is read again every `READ_SETUP_EVERY`, spread over the hour per queue,
  because the SCM sends no event when a rule changes.
- **REL-25** `sync_queue` (`POST /orgs/{org}/repos/{repo}/queues/{queue}/sync`) requests all these reads at once in
  class `interactive`: the way out after an incident.

## Access loss

Code: `crates/platform/src/installations.rs` (`SYNC_AFTER`, `request_installations_sync`),
`crates/platform/src/connections.rs` (`SYNC_REPOS`), `crates/platform/src/persons.rs` (`ACCESS_LIFETIME`)

- **REL-26** A repository removed from the installation, or a suspended or removed installation, gives
  `access.lost` to its queues: they stop and keep their state, which the admin can still change, and their items
  stay (`specs/DOMAIN.md`, DOM-19). The return of the access gives `access.restored`: the queues go on in their
  state, and the reconciliation checks the items again.
- **REL-27** Without its webhook, the access change is caught by the sync of the installation at the next delivery
  of an active connection, by a sync job that the SCM refuses, or by the full daily sync. An unknown installation
  is created by its first delivery or by that sync.
- **REL-28** An SCM outage during the check of the access of a person fails only the sessions whose read is old;
  queues, jobs and API tokens never check the access of a person. A token of a person that the SCM refuses ends
  every session of the person.

## Consistency invariants

Code: `crates/mq-core/src/check.rs` (`check`, `Violation`)

- **REL-29** `check` gives every consistency rule that a snapshot breaks. The rules are in `Violation`; the tests
  of `cihub-mq-core` call it.

## Fault-injection oracle

Code: `crates/mergequeue/tests/postgres.rs` (`fault_injection`), `crates/scm/src/sim/`

- **REL-30** The fault-injection test plays the queue against the GitHub simulator with random 5xx and rate limits,
  lost, duplicated and reordered webhooks, and processes killed at random. The oracle: each green PR merges exactly
  once, in queue order, each red PR fails with a reason, and a rebuild by replay gives the same projections.
- **REL-31** It runs with the default strategy and with `batched`, so that batches, bisection, depth and
  optimistic merge are covered.

## Capacity limits

Code: `crates/platform/src/jobs.rs` (`JOBS_PER_CONNECTION`)

- **REL-32** A connection has at most `JOBS_PER_CONNECTION` jobs running. A connection at its quota is left out
  before the claim, so that its backlog does not hide the jobs of the others.

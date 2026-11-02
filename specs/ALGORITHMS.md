# Queue algorithms

The rules of the core of the queue. One reconciler per queue recomputes a merge graph from the ordered admitted
items; a run is valid only while the predecessors of its group and its base SHA do not change. The same rules
cover the simple and parallel modes, batching and bisection. The source of truth is `cihub-mq-core`; the settings
and their defaults are in `crates/config/src/settings.rs` (`settings!`), the meaning of their values in
`crates/domain/src/settings.rs`.

## Conditions

Code: `crates/conditions/src/`, `crates/mq-core/src/plan/rules/readiness.rs` (`conditions_of`)

- **ALG-1** `queue_conditions` say what a PR needs to be admitted, `merge_conditions` what the test commit needs
  to merge. They are two sets because one set is the most cited limit of the native GitHub queue.
- **ALG-2** `branch_protection_injection` decides where the checks required by the rules of the branch go: the
  conditions of the PR, the required statuses of the test commit, both, or none.
- **ALG-3** Conditions count only before admission: an admitted item stays when a condition becomes false.
- **ALG-4** A branch has one queue, and a PR enters the queue of its base branch.

## Strategies

Code: `crates/config/src/settings.rs` (`strategy_layer`)

- **ALG-5** A strategy is a table of default values on the other settings, so a new strategy needs no code in the
  core. A setting that a source sets wins over the strategy, and the interface shows the deviation.

| `strategy`            | For whom                                                                   |
| --------------------- | -------------------------------------------------------------------------- |
| `sequential`          | Small repositories, short CI, no temporary branch, no protection bypass    |
| `pipelined` (default) | Repositories with moderate traffic                                         |
| `batched`             | Long and costly CI, many PRs per hour                                      |
| `parallel`            | Monorepos that publish their impacted targets (Bazel, Nx, in-house script) |

## Order and priority

Code: `crates/mq-core/src/graph.rs` (`insertion_index`, `position_at`, `renumber`)

- **ALG-6** Only `position` orders the queue. The priority decides only where a new item goes, so that a priority
  never reorders the items already in the queue.
- **ALG-7** `urgent` goes in front of every non-urgent item except those in `tests_passed`: their merge can be in
  flight, and a test that skipped them would not contain what lands (`specs/tla/Queue.tla`, `urgent_first`).
- **ALG-8** `urgent` is the only insertion that interrupts a test. Another priority goes before the first
  `pending` item with a greater value: behind the groups in test, FIFO among equals.

## Merge graph

Code: `crates/mq-core/src/graph.rs` (`targets`, `overlap`, `predecessors`)

- **ALG-9** An item has a set of impacted targets: those it sent in parallel mode, else `All`.
- **ALG-10** `j` is a predecessor of `i` when `j` is ahead of `i` and a chain of items ahead of `i`, with pairwise
  overlapping targets, links them. The simple mode is one chain; the parallel mode gives lanes without a name.
- **ALG-11** An item is tested on top of exactly its predecessors and merges only after all of them merged.

## Test branch assembly

Code: `crates/mergequeue/src/assembly.rs` (`ASSEMBLE_TEST_BRANCH`)

- **ALG-12** The executor merges the predecessors, then the members, on a temporary branch at `base_sha`, then
  creates the test branch at the result, so that the CI sees one push, for the final commit only.
- **ALG-13** A conflict on a member gives `start_test_run_merge_conflict` with a `conflict_culprit`: `base_branch`,
  or the first PR ahead that conflicts with it, which a second pass finds on a conflict only.
- **ALG-14** A conflict on a predecessor concludes nothing: the run of that predecessor meets it and decides.
- **ALG-15** The branch prefixes cannot change while the queue has live items.

## End of run

Code: `crates/mergequeue/src/assembly.rs` (`required_statuses`), `crates/mq-core/src/plan/rules/runs.rs`
(`run_conclusion`)

- **ALG-16** The required statuses come from the configuration file at `base_sha`, else from the rules of the
  branch; never from the PR, so that a PR cannot weaken its own barrier. None at all fails the run.
- **ALG-17** Names match exactly: a required name that never reports gives `test_run_timeout`, not a failure.
- **ALG-18** A report on the test SHA of a run that is no longer live is stored and ignored.

## Speculation window

Code: `crates/mq-core/src/plan/rules/formation.rs` (`current_window`, `window_to`, `window_has_room`)

- **ALG-19** The groups in test fit an adaptive window, as in Zuul: it starts at the floor, grows by 1 at each run
  that passes outside bisection, and halves at each confirmed failure. A conflict, a configuration error or a
  merge failure does not change it.
- **ALG-20** A smaller window interrupts no run. A queue with flaky tests closes by itself and stops wasting CI.
- **ALG-21** A batch takes one slot; bisection runs take slots from their own pool, `bisection_concurrency`.
- **ALG-22** A throttled connection does not change the window: assembly jobs, class `speculate`, wait for the
  budget (`specs/RELIABILITY.md`, REL-17).

## Batching

Code: `crates/mq-core/src/plan/rules/formation.rs` (`next_group`)

- **ALG-23** A batch is the longest series of consecutive waiting items with the same lane and profile, not
  `no_batch` and not blocked, up to `batching.size`.
- **ALG-24** A batch starts when full, when its oldest member waited `batching.max_wait_minutes` since its
  admission, or at once when the next item cannot join it.
- **ALG-25** When a member leaves, the batch dissolves and the others restart with `batch_member_left`, because
  their run tested its code. Each member merges as its own commit, in batch order.

## Bisection

Code: `crates/mq-core/src/plan/rules/bisection.rs` (`bisection`)

- **ALG-26** A confirmed failure of a group of several PRs bisects it at once, without the failure budget. The
  groups behind it restart, and no group forms on top of a PR in bisection until it ends.
- **ALG-27** The halves (the first rounded up) are tested at the same time, the second on top of the first.
- **ALG-28** When the first half passes, the second holds the failure: with `bisection_skip_redundant_tests` it is
  split again without its own run. A half of one PR is still tested, because an inferred failure never evicts.

## Pending failure and depth

Code: `crates/mq-core/src/plan/rules/runs.rs` (`pending_failure`, `successors_testing`)

- **ALG-29** A group whose run fails waits in `pending_failure` for each predecessor to end. If one failed, was
  cancelled or changed, the group restarts on its new predecessors.
- **ALG-30** When all predecessors merged, the group is the culprit, unless `pending_failure_depth` is `N > 0`: it
  then also waits for the runs (not the end) of its first `N` successors that contain it, so that two groups
  never wait for each other. The successors are evaluated again at each pass.

## Failure budget

Code: `crates/mq-core/src/plan/rules/runs.rs` (`pending_failure`)

- **ALG-31** One failure never names a culprit: a group of one PR in culprit position restarts its run
  `failure_budget.retries` times, then fails.
- **ALG-32** A conflict or a configuration error at assembly fails at once, because it is never transient.

## Optimistic merge

Code: `crates/mq-core/src/plan/rules/runs.rs` (`optimistic`)

- **ALG-33** With `optimistic_merging`, a run that passes proves the groups below it: those in `testing` become
  `tests_passed` (`passed_by_optimization`), those in `pending_failure` too (`successor_test_run_passed`). A group
  in bisection is not proved this way.
- **ALG-34** When a predecessor is evicted, the groups behind it always restart: their tests contained code that
  will not land.

## Direct merge

Code: `crates/mq-core/src/plan/rules/formation.rs` (`direct_merge`), `crates/mergequeue/src/merge.rs`
(`direct_merge_possible`)

- **ALG-35** With `direct_merge_mode: always`, the first `pending` item of a `running` queue with no live group gets
  one merge attempt without a test branch; no group forms for it until the attempt answers.
- **ALG-36** The job checks that the PR contains the head of the target, is mergeable and has green required
  statuses; else the item takes the test path, without a failure. Only `fast_forward` refuses a target that moved
  since the check.

## External changes

Code: `crates/mq-core/src/apply.rs` (`target_pushed`), `crates/mq-core/src/plan/rules/readiness.rs` (`validity`),
`crates/mq-core/src/plan/rules/intake.rs` (`pr_changes`)

- **ALG-37** A push on the target is the queue's own when the App sent it or its `after` is the `merged_sha` of a
  queue merge of a `tests_passed` item or of the direct merge candidate, and its `before` is unknown or is the
  known head: another `before` means that commits came in between. Any other push replaces the base of every run
  (`manual_push_to_target_branch`). `specs/tla/TargetHead.tla` checks these rules.
- **ALG-46** A late push changes nothing, and the tick reads the real head: its `after` is a former head, or its
  `before` is a former head and its `after` a merge of the queue.
- **ALG-47** A late push from outside (a former head as `before`, an `after` that the queue never saw) invalidates
  the tested bases but keeps the head, since a later read of the real head would then be ignored as a past head.
- **ALG-38** A PR merged outside the queue becomes `merged` (`manual_merge`); its dependents restart.
- **ALG-39** A push on the PR head, a new base, a draft, a close or the removal of the entry label cancels the item.

## Merge step

Code: `crates/mergequeue/src/merge.rs` (`MERGE_PR`, `land`), `crates/mq-core/src/plan/rules/formation.rs`
(`merge_ready`), `crates/mq-core/src/snapshot.rs` (`expected_head`)

- **ALG-40** Groups merge in graph order. `squash`, `merge_commit` and `rebase` use the `sha` guard at the head of
  admission; `fast_forward` (`sequential` only) moves the target to the tested commit, which is what the CI saw.
- **ALG-41** If a member of a batch fails to merge, the members behind it and all downstream groups restart.
- **ALG-42** The job records the merge commit as `merged_sha`, which ALG-37 uses.
- **ALG-48** The merge of a run must land on its expected head: the base of its group, then the merge of each
  predecessor and of each member ahead, each recorded on the head before it (`merge_parents`). A merge recorded
  on another head means that a commit came in between that the run did not test: the group restarts with
  `manual_push_to_target_branch`.
- **ALG-49** The job reads the real head right before the merge and merges nothing on another one
  (`target_moved`). The snapshot then forgets the target (head, past heads, valid bases): every group restarts,
  and the next read of the head counts, even of a head that it saw before. So a wrong view of the head costs a
  run, never an untested merge (`specs/tla/MergeGuard.tla`). The SCM cannot close the window between that read
  and the merge for `squash`, `merge_commit` and `rebase`.

## Reads and timeouts

Code: `crates/mq-core/src/plan/reads.rs` (`READ_POINTS`, `CHECK_READ_EVERY`, `TARGET_READ_EVERY`, `wake_at`)

- **ALG-43** A live PR is read at points from its submission, then every 15 minutes, or every minute while the SCM
  has not computed its mergeability. The points depend on the age of the item, because a read that changes
  nothing is not in the log.
- **ALG-44** The checks of a run without a report are read every 10 minutes, the head of the target every 5 minutes
  while the queue has work. A lost webhook delays the queue; it never blocks it.
- **ALG-45** `not_ready_timeout_hours` cancels an item that never became ready; `testing_timeout_minutes` ends a run.

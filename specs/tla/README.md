# TLA+ models of the queue

Small models of the design of the queue, which TLC checks by an exhaustive enumeration of a bounded universe
(a few PRs or merges). A model checks a principle of `AGENTS.md`, not the Rust code: it does not prove that the
code implements it. `proptest` (`crates/mq-core/tests/property.rs`) samples the real code; a model enumerates
every order of the facts and the decisions of a small design, the asynchronous ones included: a webhook late,
out of order or lost, a job between two transactions, a reconciliation that reads twice.

`mise run tla:check` runs every row of `cases.tsv` and compares the verdict of TLC with the expected one. The
logs and the configuration of each case go to `test-reports/tla/`. The tools are the `java` of the `tla:check`
task and `tla2tools.jar`, pinned by version and checksum in `scripts/tla-check.sh` and downloaded into
`target/tla/`.

## Cases

A row of `cases.tsv` is `name`, `module`, `config` (the base `.cfg` file), `overrides` (`-`, or the constants
that the case sets, `Name=value` separated by `;`), `expected`, `issue` (the Linear issue of a `bug-` case) and
`impact` (`safety` when the bug can land untested code, `efficiency` when it costs runs); both are `-` for any
other case:

- `pass`: no invariant is violated and TLC enumerated every state.
- An invariant name: the run must violate this invariant, and only this one. The script replaces the
  `INVARIANTS` of the base file, which ends it, with this invariant.

Five kinds of rows, named by their prefix:

- **Fix** (no prefix, expected `pass`): the rules of the code.
- **Mutation** (`<model>-<fault>`): one rule broken as it was before a fix (`Fault`), expected the invariant that
  the fix protects. It proves that the invariant can fail, and keeps the reason of the rule next to it.
- **Bug** (`bug-`): a violation that the rules of the code still have. The row expects the violation, so that the
  matrix stays green; the fix of the bug turns the row into a fix row, expected `pass`.
- **Limit** (`limit-`): a violation that the design accepts, with its reason here.
- **Witness** (`witness-`): an invariant that says a case never happens, expected violated. It proves that the
  model reaches the case; a witness that holds means that the checks of the fix are empty.

The script prints `PASS` for a fix, a mutation caught or a witness reached, `BUG` for a known bug still there,
with its impact and its issue, and `LIMIT` for a limit. A green run therefore does not mean that the code holds
every invariant: read the `BUG` lines, a `safety` one first. A `bug-` case whose invariant now holds fails, so
that a fix turns its case into a fix case. The CI job `TLA+` runs `mise run tla:check`, and `Build Success`
needs it.

## `Queue.tla`: the commit that lands is the one tested (principle IV)

One queue in simple mode: speculative runs on top of the items ahead, the merge job, the answer of the SCM at any
later time, and `urgent`. A merge job already sent is not taken back. The rules come from
`graph::insertion_index`, from `apply` of `MergeSucceeded` and from the `plan` rules of
`crates/mq-core/src/plan/rules/`.

| Row                           | Fault or case                                                                      |
| ----------------------------- | ---------------------------------------------------------------------------------- |
| `queue-urgent-first`          | `urgent` in front of a `tests_passed` item, the rule before `4106481`               |
| `witness-urgent-during-merge` | an urgent item arrives while a merge is in flight                                  |
| `witness-all-merged`          | every PR merges                                                                    |

With `urgent_first`, TLC finds this trace: `a` passes and its merge job goes; urgent `b` enters in front of it and
is tested on the target alone; `b` passes and its merge job goes; `a` lands, and since `a` is no longer
`tests_passed` its push is not the queue's own, so `b` goes back to `pending`; but the merge job of `b` is already
sent, and `b` lands on top of `a`, which its run did not contain.

Not in `Queue.tla`: batches, bisection, the failure budget, the speculation window, direct merge, `not_ready` and
`cancelled`, outside pushes, the inbox between a fact and its decision.

## `TargetHead.tla`: the head of the target and the valid bases (principles III, IV, V)

What the snapshot knows of the head of the target, and the bases on which runs stay valid. The merges of the
queue, a push from outside and the reads of the repair loop change the real branch; their webhooks arrive late,
in any order, or never; the merge job and the repair job give their facts through `queue_inbox`; a
reconciliation reads the facts, then each delivery, at different instants. The rules come from `target_pushed`
(`apply.rs`), the `Pushed` arm of the scheduler, `merge_in_flight` and `read_target` (`repair.rs`) and
`merge.rs`. Not modeled: the groups (any valid base may hold one), a webhook delivered twice, the limit of
`RECENT_HEADS`, the retain of the bases that no group uses.

Since the head check of the merge job (`MergeGuard.tla`), a wrong view of the head costs runs, never an untested
merge: the invariants of this model measure the view, and its open bugs are bugs of efficiency.

- `ViewOfTheHeadIsRight`: once the queue knows a head at or past the outside commit, no base before it stays valid.
- `NoSpuriousRestart`: the push of a merge of the queue never removes a base that is still right. A spurious
  restart costs every run on that base, and a merge then reads as a merge from outside.

| Row                                              | Fault or case                                                       |
| ------------------------------------------------ | ------------------------------------------------------------------- |
| `target-head-inflight-jobs-only`                 | `merge_in_flight` counts only the jobs, the rule before `f05d5e4`    |
| `target-head-end-before-fact`                    | the merge job ends before it gives its fact                         |
| `target-head-own-without-before`                 | a known merge is own whatever its `before`, the rule before `d95bd82` |
| `bug-target-head-read-before-merge-fact`         | see "Open bugs", 1                                                  |
| `bug-target-head-read-of-own-head`               | see "Open bugs", 2                                                  |
| `bug-target-head-reversed-merge-webhooks`        | see "Open bugs", 3                                                  |
| `bug-target-head-head-moves-back`                | see "Open bugs", 4                                                  |
| `witness-target-head-outside-seen`               | the queue learns the outside push                                   |
| `witness-target-head-all-merged`                 | every merge lands                                                   |

The faults `late_push_ignored` (before `d95bd82`) and `late_own_restarts` (before `6d1af32`) have no row: their
cases need two merges, where the open bugs 3 and 4 already violate both invariants. They get a row when those
bugs are fixed.

### Open bugs

Each trace was replayed on the real `apply` of `cihub-mq-core`; each issue holds its trace and its probe test.

1. **A read of the head before the fact of a merge restarts the runs** (CIH-139). `read_target` checks that no
   merge is in flight, then reads the head. A merge can land in between: the read gives `TargetPushed { before:
   None, after: m1 }` before `merge.succeeded` of `m1`, `m1` is not yet own, and every base is removed. With
   `ReadChecksAgain`, the job checks `merge_in_flight` again after its read and gives nothing while a merge is
   in flight; TLC finds no spurious restart then (`MaxReads=1;ReadChecksAgain=TRUE`, `NoSpuriousRestart`).
2. **A read of an own head keeps a base that an outside commit made wrong** (CIH-138). A read has no `before`: when its
   `after` is a known merge, `target_pushed` counts it as own and keeps the old bases, even when an outside
   commit landed before the merge. When the webhook of that commit is lost, the repair loop does not catch it;
   the head check of the next merge does, and that run restarts.
3. **The webhooks of two merges in reverse order restart the runs** (CIH-140). `m1` then `m2` land; the webhook of `m2`
   (`before: m1`) arrives first. Its `before` is not the known head, so the push is not own, and every base is
   removed, though the commits in between are the queue's own.
4. **A late webhook of a merge moves the head back** (CIH-137). `m1`, `m2`, then an outside `x` land; the webhook of `x`
   arrives, then the one of `m2` (`before: m1`, never a head). The push is not late and not own: the head goes
   back from `x` to `m2`, and the only valid base is `m2`, which `x` made wrong. A read of the real head `x` is
   then ignored, since `x` is a past head. The next merge, tested on `m2`, finds `x`: the job refuses it
   (`target_moved`), the queue forgets the target and takes the read of `x`, and the run restarts on `x`
   (`a_target_that_moved_at_merge_time_restarts_the_groups_on_the_read_head`).

## `MergeGuard.tla`: a merge lands only on what its run tested (principle IV)

A run tests its group on a base and the PRs of its predecessors; its merge must land on that base followed by the
merges of those predecessors, and on nothing else. Two checks keep it: the chain (`expected_head`: each
predecessor recorded on the head before it, else the group restarts) and the head (the job reads the real head
and merges only on the expected one, else `target_moved`). What the queue believes of the head is any commit of
the branch, a past one included, so the model holds whatever `target_pushed` decides, its bugs included. Not
modeled: the window between the read of the head and the merge, which the SCM gives no way to close.

| Row                                        | Fault or case                                      |
| ------------------------------------------ | -------------------------------------------------- |
| `merge-guard-no-chain-check`               | plan does not check the chain                      |
| `merge-guard-no-head-check`                | the job merges whatever the real head              |
| `witness-merge-guard-all-merged`           | every PR merges                                    |
| `witness-merge-guard-merged-after-outside` | a merge is refused, then the PR merges on the push |

## Pins: the model follows the code

A model copies rules of the code, and nothing makes the code follow the model: a change of `target_pushed` leaves
`TargetHead.tla` checking the old rule. So each model pins the code that it copies, in `\* pin` lines: a Rust
function (`fn`), or a region between `// tla: begin <name>` and `// tla: end <name>`, with the hash of its text.

`scripts/tla-pins.sh check`, run by `mise run lint` (no Java), fails when a pinned code is gone or its text
changed, when a model has no pin, and when a region has no pin. On such a failure:

1. Read the model against the new code. When the rule changed, change the model, its invariants or its cases.
2. Run `mise run tla:check`.
3. Run `mise run tla:pin`, which writes the new hashes. The change of the hash in the diff tells the review that
   the model was read again.

A change of a comment or of a format changes the hash too: the review is then short, but it happens.

## Rules

- A model uses the vocabulary of `cihub-domain`: its states are `ItemState` values, never a new one.
- The facts (CI, SCM) can happen at any time; the decisions of `plan` happen only when their guard holds.
- A model whose actions can all stop ends with an explicit stuttering step, so that TLC still reports any other
  deadlock. In `TargetHead.tla` a reconciliation can always run, so deadlock detection proves nothing there.
- A change of a rule that a model covers changes the model in the same change; a fixed race adds a mutation, and
  a fixed bug turns its `bug-` row into a fix row. The pins enforce the first part.
- A new model pins the code that it copies; a new rule of the code that a model must cover gets a pin.
- A trace that a model finds is replayed on the real code before it becomes a `bug-` row.

# Configuration

The model of the file is in `crates/config/src/file.rs`; the settings, their default values, their ranges
and the strategy table are in `crates/config/src/settings.rs`. This file explains them.

The behavior of a queue is configured in one file of the repository, `.cihub.yaml`, `.cihub.yml`,
`.cihub/config.yaml` or `.cihub/config.yml`, read on
the head of the target branch (so that a PR cannot loosen its own gate) by the `load_config` job, at
each new head; the settings apply from the reconciliation that follows the
read. A branch has at most one queue, and the branch names it: two queues on one branch would each merge
without testing on top of the other, and each merge of one would restart the runs of the other
(`specs/LESSONS.md`). The `branch` of an entry is a branch, or a glob of branches (`release/*`, with the
syntax of `*=`: `*` stays in one `/` segment, `**` crosses them); each branch that a glob matches has its own
queue. A branch takes the entry that names it, else the first glob, in file order, that matches it. No queue
is created by hand: the first `/cihub merge` on a branch without a queue creates its queue when an entry of
the file serves that branch, or when it is the default branch of the repository, with the default settings
and `defaults`. Without a file, a team thus starts with one queue on its default branch; an entry
`branch: main` then sets that same queue. A queue without an entry has `defaults`. A queue that the file no
longer serves, other than the one of the default branch, takes no new submission and finishes its items.
This file is the only
source of the settings: the dashboard helps write it through a form that opens a PR on
the repository, never a direct commit on the target. If several of these files exist, the configuration is in error, because none
wins. A PR enters the queue of its base branch, and waits in `not_ready` there until its `queue_conditions`
(its checks, its reviews) are satisfied: those of `defaults`, then those of its entry. The first rule of
`priority_rules` that the PR satisfies gives its priority, unless the command gives one (`--priority`): an
urgent PR goes ahead of the others in the same train, rather than in a second queue. The first profile of the
queue whose `conditions` the PR satisfies gives its settings, unless the command names one (`--profile`; see
below).

```yaml
version: 1
queues:
  - branch: main
    strategy: batched                     # sequential | pipelined | batched | parallel
    merge_method: squash                  # squash | merge_commit | rebase | fast_forward
    trigger: draft_pr                     # draft_pr | push
    queue_conditions:                     # admission: the PR itself
      - review-decision = approved
      - -draft
      - -label = do-not-merge
      - check-success = lint
    merge_conditions:                     # on the speculative commit
      - check-success = build
      - check-success = integration-tests
    window: { floor: 2, ceiling: 20 }
    batching: { size: 8, max_wait_minutes: 5 }
    failure_budget: { retries: 1 }
    pending_failure_depth: 1
    optimistic_merging: true
    profiles:                             # settings of a kind of PR, in the same train
      - name: dependencies
        conditions:                       # choose the profile, once, at the submission
          - author = dependabot[bot]
        merge_method: merge_commit
        batching: { size: 8, max_wait_minutes: 15 }
        failure_budget: { retries: 2 }
  - branch: "release/*"                   # a queue for each branch that the glob matches
    strategy: sequential
    merge_method: fast_forward
    queue_conditions:
      - sender-permission >= write
priority_rules:
  - name: urgent
    conditions:
      - label = urgent
    priority: urgent                      # urgent | high | medium | low | 0-255
  - name: dependabot
    conditions:
      - author = dependabot[bot]          # an inline list breaks the YAML on the brackets
    priority: low
defaults:                                 # any queue setting; each queue can override it
  label_commands: { enqueue_label: ready-to-merge }
  merge_commit_title: pr_title            # pr_title | pr_title_and_number
  queue_conditions:                       # every queue adds its own after them
    - -label = do-not-merge
```

## Errors

The file must start with `version: 1`: another value, or its absence, gives
`start_test_run_config_bad_version`. An unknown key, an out-of-range value or two settings that do not go
together give `start_test_run_config_parsing_failure`, with the line and column of the faulty value. A
`required_statuses` list with an empty or duplicate name gives `start_test_run_config_bad_required_statuses`.
For now, `merge_conditions` accepts only `check-success = <name>` at the first level:
another form gives `start_test_run_config_parsing_failure`, with its position. `defaults` accepts every
queue setting and the conditions, but not `branch` or `profiles`; two entries cannot have the same `branch`,
and a `branch` that is not a valid glob is refused. In an entry, two profiles cannot have the same `name`, and a
profile cannot set `merge_method: fast_forward`, which moves the target of the whole train.

## Profiles

A profile gives settings to a kind of PR in the train of its queue: Dependabot updates merged with a merge
commit and in large batches, while the PRs of people are squashed. It is not a second queue: its PRs are tested
on top of the others, in the same order (`specs/LESSONS.md`). A profile sets only the settings of a PR:
`merge_method` (but `fast_forward`), `merge_commit_title`, `batching` (`enabled`, `size`, `max_wait_minutes`) and
`failure_budget`; a setting that it does not set is the one of its queue. The settings of the train (`strategy`,
`mode`, `window`, `trigger`, the timeouts, the prefixes) stay on the queue.

The submission chooses the profile once, as the routing did before: the profile that the command names
(`/cihub merge --profile=<name>`, `--profile` of the CLI, `profile` of the API, refused when the queue does not
have it), else the first profile, in file order, whose `conditions` the PR satisfies; a profile without
`conditions` is chosen only by its name. The item keeps its profile until it ends; a resubmission on a push
keeps it too. `conditions` choose and `queue_conditions` admit: write in `conditions` what says what the PR is
and does not change (its author, its head branch, its files), as a PR that does not satisfy a condition yet at
the submission would keep the wrong profile. A batch holds the PRs of one profile, as it merges with one set of
settings, and a culprit spends the failure budget of its profile. A profile that the file no longer has leaves
its items the settings of their queue. The comment of CIHub on the PR, the `profile` of the API `Pr` and the
dashboard name the profile of a PR. The submission reads the files and the review threads of the PR once, for
its `queue_conditions`, its priority and its profile together.

## Sources and strategies

Each setting takes the value of the strongest source, in the order `default` < `strategy` < `file`
(`defaults`, then the queue). `cihub-config` returns each value with its source, which the interface shows to
flag a deviation from the strategy. The history of the settings is that of the file in the repository. An invalid
file fails the run that reads it at `base_sha` (errors above); for the other settings, the queue
keeps the last valid file and shows the error. The state of the queue (`running`, `paused`, `draining`) is not
a setting: only the dashboard, the API and the CLI change it.

The first setting of a queue is `strategy` (`pipelined` by default), which sets the values of the ones that
follow. A strategy is a table of default values, with no code in the core:

| `strategy`   | `mode`     | Window (floor / ceiling)     | Batches            | `pending_failure_depth` | `optimistic_merging` |
| ------------ | ---------- | ---------------------------- | ------------------ | ----------------------- | -------------------- |
| `sequential` | `single`   | 1 / 1                        | no                 | 0                       | no                   |
| `pipelined`  | `single`   | 2 / 20                       | no                 | 0                       | no                   |
| `batched`    | `single`   | 2 / 20                       | yes, at most 4     | 1                       | yes                  |
| `parallel`   | `parallel` | 2 / 20                       | yes, at most 4     | 1                       | yes                  |

All strategies enable direct merge (`direct_merge_mode: always`) and give one
failure budget retry.

## Settings

| Setting                                                        | Default                             | Range                                                                      |
| -------------------------------------------------------------- | ----------------------------------- | -------------------------------------------------------------------------- |
| `mode`                                                         | `single`                            | `single`, `parallel`                                                       |
| `window.floor` / `window.ceiling`                              | 2 / 20                              | 1–100, floor ≤ ceiling                                                     |
| `testing_concurrency`                                          | none (shortcut)                     | 1–100; sets floor and ceiling to the same value; exclusive with `window`   |
| `bisection_concurrency`                                        | the window ceiling                  | 1–200                                                                      |
| `testing_timeout_minutes`                                      | 300                                 | 10–1440                                                                    |
| `not_ready_timeout_hours`                                      | 168                                 | 1–720                                                                      |
| `merge_method`                                                 | `squash`                            | `squash`, `merge_commit`, `rebase`, `fast_forward` (`sequential` strategy only) |
| `merge_commit_title`                                           | `pr_title`                          | `pr_title`, `pr_title_and_number`                                          |
| `trigger`                                                      | `draft_pr`                          | `draft_pr`, `push`; `inplace` arrives with a later lot and is refused until then |
| `update_method`                                                | `merge`                             | `merge`; used by `inplace` (`rebase` arrives with a later lot) |
| `batching.enabled` / `size` / `max_wait_minutes`               | disabled / 4 / 5                    | maximum size 2–20, wait 1–60                                               |
| `failure_budget.retries`                                       | 1                                   | 0–10                                                                       |
| `optimistic_merging`                                           | disabled                            | boolean                                                                    |
| `pending_failure_depth`                                        | 0                                   | 0–10                                                                       |
| `direct_merge_mode`                                            | `always`                            | `off`, `always`                                                            |
| `optimization_mode`                                            | `bisection_skip_redundant_tests`    | `off`, `bisection_skip_redundant_tests`                                    |
| `comments_enabled`, `commands_enabled`, `status_check_enabled` | enabled                             | boolean                                                                    |
| `label_commands.enabled` / `enqueue_label`                     | enabled / `cihub-merge-queue-submit` | non-empty string                                                           |
| `state_labels_enabled`                                         | disabled                            | boolean                                                                    |
| `resubmit_on_push`                                             | disabled                            | boolean; a push on a queued PR resubmits it at the end of the queue |
| `keep_test_branches_minutes`                                   | 0                                   | 0–1440                                                                     |
| `branch_prefix`                                                | `cihub`                             | string without `/` or space; branches `<prefix>-merge/` and `<prefix>-temp/` |
| `branch_protection_injection`                                  | `both`                              | `both`, `merge`, `none`                                                    |
| `required_statuses`                                            | inherited from branch protection    | list; empty list = none, `null` = inherited                                |

## Per-PR options and several branches

Per-PR options are set at submission and stored on the item: priority, `no_batch`, `profile`, `direct`
(admins, requires the sudo App). Named priorities are shortcuts for values (`urgent` 0, `high` 10,
`medium` 100, `low` 200). A repository has one queue per target branch (`main`, `release/2.0`),
each with its own settings; a PR enters the queue of its base branch, and a base branch change cancels
the item with `pr_base_branch_changed`.

A queue cannot be deleted while it contains live items; its deletion erases its settings
and its history from Postgres but not from ClickHouse. Changing `mode` with items in flight switches to
`switching_modes`, refuses new submissions with `queue_switching_modes`, and ends automatically
when the last item concludes.

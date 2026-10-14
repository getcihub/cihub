# Configuration

The model of the file is in `crates/config/src/file.rs`; the settings, their default values, their ranges
and the strategy table are in `crates/config/src/settings.rs`. This file explains them.

The behavior of a queue is configured in one file of the repository, `.cihub.yaml`, `.cihub.yml`,
`.cihub/config.yaml` or `.cihub/config.yml`, read on
the head of the target branch (so that a PR cannot loosen its own gate) by the `load_config` job, at
each new head; the settings apply from the reconciliation that follows the
read. No queue is created by hand: the first `/cihub merge` on a branch without a queue
creates the queues that the file names on that branch, or, without such an entry, on the default branch of the
repository, a queue named after the branch, with the default settings and `defaults`. Without a file, a team thus
starts with one queue on its default branch; an entry `name: main`, `branch: main` then sets that same queue. An
entry of `queues` links to its queue by `branch` and `name`; a queue without an entry has `defaults`. A queue
that the file no longer names takes no new submission and finishes its items. This file is the only
source of the settings: the dashboard helps write it through a form that opens a PR on
the repository, never a direct commit on the target. If several of these files exist, the configuration is in error, because none
wins. Several queues can target the same branch: a PR is routed to the queue named in the command
(`/cihub merge --queue=hotfix`), otherwise to the first queue, in file order, whose `route_conditions` are
satisfied (a queue without `route_conditions` takes every PR), otherwise to the first queue of the branch. The routing reads the PR once, at the submission, and an item never changes queue.
`queue_conditions` do not route: they only admit, and the item waits in `not_ready` in its queue until they are
satisfied. Write in `route_conditions` what says where a PR goes and does not change (its head branch, for example),
and in `queue_conditions` what says whether it is ready (its checks, its reviews). The first rule of `priority_rules` that the PR satisfies gives its priority, unless the
command gives one (`--priority`).

```yaml
version: 1
queues:
  - name: main
    branch: main
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
  - name: hotfix
    branch: main
    strategy: sequential
    merge_method: fast_forward
    route_conditions:                     # routing: the queue that the PR enters
      - label = hotfix
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
```

## Errors

The file must start with `version: 1`: another value, or its absence, gives
`start_test_run_config_bad_version`. An unknown key, an out-of-range value or two settings that do not go
together give `start_test_run_config_parsing_failure`, with the line and column of the faulty value. A
`required_statuses` list with an empty or duplicate name gives `start_test_run_config_bad_required_statuses`.
For now, `merge_conditions` accepts only `check-success = <name>` at the first level:
another form gives `start_test_run_config_parsing_failure`, with its position. `defaults` accepts every
queue setting, but not `name`, `branch` or the conditions (`route_conditions`, `queue_conditions`,
`merge_conditions`); two queues cannot have the same `name`.

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
| `comments_enabled`, `commands_enabled`, `status_check_enabled` | enabled                             | boolean; `status_check_enabled` turns off both statuses, the queue one and the conditions one |
| `label_commands.enabled` / `enqueue_label`                     | enabled / `cihub-merge-queue-submit` | non-empty string                                                           |
| `state_labels_enabled`                                         | disabled                            | boolean                                                                    |
| `resubmit_on_push`                                             | disabled                            | boolean; a push on a queued PR resubmits it at the end of the queue |
| `keep_test_branches_minutes`                                   | 0                                   | 0–1440                                                                     |
| `branch_prefix`                                                | `cihub`                             | string without `/` or space; branches `<prefix>-merge/` and `<prefix>-temp/` |
| `branch_protection_injection`                                  | `both`                              | `both`, `merge`, `none`                                                    |
| `required_statuses`                                            | inherited from branch protection    | list; empty list = none, `null` = inherited                                |

## Per-PR options and multiple queues

Per-PR options are set at submission and stored on the item: priority, `no_batch`, `direct` (admins,
requires the sudo App). Named priorities are shortcuts for values (`urgent` 0, `high` 10,
`medium` 100, `low` 200). A repository can have several queues (`main`,
`release/*` branches), each with its own settings; a PR is routed by its base branch, and a
base branch change cancels the item with `pr_base_branch_changed`.

A queue cannot be deleted while it contains live items; its deletion erases its settings
and its history from Postgres but not from ClickHouse. Changing `mode` with items in flight switches to
`switching_modes`, refuses new submissions with `queue_switching_modes`, and ends automatically
when the last item concludes.

# Configuration

The model of the file is in `crates/config/src/file.rs`; the settings, their default values, their ranges
and the strategy table are in `crates/config/src/settings.rs`. This file explains them.

The behavior of a queue is configured in a `.cihub.yaml` or `.cihub.yml` file at the root of the repository, read on
the head of the target branch (so that a PR cannot loosen its own gate) by the `load_config` job, at
each new head (`DECISIONS.md` D136); the settings apply from the reconciliation that follows the
read. An entry of `queues` links to its queue by `branch` and `name`; a queue without an entry has the default
settings, without `defaults` (D138). This file is the only
source of the settings (`DECISIONS.md` D134): the dashboard helps write it through a form that opens a PR on
the repository, never a direct commit on the target. If both file names exist, the configuration is in error, because neither
one wins. Several queues can target the same branch: a PR is routed to the first queue, in
file order, whose `queue_conditions` are satisfied, or to the one named in the command
(`/cihub merge --queue=hotfix`), otherwise to the first queue of the branch; an item never changes queue
(`DECISIONS.md` D148). The first rule of `priority_rules` that the PR satisfies gives its priority, unless the
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
    queue_conditions:
      - label = hotfix
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
For now, `merge_conditions` accepts only `check-success = <name>` at the first level (`DECISIONS.md` D146):
another form gives `start_test_run_config_parsing_failure`, with its position. `defaults` accepts every
queue setting, but not `name`, `branch` or the conditions; two queues cannot have the same `name`.

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
| `batched`    | `single`   | 2 / 20                       | yes, at most 8     | 1                       | yes                  |
| `parallel`   | `parallel` | 2 / 20                       | yes, at most 4     | 1                       | yes                  |

All strategies keep direct merge disabled (`direct_merge_mode: off`, `DECISIONS.md` D153) and give one
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
| `merge_method`                                                 | `squash`                            | `squash`, `merge_commit`, `rebase`, `fast_forward` (`sequential` strategy only, `DECISIONS.md` D120) |
| `merge_commit_title`                                           | `pr_title`                          | `pr_title`, `pr_title_and_number`                                          |
| `trigger`                                                      | `draft_pr`                          | `draft_pr`, `push`; `inplace` arrives with a later lot and is refused until then (`DECISIONS.md` D119) |
| `update_method`                                                | `merge`                             | `merge`; used by `inplace` (`rebase` arrives with a later lot, `DECISIONS.md` D114) |
| `batching.enabled` / `size` / `max_wait_minutes`               | disabled / 4 / 5                    | maximum size 2–20, wait 1–60                                               |
| `failure_budget.retries`                                       | 1                                   | 0–10                                                                       |
| `optimistic_merging`                                           | disabled                            | boolean                                                                    |
| `pending_failure_depth`                                        | 0                                   | 0–10                                                                       |
| `direct_merge_mode`                                            | `off`                               | `off`, `always`                                                            |
| `optimization_mode`                                            | `bisection_skip_redundant_tests`    | `off`, `bisection_skip_redundant_tests`                                    |
| `comments_enabled`, `commands_enabled`, `status_check_enabled` | enabled                             | boolean                                                                    |
| `label_commands.enabled` / `enqueue_label`                     | enabled / `cihub-merge-queue-submit` | non-empty string                                                           |
| `state_labels_enabled`                                         | disabled                            | boolean                                                                    |
| `resubmit_on_push`                                             | disabled                            | boolean; a push on a queued PR resubmits it at the end of the queue (`DECISIONS.md` D152) |
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

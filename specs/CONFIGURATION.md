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
syntax of `matches`: `*` stays in one `/` segment, `**` crosses them); each branch that a glob matches has its own
queue. A branch takes the entry that names it, else the first glob, in file order, that matches it. No queue
is created by hand: the first `/cihub merge` on a branch without a queue creates its queue when an entry of
the file serves that branch, or when it is the default branch of the repository, with the default settings
and `defaults`. Without a file, a team thus starts with one queue on its default branch; an entry
`branch: main` then sets that same queue. A queue without an entry has `defaults`. A queue that the file no
longer serves, other than the one of the default branch, takes no new submission and finishes its items.
This file is the only
source of the settings: the dashboard helps write it through a form that opens a PR on
the repository, never a direct commit on the target. If several of these files exist, the configuration is in error, because none
wins. The anchors (`&`, `*`) and the merge keys (`<<:`) of YAML are read, so that a part of the file is written
once. A PR enters the queue of its base branch, and waits in `not_ready` there until its `queue_conditions`
(its checks, its reviews) are satisfied: those of `defaults`, then those of its entry. The first rule of
`priority_rules` that the PR satisfies gives its priority, unless the command gives one (`--priority`): an
urgent PR goes ahead of the others in the same train, rather than in a second queue. The first profile of the
queue whose `conditions` the PR satisfies gives its settings, unless the command names one (`--profile`; see
below).

```yaml
# yaml-language-server: $schema=https://cihub.io/schema/config/v1.json
version: 1
queues:
  - branch: main
    strategy: batched                     # sequential | pipelined | batched | parallel
    merge_method: squash                  # squash | merge_commit | rebase | fast_forward
    trigger: draft_pr                     # draft_pr | push
    queue_conditions:                     # admission: the PR itself
      review_decision: approved
      draft: false
      labels:
        none:
          - do-not-merge
      checks:
        success:
          - lint
    merge_conditions:                     # on the speculative commit
      checks:
        success:
          - build
          - integration-tests
    window:
      floor: 2
      ceiling: 20
    batching:
      size: 8
      max_wait_minutes: 5
    failure_budget:
      retries: 1
    pending_failure_depth: 1
    optimistic_merging: true
    profiles:                             # settings of a kind of PR, in the same train
      - name: dependencies
        conditions:                       # choose the profile, once, at the submission
          author:
            - dependabot[bot]
            - cihub-renovate[bot]
        merge_method: merge_commit
        batching:
          size: 8
          max_wait_minutes: 15
        failure_budget:
          retries: 2
  - branch: release/*                     # a queue for each branch that the glob matches
    strategy: sequential
    merge_method: fast_forward
    queue_conditions:
      sender_permission:
        min: write
priority_rules:
  - name: urgent
    conditions:
      any:
        - labels:
            any:
              - urgent
              - hotfix
        - head:
            matches: hotfix/**
    priority: urgent                      # urgent | high | medium | low | 0-255
  - name: dependabot
    conditions:
      author: dependabot[bot]
    priority: low
defaults:                                 # any queue setting; each queue can override it
  label_commands:
    enqueue_label: ready-to-merge
  merge_commit_title: pr_title            # pr_title | pr_title_and_number
  queue_conditions:                       # every queue adds its own after them
    labels:
      none:
        - do-not-merge
```

## Errors

The file must start with `version: 1`: another value, or its absence, gives
`start_test_run_config_bad_version`. An unknown key, an out-of-range value or two settings that do not go
together give `start_test_run_config_parsing_failure`, with the line and column of the faulty value. A
`required_statuses` list with an empty or duplicate name gives `start_test_run_config_bad_required_statuses`.
A condition that the section Conditions does not allow gives `start_test_run_config_parsing_failure`, with
its position. Only `true` and `false` are booleans, in every key of the file: `yes`, `no`, `on` and `off` are
strings. `defaults` accepts every
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
| `comments_enabled`, `commands_enabled`, `status_check_enabled` | enabled                             | boolean; `status_check_enabled` turns off both statuses, the queue one and the conditions one |
| `label_commands.enabled` / `enqueue_label`                     | enabled / `cihub-merge-queue-submit` | non-empty string                                                           |
| `state_labels_enabled`                                         | disabled                            | boolean                                                                    |
| `resubmit_on_push`                                             | disabled                            | boolean; a push on a queued PR resubmits it at the end of the queue |
| `keep_test_branches_minutes`                                   | 0                                   | 0–1440                                                                     |
| `branch_prefix`                                                | `cihub`                             | string without `/` or space; branches `<prefix>-merge/` and `<prefix>-temp/` |
| `branch_protection_injection`                                  | `both`                              | `both`, `merge`, `none`                                                    |
| `required_statuses`                                            | inherited from branch protection    | list; empty list = none, `null` = inherited                                |

## Per-PR options and several branches

Per-PR options are set at submission and stored on the item: priority, `no_batch`, `profile`. Named priorities are shortcuts for values
(`urgent` 0, `high` 10, `medium` 100, `low` 200). A repository has one queue per target branch (`main`, `release/2.0`),
each with its own settings; a PR enters the queue of its base branch, and a base branch change cancels
the item with `pr_base_branch_changed`.

A queue cannot be deleted while it contains live items; its deletion erases its settings
and its history from Postgres but not from ClickHouse. Changing `mode` with items in flight switches to
`switching_modes`, refuses new submissions with `queue_switching_modes`, and ends automatically
when the last item concludes.

## Conditions

The conditions of the file are typed YAML maps: `queue_conditions`, `merge_conditions`, the `conditions` of a
profile and of a priority rule (`cihub-conditions` checks and evaluates them, `cihub-config` reads them). In a
map, the key is an attribute and the value is typed; every key of a map must hold. The documentation writes
the block style: one key per line, one list element per line; a value such as `dependabot[bot]` then needs no
quotes, and a value that starts with `*` takes quotes (`"**/*.md"`), as YAML reads it as an alias.

| Type                      | Keys under the attribute                                                                                  | Example                                    |
| ------------------------- | --------------------------------------------------------------------------------------------------------- | ------------------------------------------ |
| Boolean                   | none: the value is `true` or `false`                                                                      | `draft: false`                             |
| String                    | none (one of the values), `not` (none of the values), `matches` (globs), `regex`                          | `head:` then `matches: hotfix/**`          |
| List                      | none (shortcut of `all`), `all`, `any`, `none`, `any_match`, `all_match`, `none_match` (globs), `count`   | `labels:` then `none:` then `- wip`        |
| Integer                   | none (equal), `min`, `max` (inclusive)                                                                    | `additions:` then `max: 500`               |
| Duration                  | `min`, `max` (inclusive); no value, as an exact duration never holds                                      | `pr_age:` then `min: 2h`                   |
| Closed choice             | none (one of the values); an ordered choice also takes `min`, `max`                                       | `review_decision: approved`                |
| Logic, at any level       | `all`, `any`, `none`, each followed by a list of maps: every map, at least one, no map holds             | `any:` then `- labels: …`                  |

A key takes one value or a list of values; `regex`, `min`, `max`, `count` and the value of a boolean or an
integer take one. In one map, a `max` below the `min` is refused. `count` takes an integer, or `min` and `max`.
A key that appears in a list operator (`all`, `any`, `none`) has the same meaning at both levels: on a list
attribute it counts the values that the PR has, at the level of the map it counts the maps that hold. `all_match` is true when every element
matches one of the globs, and false on an empty list. A glob is the glob of `branch`. `regex` is accepted on a
string only and looks for a match anywhere, with the `regex` crate, in linear time, without lookaround or
backreferences. An absent string (`sender` without a sender) behaves like an empty list; a comparison on an
absent value (`queue_age` and `queue_position` outside the queue, `sender_permission` without a sender) is
false. No templates (`{{ author }}`); no alias for a name.

| Attribute                                                                                | Type                                                       | Note                                                                         |
| ---------------------------------------------------------------------------------------- | ---------------------------------------------------------- | ---------------------------------------------------------------------------- |
| `draft`, `conflict`, `from_fork`, `behind_base`                                          | boolean                                                    | `behind_base` is read from `mergeStateStatus = BEHIND`                       |
| `number`, `additions`, `deletions`, `queue_position`                                     | integer                                                    | `queue_position`: 0 at the head of the queue                                 |
| `queue_age`, `pr_age`                                                                    | duration (`30m`, `2h`, `7d`)                               |                                                                              |
| `title`, `body`, `author`, `sender`, `base`, `head`                                      | string                                                     | `sender`: author of the command or of the submission                         |
| `review_decision`                                                                        | `approved`, `changes_requested`, `review_required`, `none` |                                                                              |
| `sender_permission`                                                                      | `none` < `read` < `write` < `admin`                        | Ordered                                                                      |
| `labels`, `files`                                                                        | list                                                       | `files` is read only if a condition uses it                                  |
| `approved_reviews_by`, `changes_requested_reviews_by`, `review_requested`                | list                                                       |                                                                              |
| `review_threads_unresolved`                                                              | list                                                       | Paths of the unresolved threads, read only if a condition uses it           |
| `checks.success`, `checks.neutral`, `checks.skipped`, `checks.failure`, `checks.pending` | list of check names                                        | Under one `checks` map; `failure`: `failure`, `error`, `cancelled`, `timed_out`, `action_required` or `stale`; `pending`: no conclusion yet |
| `schedule`                                                                               | time range                                                 | Deferred: requires a time zone database in the pure core                    |

The checks are read on the head of the PR in `queue_conditions`, and on the test SHA in `merge_conditions`;
names match exactly, without an `@app/` prefix. For now, `merge_conditions` accepts only `checks.success` with
names, which are added to the required statuses of the run. The error of another condition points at the
deepest key that is refused (`checks.pending`, or `any` under `checks.success`).

A value is kept as the file writes it: a check `3.10` stays `3.10`, not the number `3.1`. An unknown
attribute, an unknown key under an attribute, a key that does not fit the type, an invalid value, or an empty
map or block list inside a condition (it would hold, or fail, without a word), or an empty or null
`conditions` of a profile or of a priority rule (it would match every PR; a profile without the key is
valid), gives `start_test_run_config_parsing_failure` with its line and column, and a message that starts
with the path of the condition (`files.count.max: `x` is not an integer from 0 to 4294967295`). A PR that
waits shows each condition that does not hold, in the order of the file, as its path and its values (`checks.success: lint`, `labels.none:
wip`): a key whose values each hold on their own (a list attribute, `all`, `none`) gives one condition per
value, so that only the values that fail are shown. A block shows as its key and its maps, separated by `;`
(`any: [labels.any: urgent, hotfix; head.matches: hotfix/**]`).

## JSON schema

`schema/config/v1.json` describes the file in JSON Schema (draft-07, which the editors read), with a
description for each key: an editor or an agent completes and checks the file with it. A file declares it on
its first line, as the example above; `v1` is the `version: 1` of the file:

```yaml
# yaml-language-server: $schema=https://cihub.io/schema/config/v1.json
```

The schema is written by hand, and `crates/config/tests/schema.rs` keeps it equal to the code: the keys of
each level and the values of each closed setting are those that the parser accepts, the range of each integer
setting is the one that it checks, the attributes, the operators of each attribute and the values of each
closed choice are those of `cihub-conditions`, and every key has a description. A key, a value, a range, an
attribute or an operator that the code adds or removes thus fails a test until the schema follows. The site
(`apps/website/`) publishes the schema of `main` at `https://cihub.io/schema/config/v1.json`, the URL of the
documentation, a copy that its build makes; a file that the dashboard writes will declare the URL of the tag
of its instance on GitHub (`…/cihub/<tag>/schema/config/v1.json`), whose keys are those that this instance
accepts.

**Not yet.** A `cihub config check` command that gives the errors of the server, for a person and for an
agent, and an entry in SchemaStore, so that an editor finds the schema without the first line.

# Configuration

This spec gives the rules of `.cihub.yaml`, the configuration file of a repository. It does not list the keys, the
defaults or the ranges: `crates/config/src/file.rs` (`RepoConfig`) holds the model of the file,
`crates/config/src/settings.rs` (`QueueSettings`, `strategy_layer`) the settings, their defaults, their ranges and
the strategies, and `schema/config/v1.json` describes each key.

```yaml
# yaml-language-server: $schema=https://cihub.io/schema/config/v1.json
version: 1
queues:
  - branch: main
    strategy: batched
    queue_conditions:
      review_decision: approved
      draft: false
    merge_conditions:
      checks:
        success:
          - build
    profiles:
      - name: dependencies
        conditions:
          author: dependabot[bot]
        merge_method: merge_commit
  - branch: release/*
    strategy: sequential
```

## The file

Code: `crates/config/src/file.rs` (`FILE_NAMES`, `find_config_file`), `crates/mergequeue/src/config.rs` (`load_config`)

- **CFG-1** The file is the only source of the settings of a queue. The dashboard helps write it through a form
  that opens a PR on the repository, never through a direct commit on the target.
- **CFG-2** The file has one of the names of `FILE_NAMES`. Two of them in one repository is an error, because
  none wins.
- **CFG-3** `load_config` reads the file on the head of the target branch at each new head, never on the PR, so
  that a PR cannot loosen its own gate. The settings apply from the reconciliation that follows the read.
- **CFG-4** An invalid file fails the run that reads it at its `base_sha`; for the other settings, the queue keeps
  the last valid file and shows the error.
- **CFG-5** The history of the settings is the history of the file in the repository. The state of a queue
  (`running`, `paused`, `draining`) is not a setting: the dashboard, the API and the CLI change it.
- **CFG-6** The parser reads the anchors (`&`, `*`) and the merge keys (`<<:`) of YAML, so that a part of the file
  is written once. Only `true` and `false` are booleans: `yes`, `no`, `on` and `off` are strings. A value is kept
  as the file writes it: a check `3.10` stays `3.10`.

## Queues and branches

- **CFG-7** A branch has at most one queue, and the branch names it: two queues on one branch would each merge
  without a test on top of the other, and each merge of one would restart the runs of the other
  (`specs/LESSONS.md`).
- **CFG-8** The `branch` of an entry is a branch or a glob of branches (`*` stays in one `/` segment, `**` crosses
  them); each branch that a glob matches has its own queue. A branch takes the entry that names it, else the first
  glob, in file order, that matches it.
- **CFG-9** No queue is created by hand. The first `/cihub merge` on a branch creates its queue when an entry
  serves that branch, or when it is the default branch of the repository. Without a file, a team thus starts with
  one queue on its default branch.
- **CFG-10** A queue that the file no longer serves, other than the queue of the default branch, takes no new
  submission and finishes its items.
- **CFG-11** A PR enters the queue of its base branch. A change of its base branch cancels the item with
  `pr_base_branch_changed`.
- **CFG-12** A change of `mode` with items in flight switches the queue to `switching_modes`, which refuses new
  submissions with `queue_switching_modes` and ends when the last item concludes.

## Sources and strategies

Code: `crates/config/src/settings.rs` (`Source`, `QueueSettings::resolve`, `strategy_layer`)

- **CFG-13** Each setting takes the value of the strongest source, in the order `default` < `strategy` < `file`
  (`defaults`, then the entry of the queue). `cihub-config` returns each value with its source, so that the
  interface can show a deviation from the strategy.
- **CFG-14** A strategy is a table of default values, with no code in the core (principle X).
- **CFG-15** `defaults` accepts every queue setting and the conditions, but not `branch` or `profiles`. The
  `queue_conditions` of a queue are those of `defaults`, then those of its entry.
- **CFG-16** `merge_method: fast_forward` needs the `sequential` strategy and is refused in a profile, because a
  fast-forward moves the target of the whole train to the tested commit.
- **CFG-17** Without `required_statuses`, or with `null`, a queue takes the required checks of the rules of its
  target branch. An empty list fails the run with `start_test_run_config_missing_required_statuses`, because a
  run without a required check would pass on nothing.

## Profiles and priorities

Code: `crates/config/src/file.rs` (`ProfileConfig`, `PriorityRule`), `crates/domain/src/item.rs` (`Priority`)

- **CFG-18** A profile gives settings to a kind of PR in the train of its queue (Dependabot updates merged with a
  merge commit, the PRs of people squashed). It is not a second queue: its PRs are tested on top of the others, in
  the same order (`specs/LESSONS.md`).
- **CFG-19** A profile sets only the settings of a PR (`merge_method`, `merge_commit_title`, `batching`,
  `failure_budget`). The settings of the train (`strategy`, `mode`, `window`, `trigger`, the timeouts, the
  prefixes) stay on the queue; a setting that the profile does not set is the one of its queue.
- **CFG-20** The submission chooses the profile once: the profile that the command names (refused when the queue
  does not have it), else the first profile, in file order, whose `conditions` the PR satisfies. A profile without
  `conditions` is chosen only by its name.
- **CFG-21** The item keeps its profile until it ends, and a resubmission on a push keeps it too. Write in
  `conditions` what the PR is and does not change (its author, its head branch, its files), because a condition
  that becomes true after the submission does not change the profile. `queue_conditions` admit; `conditions`
  choose.
- **CFG-22** A batch holds the PRs of one profile, as it merges with one set of settings, and a culprit spends the
  failure budget of its profile. The items of a profile that the file no longer has take the settings of their
  queue.
- **CFG-23** The first rule of `priority_rules` that the PR satisfies gives its priority, unless the command gives
  one. An urgent PR thus goes ahead of the others in the same train, not in a second queue.
- **CFG-24** The submission reads the files and the review threads of the PR once, for its `queue_conditions`,
  its priority and its profile together.

## Conditions

Code: `crates/conditions/src/attribute.rs` (`Attribute`), `crates/conditions/src/operator.rs`,
`crates/conditions/src/condition.rs`

- **CFG-25** A condition is a typed YAML map, not a string language: the key is an attribute, the value is typed,
  and every key of a map must hold. `all`, `any` and `none`, at any level, take a list of maps.
- **CFG-26** A key of a list operator has the same meaning at both levels: on a list attribute it counts the values
  that the PR has; at the level of the map it counts the maps that hold. `all_match` is false on an empty list.
- **CFG-27** `regex` applies to a string only, looks for a match anywhere, and uses the `regex` crate: linear time,
  without lookaround or backreferences.
- **CFG-28** An absent string (`sender` without a sender) behaves as an empty list. A comparison on an absent value
  (`queue_position` outside the queue, `sender_permission` without a sender) is false.
- **CFG-29** No templates (`{{ author }}`) and no alias for a name.
- **CFG-30** An empty map, an empty block list, or an empty or `null` `conditions` of a profile or a priority rule
  is refused, because it would hold, or fail, without a word.
- **CFG-31** `files` and `review_threads_unresolved` cost a read of the SCM each: CIHub reads them only when a
  condition of the queue uses them.
- **CFG-32** The checks are read on the head of the PR in `queue_conditions`, and on the test SHA in
  `merge_conditions`. Names match exactly. `merge_conditions` accepts only `checks.success`, whose names are added
  to the required statuses of the run.
- **CFG-33** A PR that waits shows each condition that does not hold, in file order, as its path and its values
  (`labels.none: wip`). A key whose values each hold on their own gives one condition per value, so that only the
  values that fail show.
- **CFG-34** The documentation writes the block style, one key or element per line. A value that starts with `*`
  takes quotes (`"**/*.md"`), because YAML reads it as an alias.

## Errors

Code: `crates/config/src/error.rs` (`ConfigErrorKind`, `ConfigError`)

- **CFG-35** An error gives the line and the column of the value at fault. An error in a condition gives the path
  of the deepest key that is refused (`files.count.max: ...`).
- **CFG-36** An unknown key is an error, never ignored, because a typo would otherwise change nothing without a
  word.
- **CFG-37** Two entries with the same `branch`, or two profiles of an entry with the same `name`, are an error.

## JSON schema

Code: `schema/config/v1.json`, `crates/config/tests/schema.rs`

- **CFG-38** The schema is written by hand in JSON Schema draft-07, which the editors read.
  `crates/config/tests/schema.rs` keeps its keys, closed values, ranges, attributes and operators equal to the code,
  and checks that every key has a description.
- **CFG-39** A file declares the schema on its first line, as the example above does (a test parses that example);
  `v1` is the `version: 1` of the file.
- **CFG-40** The site (`apps/website/`) publishes the schema of `main` at `https://cihub.io/schema/config/v1.json`.

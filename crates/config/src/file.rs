//! The `.cihub.yaml` model and what the queues read from it.

use std::collections::HashSet;

use cihub_conditions::{Conditions, Expr, Glob};
use cihub_domain::Priority;

use crate::conditions::required_check;
use crate::{ConfigError, Layer, QueueSettings, read};

/// Paths of the configuration file in the repository.
pub const FILE_NAMES: [&str; 4] = [
    ".cihub.yaml",
    ".cihub.yml",
    ".cihub/config.yaml",
    ".cihub/config.yml",
];

/// Chooses the configuration file among the paths that exist in the repository.
///
/// # Errors
///
/// Several paths exist: none wins.
pub fn find_config_file<'a>(
    found: impl IntoIterator<Item = &'a str>,
) -> Result<Option<&'static str>, ConfigError> {
    let found: HashSet<_> = found.into_iter().collect();
    let mut existing = FILE_NAMES.into_iter().filter(|name| found.contains(name));

    match (existing.next(), existing.next()) {
        (name, None) => Ok(name),
        _ => Err(ConfigError::parsing(
            None,
            "several configuration files exist; keep only one of .cihub.yaml, .cihub.yml, \
             .cihub/config.yaml and .cihub/config.yml",
        )),
    }
}

#[derive(Clone, Debug)]
pub struct RepoConfig {
    /// Settings for every queue; each queue can replace them.
    pub defaults: Layer,
    /// The conditions of `defaults`, which every queue adds to its own.
    pub queue_conditions: Conditions,
    pub merge_conditions: Conditions,
    /// One entry per branch or pattern.
    pub queues: Vec<QueueConfig>,
    pub priority_rules: Vec<PriorityRule>,
}

#[derive(Clone, Debug)]
pub struct QueueConfig {
    /// A branch, or a glob of branches (`release/*`): each branch that it
    /// matches has its own queue.
    pub branch: String,
    /// `None` when `branch` names one branch.
    pub(crate) pattern: Option<Glob>,
    pub queue_conditions: Conditions,
    pub merge_conditions: Conditions,
    pub settings: Layer,
    pub profiles: Vec<ProfileConfig>,
}

/// Settings of a kind of PR in the train of a queue. Its `conditions`
/// choose it at the submission; the first profile that a PR meets wins.
#[derive(Clone, Debug)]
pub struct ProfileConfig {
    pub name: String,
    /// Empty: only a command that names the profile chooses it.
    pub conditions: Conditions,
    pub settings: Layer,
}

#[derive(Clone, Debug)]
pub struct PriorityRule {
    pub name: String,
    pub conditions: Conditions,
    pub priority: Priority,
}

impl QueueConfig {
    fn serves(&self, branch: &str) -> bool {
        match &self.pattern {
            Some(pattern) => pattern.is_match(branch),
            None => self.branch == branch,
        }
    }

    /// The settings of the queue, with those of each of its profiles.
    pub(crate) fn resolve(&self, defaults: &Layer) -> Result<QueueSettings, ConfigError> {
        let mut settings = QueueSettings::resolve(&self.settings, defaults)?;
        for profile in &self.profiles {
            let resolved =
                QueueSettings::resolve_profile(&profile.settings, &self.settings, defaults)?;
            settings.profiles.insert(profile.name.clone(), resolved);
        }

        Ok(settings)
    }
}

impl RepoConfig {
    /// Parses `.cihub.yaml`. Each queue is also resolved, so that values
    /// that do not go together fail here, with their position.
    ///
    /// # Errors
    ///
    /// The first error in the file.
    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        read::repo_config(text)
    }

    /// The entry of the queue of the branch `branch`, if any: the entry that
    /// names it, else the first pattern that matches it.
    pub fn queue(&self, branch: &str) -> Option<&QueueConfig> {
        self.queues
            .iter()
            .find(|queue| queue.pattern.is_none() && queue.branch == branch)
            .or_else(|| {
                self.queues
                    .iter()
                    .find(|queue| queue.pattern.is_some() && queue.serves(branch))
            })
    }

    /// The `queue_conditions` of the queue of `branch`: those of `defaults`,
    /// then those of its entry.
    pub fn queue_conditions(&self, branch: &str) -> Conditions {
        let own = self.queue(branch).map(|queue| &queue.queue_conditions);

        Conditions(
            defaults_then_own(&self.queue_conditions, own)
                .cloned()
                .collect(),
        )
    }

    /// The checks that the `merge_conditions` of `defaults` and of the queue
    /// of `branch` require on the test SHA.
    pub fn merge_checks(&self, branch: &str) -> Vec<String> {
        let own = self.queue(branch).map(|queue| &queue.merge_conditions);
        let required =
            defaults_then_own(&self.merge_conditions, own).filter_map(|expr| match expr {
                Expr::Condition(condition) => required_check(condition),
                _ => None,
            });

        let mut checks: Vec<String> = Vec::new();
        for check in required {
            if !checks.iter().any(|known| known == check) {
                checks.push(check.to_owned());
            }
        }
        checks
    }

    /// The profile `name` of the queue of `branch`, if the file has it.
    pub fn profile(&self, branch: &str, name: &str) -> Option<&ProfileConfig> {
        self.queue(branch)?
            .profiles
            .iter()
            .find(|profile| profile.name == name)
    }

    /// The settings of the queue of the branch `branch`. A queue without an
    /// entry has the `defaults` of the file.
    ///
    /// # Errors
    ///
    /// The values of the entry do not go together: `parse` already refuses
    /// such a file.
    pub fn settings(&self, branch: &str) -> Result<QueueSettings, ConfigError> {
        match self.queue(branch) {
            Some(queue) => queue.resolve(&self.defaults),
            None => QueueSettings::resolve(&Layer::default(), &self.defaults),
        }
    }
}

fn defaults_then_own<'a>(
    defaults: &'a Conditions,
    own: Option<&'a Conditions>,
) -> impl Iterator<Item = &'a Expr> {
    defaults
        .0
        .iter()
        .chain(own.into_iter().flat_map(|conditions| &conditions.0))
}

#[cfg(test)]
mod tests {
    use cihub_conditions::Context;
    use cihub_domain::{MergeMethod, Strategy};

    use super::*;
    use crate::{ConfigErrorKind, Position, Source};

    /// The example of the spec, section Configuration.
    const SPEC_EXAMPLE: &str = r"version: 1
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
";

    fn parse_error(yaml: &str) -> ConfigError {
        RepoConfig::parse(yaml).unwrap_err()
    }

    fn at(line: u64, column: u64) -> Position {
        Position { line, column }
    }

    fn queue(body: &str) -> String {
        format!("version: 1\nqueues:\n  - branch: main\n{body}")
    }

    #[test]
    fn parses_the_spec_example() {
        let config = RepoConfig::parse(SPEC_EXAMPLE).unwrap();
        let [main, release] = &config.queues[..] else {
            panic!("expected two queues");
        };

        assert_eq!(
            (main.branch.as_str(), release.branch.as_str()),
            ("main", "release/*")
        );
        assert_eq!(main.queue_conditions.0.len(), 4);
        assert_eq!(config.queue_conditions("main").0.len(), 5);
        assert_eq!(config.queue_conditions("release/2.0").0.len(), 2);
        assert_eq!(main.profiles[0].name, "dependencies");
        assert_eq!(
            config.settings("release/2.0").unwrap().merge_method.value,
            MergeMethod::FastForward
        );
        assert_eq!(
            config
                .priority_rules
                .iter()
                .map(|rule| rule.priority)
                .collect::<Vec<_>>(),
            [Priority::URGENT, Priority::LOW]
        );

        let settings = QueueSettings::resolve(&main.settings, &config.defaults).unwrap();
        assert_eq!(settings.batching_size.value, 8);
        assert_eq!(settings.batching_size.source, Source::File);
        assert_eq!(settings.batching_size.at, Some(at(25, 13)));
        assert_eq!(settings.batching_enabled.source, Source::Strategy);
        assert_eq!(settings.enqueue_label.value, "ready-to-merge");
        assert_eq!(settings.enqueue_label.source, Source::File);

        let ctx = Context {
            review_decision: cihub_domain::ReviewDecision::Approved,
            ..Context::default()
        };

        assert_eq!(main.queue_conditions.eval(&ctx), Ok(false));
        assert_eq!(
            config.priority_rules[0].conditions.0[0].to_string(),
            "any: [labels.any: urgent, hotfix; head.matches: hotfix/**]"
        );
        assert_eq!(config.merge_checks("main"), ["build", "integration-tests"]);
    }

    #[test]
    fn merge_conditions_accept_only_required_checks() {
        let config = RepoConfig::parse(&queue(
            "    merge_conditions:\n      checks:\n        success:\n          - build\n          - e2e tests\n",
        ))
        .unwrap();
        assert_eq!(config.merge_checks("main"), ["build", "e2e tests"]);
        assert_eq!(config.merge_checks("other"), [] as [String; 0]);
        for (body, shown, position) in [
            ("      labels: ready\n", "labels: ready", at(5, 7)),
            (
                "      checks:\n        success:\n          any:\n            - a\n",
                "checks.success.any: a",
                at(7, 11),
            ),
            (
                "      any:\n        - checks:\n            success: a\n",
                "any: [checks.success: a]",
                at(5, 7),
            ),
        ] {
            let error = parse_error(&queue(&format!("    merge_conditions:\n{body}")));
            assert_eq!(error.position, Some(position), "{body}");
            assert_eq!(
                error.message,
                format!("merge_conditions accept only `checks.success`, not `{shown}`")
            );
        }
    }

    #[test]
    fn a_queue_finds_its_entry_by_branch() {
        let text = "version: 1
defaults:
  testing_timeout_minutes: 60
queues:
  - branch: main
    strategy: sequential
  - branch: release/2.0
";
        let config = RepoConfig::parse(text).unwrap();
        let main = config.settings("main").unwrap();

        assert_eq!(main.strategy.value, Strategy::Sequential);
        assert_eq!(main.testing_timeout_minutes.value, 60);
        let release = config.settings("release/2.0").unwrap();
        assert_eq!(release.strategy.value, Strategy::Pipelined);
        assert_eq!(release.testing_timeout_minutes.source, Source::File);

        assert!(config.queue("dev").is_none());
        let settings = config.settings("dev").unwrap();
        assert_eq!(settings.strategy.value, Strategy::Pipelined);
        assert_eq!(settings.testing_timeout_minutes.value, 60);
    }

    #[test]
    fn defaults_without_a_queue_must_go_together() {
        let error = parse_error("version: 1\ndefaults:\n  merge_method: fast_forward\n");
        assert!(
            error.message.contains("needs strategy"),
            "{}",
            error.message
        );
    }

    #[test]
    fn testing_concurrency_sets_the_whole_window() {
        let config = RepoConfig::parse(&queue("    testing_concurrency: 3\n")).unwrap();
        let settings =
            QueueSettings::resolve(&config.queues[0].settings, &config.defaults).unwrap();

        assert_eq!(
            (settings.window_floor.value, settings.window_ceiling.value),
            (3, 3)
        );
        assert_eq!(settings.bisection_concurrency.value, 3);

        let error = parse_error(&queue(
            "    window: { floor: 2 }\n    testing_concurrency: 3\n",
        ));

        assert_eq!(error.position, Some(at(5, 26)));
        assert!(error.message.contains("cannot both be set"));
    }

    #[test]
    fn required_statuses_null_inherits_and_absent_leaves_it_unset() {
        let config = RepoConfig::parse(&queue("    required_statuses: null\n")).unwrap();
        assert_eq!(
            config.queues[0]
                .settings
                .required_statuses
                .as_ref()
                .map(|setting| &setting.value),
            Some(&None)
        );
        let config = RepoConfig::parse(&queue("    required_statuses: []\n")).unwrap();
        assert_eq!(
            config.queues[0]
                .settings
                .required_statuses
                .as_ref()
                .map(|setting| &setting.value),
            Some(&Some(vec![]))
        );
        let config = RepoConfig::parse(&queue("")).unwrap();
        assert_eq!(config.queues[0].settings.required_statuses, None);
    }

    #[test]
    fn reports_condition_errors_at_their_key_or_value() {
        let conditions =
            |body: &str| parse_error(&queue(&format!("    queue_conditions:\n{body}")));

        let error = conditions("      draft: false\n      lables: x\n");
        assert_eq!(error.position, Some(at(6, 7)));
        assert_eq!(error.message, "unknown attribute `lables`");

        let error = conditions("      checks:\n        green: [a]\n");
        assert_eq!(error.position, Some(at(6, 9)));
        assert_eq!(error.message, "unknown attribute `checks.green`");

        let error = conditions("      labels:\n        some: [a]\n");
        assert_eq!(error.position, Some(at(6, 9)));
        assert_eq!(
            error.message,
            "unknown key `some` under `labels`: expected all, any, none, all_match, any_match, none_match, count"
        );

        let error = conditions("      title:\n        regex: (\n");
        assert_eq!(error.position, Some(at(6, 16)));
        assert_eq!(
            error.message,
            "title.regex: invalid regular expression: unclosed group"
        );

        let error = conditions("      files:\n        count:\n          max: x\n");
        assert_eq!(error.position, Some(at(7, 16)));
        assert_eq!(
            error.message,
            "files.count.max: `x` is not an integer from 0 to 4294967295"
        );

        let error = conditions("      review_decision:\n        - approved\n        - ok\n");
        assert_eq!(error.position, Some(at(7, 11)));

        let error = conditions("      additions:\n        max: [1, 2]\n");
        assert_eq!(error.message, "additions.max: takes one value, not a list");

        let error = conditions("      additions:\n        min: 10\n        max: 5\n");
        assert_eq!(error.position, Some(at(7, 14)));
        assert_eq!(error.message, "additions.max: below `min`");

        let error =
            conditions("      labels:\n        count:\n          max: 1\n          min: 2\n");
        assert_eq!(error.position, Some(at(7, 16)));
        assert_eq!(error.message, "labels.count.max: below `min`");

        let error = conditions("      sender_permission:\n        min: admin\n        max: read\n");
        assert_eq!(error.position, Some(at(7, 14)));
        assert_eq!(error.message, "sender_permission.max: below `min`");

        let error = conditions("      pr_age: 2h\n");
        assert_eq!(error.position, Some(at(5, 15)));
        assert_eq!(
            error.message,
            "pr_age: expected a key (min, max), not a value"
        );

        let error = conditions("      draft:\n");
        assert_eq!(error.message, "draft: missing value");

        let error = conditions("      draft: no\n");
        assert_eq!(error.message, "draft: `no` is not `true` or `false`");

        let config =
            RepoConfig::parse(&queue("    queue_conditions:\n      labels: [no, on]\n")).unwrap();
        assert_eq!(
            config.queue_conditions("main").0[1].to_string(),
            "labels: on"
        );

        let error = conditions("      labels: [a]\n      labels: [b]\n");
        assert_eq!(error.position.map(|position| position.line), Some(6));

        let error = conditions("      labels:\n        any:\n          - [a]\n");
        assert_eq!(error.position, Some(at(7, 13)));
        assert_eq!(error.message, "expected a value or a list of values");

        let error = conditions("      - draft: false\n");
        assert_eq!(error.message, "expected a map of conditions");

        let error = conditions("      any:\n        labels: [a]\n");
        assert_eq!(error.message, "`any` takes a list of condition maps");

        let error = conditions("      any:\n        - or: x\n");
        assert_eq!(error.position, Some(at(6, 11)));
        assert_eq!(error.message, "unknown attribute `or`");

        let error = conditions("      checks: [lint]\n");
        assert!(
            error.message.starts_with("expected a map of check states"),
            "{error}"
        );
    }

    #[test]
    fn refuses_empty_maps_and_a_second_form_of_a_key() {
        let conditions =
            |body: &str| parse_error(&queue(&format!("    queue_conditions:\n{body}")));

        for (body, message) in [
            (
                "      labels: {}\n",
                "expected the operators of `labels`, not an empty map",
            ),
            ("      checks: {}\n", "not an empty map"),
            ("      any: []\n", "`any` takes a list of condition maps"),
            (
                "      all:\n        - {}\n",
                "expected a map of conditions, not an empty map",
            ),
            (
                "      files:\n        count: {}\n",
                "expected `min` or `max`, not an empty map",
            ),
            (
                "      checks.success: lint\n",
                "unknown attribute `checks.success`",
            ),
            (
                "      files:\n        count.min: 1\n",
                "unknown key `count.min` under `files`",
            ),
            (
                "      files:\n        count:\n          least: 1\n",
                "unknown key `least` under `count`",
            ),
        ] {
            let error = conditions(body);
            assert!(error.message.contains(message), "{body}: {error}");
        }
    }

    #[test]
    fn accepts_a_min_equal_to_the_max() {
        let body = "    queue_conditions:\n      additions:\n        min: 5\n        max: 5\n";

        assert!(RepoConfig::parse(&queue(body)).is_ok());
    }

    #[test]
    fn keeps_a_number_as_the_file_writes_it() {
        let config = RepoConfig::parse(&queue(
            "    queue_conditions:\n      checks:\n        success:\n          - 3.10\n          - 1.0\n          - 0x10\n      additions:\n        max: 50\n",
        ))
        .unwrap();
        let shown: Vec<String> = config
            .queue_conditions("main")
            .0
            .iter()
            .map(ToString::to_string)
            .collect();

        assert_eq!(
            shown,
            [
                "checks.success: 3.10",
                "checks.success: 1.0",
                "checks.success: 0x10",
                "additions.max: 50"
            ]
        );
    }

    #[test]
    fn a_merge_condition_error_points_at_its_key() {
        let error = parse_error(&queue(
            "    merge_conditions:\n      checks:\n        success: [ci]\n      labels: [ready]\n",
        ));
        assert_eq!(error.position, Some(at(7, 7)));

        let nested = "    merge_conditions:\n      checks:\n        success:\n          all: [a]\n          any: [b]\n        pending: [c]\n";
        let error = parse_error(&queue(nested));
        assert_eq!(error.position, Some(at(8, 11)));
        assert!(
            error.message.ends_with("not `checks.success.any: b`"),
            "{}",
            error.message
        );

        let error = parse_error(&queue(&nested.replace("          any: [b]\n", "")));
        assert_eq!(error.position, Some(at(8, 9)));
        assert!(
            error.message.ends_with("not `checks.pending: c`"),
            "{}",
            error.message
        );
    }

    #[test]
    fn an_empty_conditions_map_of_a_profile_or_a_rule_is_refused() {
        let profile = |conditions: &str| {
            queue(&format!(
                "    profiles:\n      - name: bots\n        conditions:{conditions}\n"
            ))
        };
        let rule = |conditions: &str| {
            format!(
                "version: 1\npriority_rules:\n  - name: a\n    priority: high\n    conditions:{conditions}\n"
            )
        };

        let empty = "expected a map of conditions, not an empty map";
        for (yaml, position, message) in [
            (profile(" {}"), at(6, 21), empty),
            (profile(""), at(6, 19), "expected a map of conditions"),
            (rule(" {}"), at(5, 17), empty),
            (rule(""), at(5, 15), "expected a map of conditions"),
        ] {
            let error = parse_error(&yaml);
            assert_eq!(error.position, Some(position), "{yaml}");
            assert_eq!(error.message, message, "{yaml}");
        }

        let named = queue("    profiles:\n      - name: bots\n");
        assert!(
            RepoConfig::parse(&named).unwrap().queues[0].profiles[0]
                .conditions
                .0
                .is_empty()
        );
    }

    #[test]
    fn a_block_holds_maps_of_one_or_several_conditions() {
        let config = RepoConfig::parse(&queue(
            "    queue_conditions:\n      none:\n        - draft: true\n        - author: bob\n          labels: [wip]\n",
        ))
        .unwrap();

        assert_eq!(
            config.queue_conditions("main").0[0].to_string(),
            "none: [draft: true; all: [author: bob; labels: wip]]"
        );
    }

    #[test]
    fn reports_invalid_settings_at_their_value() {
        let error = parse_error(&queue("    batching: { size: 50 }\n"));
        assert_eq!(error.position, Some(at(4, 23)));
        assert_eq!(error.message, "batching.size is 50, out of range 2–20");

        let error = parse_error(&queue("    window: { floor: 30, ceiling: 20 }\n"));
        assert_eq!(error.position, Some(at(4, 22)));

        let error = parse_error(&queue("    trigger: inplace\n"));
        assert_eq!(error.position, Some(at(4, 14)));

        let error = parse_error(&queue("    strategy: fast\n"));
        assert_eq!(error.kind, ConfigErrorKind::Parsing);
        assert_eq!(error.position.map(|position| position.line), Some(4));

        let error = parse_error(&queue("    colour: blue\n"));
        assert_eq!(error.position.map(|position| position.line), Some(4));
        assert!(error.message.contains("colour"), "{error}");

        let error = parse_error(&queue("    required_statuses: [ci, ci]\n"));
        assert_eq!(error.kind, ConfigErrorKind::BadRequiredStatuses);
        assert_eq!(
            error.job_error(),
            cihub_domain::JobError::ConfigBadRequiredStatuses
        );
    }

    #[test]
    fn checks_the_version_first() {
        let error = parse_error("queues: []\n");
        assert_eq!(error.kind, ConfigErrorKind::BadVersion);
        let error = parse_error("version: 2\nqueues:\n  - colour: blue\n");
        assert_eq!(error.kind, ConfigErrorKind::BadVersion);
        assert_eq!(error.position, Some(at(1, 10)));
        assert_eq!(parse_error("").kind, ConfigErrorKind::BadVersion);
    }

    #[test]
    fn checks_queues_and_defaults() {
        let error = parse_error("version: 1\nqueues:\n  - strategy: batched\n");
        assert_eq!(error.message, "a queue needs a `branch`");
        assert_eq!(error.position.map(|position| position.line), Some(3));

        let error = parse_error(&queue("    name: hotfix\n"));
        assert!(error.message.contains("name"), "{error}");
        assert_eq!(error.position.map(|position| position.line), Some(4));

        let error = parse_error(&format!("{}  - branch: main\n", queue("")));
        assert_eq!(error.message, "the queue of `main` is defined twice");
        assert_eq!(error.position, Some(at(4, 13)));

        for (defaults, key, position) in [
            ("  branch: main\n", "branch", at(3, 11)),
            ("  profiles:\n    - name: a\n", "profiles", at(4, 5)),
        ] {
            let error = parse_error(&format!("version: 1\ndefaults:\n{defaults}"));
            assert_eq!(
                error.message,
                format!("`{key}` is not allowed in `defaults`")
            );
            assert_eq!(error.position, Some(position), "{key}");
        }
    }

    #[test]
    fn reads_yaml_anchors_and_merge_keys() {
        let text = "version: 1
queues:
  - branch: main
    queue_conditions: &checks
      checks:
        success: [ci]
    <<: &fast
      strategy: batched
      batching: { size: 8 }
  - branch: dev
    queue_conditions: *checks
    <<: *fast
";
        let config = RepoConfig::parse(text).unwrap();

        for branch in ["main", "dev"] {
            let settings = config.settings(branch).unwrap();
            assert_eq!(settings.strategy.value, Strategy::Batched, "{branch}");
            assert_eq!(settings.batching_size.value, 8, "{branch}");
            assert_eq!(config.queue_conditions(branch).0.len(), 1, "{branch}");
        }
    }

    #[test]
    fn an_entry_that_names_the_branch_wins_over_a_pattern() {
        let text = "version: 1
queues:
  - branch: \"release/*\"
    strategy: sequential
  - branch: release/2.0
    strategy: batched
";
        let config = RepoConfig::parse(text).unwrap();
        let strategy = |branch: &str| config.settings(branch).unwrap().strategy.value;

        assert_eq!(strategy("release/2.0"), Strategy::Batched);
        assert_eq!(strategy("release/2.1"), Strategy::Sequential);
        assert_eq!(strategy("release/2.1/fix"), Strategy::Pipelined);
        assert!(config.queue("main").is_none());

        let error = parse_error("version: 1\nqueues:\n  - branch: \"release/[\"\n");
        assert!(error.message.starts_with("branch `release/[`"), "{error}");
        assert_eq!(error.position.map(|position| position.line), Some(3));
    }

    #[test]
    fn the_conditions_of_defaults_come_before_those_of_the_queue() {
        let text = "version: 1
defaults:
  queue_conditions:
    draft: false
  merge_conditions:
    checks:
      success: [build]
queues:
  - branch: main
    queue_conditions:
      labels: [ready]
    merge_conditions:
      checks:
        success: [e2e, build]
";
        let config = RepoConfig::parse(text).unwrap();
        let shown = |branch: &str| {
            config
                .queue_conditions(branch)
                .0
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        };

        assert_eq!(shown("main"), ["draft: false", "labels: ready"]);
        assert_eq!(shown("dev"), ["draft: false"]);
        assert_eq!(config.merge_checks("main"), ["build", "e2e"]);
        assert_eq!(config.merge_checks("dev"), ["build"]);
    }

    #[test]
    fn a_profile_sets_the_settings_of_a_pr_over_those_of_its_queue() {
        let text = "version: 1
queues:
  - branch: main
    strategy: batched
    merge_method: squash
    profiles:
      - name: dependencies
        conditions:
          author: dependabot[bot]
        merge_method: merge_commit
        batching: { size: 8 }
        failure_budget: { retries: 2 }
      - name: manual
";
        let config = RepoConfig::parse(text).unwrap();
        let settings = config.settings("main").unwrap();
        let dependencies = settings.of_profile(Some("dependencies"));

        assert_eq!(dependencies.merge_method.value, MergeMethod::MergeCommit);
        assert_eq!(dependencies.batching_size.value, 8);
        assert_eq!(dependencies.failure_budget_retries.value, 2);
        assert_eq!(dependencies.strategy.value, Strategy::Batched);
        assert!(dependencies.batching_enabled.value);
        assert_eq!(
            settings.of_profile(None).merge_method.value,
            MergeMethod::Squash
        );
        assert_eq!(settings.of_profile(Some("gone")).batching_size.value, 4);
        assert!(
            config
                .profile("main", "manual")
                .unwrap()
                .conditions
                .0
                .is_empty()
        );
        assert!(config.profile("dev", "manual").is_none());
    }

    #[test]
    fn a_profile_sets_only_the_settings_of_a_pr() {
        let profiles = |body: &str| queue(&format!("    profiles:\n      - name: a\n{body}"));

        let error = parse_error(&profiles("        merge_method: fast_forward\n"));
        assert!(
            error
                .message
                .contains("cannot set merge_method `fast_forward`"),
            "{error}"
        );
        assert_eq!(error.position, Some(at(6, 23)));

        let error = parse_error(&profiles("        strategy: sequential\n"));
        assert!(error.message.contains("strategy"), "{error}");

        let error = parse_error(&profiles("        batching: { size: 50 }\n"));
        assert_eq!(error.message, "batching.size is 50, out of range 2–20");

        let error = parse_error(&format!("{}      - name: a\n", profiles("")));
        assert_eq!(error.message, "the profile `a` is defined twice");
        assert_eq!(error.position.map(|position| position.line), Some(6));
    }

    #[test]
    fn reads_priorities_by_name_or_value() {
        let rules = |priority: &str| {
            RepoConfig::parse(&format!(
                "version: 1\npriority_rules:\n  - name: r\n    conditions: {{ draft: true }}\n    priority: {priority}\n"
            ))
        };
        assert_eq!(
            rules("42").unwrap().priority_rules[0].priority,
            Priority(42)
        );
        assert_eq!(
            rules("high").unwrap().priority_rules[0].priority,
            Priority::HIGH
        );
        assert_eq!(rules("top").unwrap_err().position, Some(at(5, 15)));
    }

    #[test]
    fn finds_one_config_file() {
        assert_eq!(
            find_config_file(["README.md", ".cihub.yml"]),
            Ok(Some(".cihub.yml"))
        );
        assert_eq!(
            find_config_file(["README.md", ".cihub/config.yaml"]),
            Ok(Some(".cihub/config.yaml"))
        );
        assert_eq!(find_config_file(["README.md"]), Ok(None));
        assert!(find_config_file([".cihub.yaml", ".cihub.yml"]).is_err());
        assert!(find_config_file([".cihub.yml", ".cihub/config.yml"]).is_err());
    }
}

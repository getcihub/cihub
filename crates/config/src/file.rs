//! The `.cihub.yaml` model, read with the position of each value.

use std::collections::HashSet;

use cihub_conditions::{Conditions, Expr, Glob};
use cihub_domain::{
    BranchProtectionInjection, DirectMergeMode, MergeCommitTitle, MergeMethod, OptimizationMode,
    Priority, QueueMode, Strategy, Trigger, UpdateMethod,
};
use serde::Deserialize;
use serde::de::{Deserializer, IgnoredAny};
use serde_saphyr::{DefaultMessageFormatter, MessageFormatter, Spanned};

use crate::conditions::{RawNode, Reader};
use crate::settings::in_range;
use crate::{ConfigError, ConfigErrorKind, Layer, Position, QueueSettings, Setting};

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
    pattern: Option<Glob>,
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

impl QueueConfig {
    fn serves(&self, branch: &str) -> bool {
        match &self.pattern {
            Some(pattern) => pattern.is_match(branch),
            None => self.branch == branch,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PriorityRule {
    pub name: String,
    pub conditions: Conditions,
    pub priority: Priority,
}

impl RepoConfig {
    /// Parses `.cihub.yaml`. Each queue is also resolved, so that values
    /// that do not go together fail here, with their position.
    ///
    /// # Errors
    ///
    /// The first error in the file.
    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        check_version(text)?;
        let file: RawFile = from_str(text)?;
        let reader = Reader { text };
        let (defaults, queue_conditions, merge_conditions) = match file.defaults {
            Some(raw) => defaults(raw, reader)?,
            None => Default::default(),
        };

        QueueSettings::resolve(&Layer::default(), &defaults)?;
        let mut branches = HashSet::new();
        let mut queues = Vec::with_capacity(file.queues.len());

        for raw in file.queues {
            let (queue, branch_at) = queue(raw, reader)?;
            if !branches.insert(queue.branch.clone()) {
                return Err(ConfigError::parsing(
                    branch_at,
                    format!("the queue of `{}` is defined twice", queue.branch),
                ));
            }
            QueueSettings::resolve(&queue.settings, &defaults)?;
            for profile in &queue.profiles {
                QueueSettings::resolve_profile(&profile.settings, &queue.settings, &defaults)?;
            }
            queues.push(queue);
        }

        let priority_rules = file
            .priority_rules
            .into_iter()
            .map(|rule| priority_rule(rule, reader))
            .collect::<Result<_, _>>()?;

        Ok(Self {
            defaults,
            queue_conditions,
            merge_conditions,
            queues,
            priority_rules,
        })
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
        let own = self.queue(branch).map(|queue| &queue.queue_conditions.0);
        Conditions(
            self.queue_conditions
                .0
                .iter()
                .chain(own.into_iter().flatten())
                .cloned()
                .collect(),
        )
    }

    /// The checks that the `merge_conditions` of `defaults` and of the queue
    /// of `branch` require on the test SHA.
    pub fn merge_checks(&self, branch: &str) -> Vec<String> {
        let own = self.queue(branch).map(|queue| &queue.merge_conditions.0);
        let mut checks: Vec<String> = Vec::new();
        for check in self
            .merge_conditions
            .0
            .iter()
            .chain(own.into_iter().flatten())
            .filter_map(|expr| match expr {
                Expr::Condition(condition) => condition.required_check(),
                _ => None,
            })
        {
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
        let Some(queue) = self.queue(branch) else {
            return QueueSettings::resolve(&Layer::default(), &self.defaults);
        };

        let mut settings = QueueSettings::resolve(&queue.settings, &self.defaults)?;
        for profile in &queue.profiles {
            let resolved =
                QueueSettings::resolve_profile(&profile.settings, &queue.settings, &self.defaults)?;
            settings.profiles.insert(profile.name.clone(), resolved);
        }
        Ok(settings)
    }
}

fn check_version(text: &str) -> Result<(), ConfigError> {
    #[derive(Deserialize)]
    struct Probe {
        #[serde(default)]
        version: Option<Spanned<u32>>,
    }
    let probe: Option<Probe> = from_str(text)?;
    let bad_version = |position, message: String| ConfigError {
        kind: ConfigErrorKind::BadVersion,
        position,
        message,
    };
    match probe.and_then(|probe| probe.version) {
        None => Err(bad_version(None, "missing `version: 1`".into())),
        Some(version) if version.value != 1 => Err(bad_version(
            position(&version),
            format!(
                "unsupported version {}; this CIHub reads version 1",
                version.value
            ),
        )),
        Some(_) => Ok(()),
    }
}

/// Only `true` and `false` are booleans: YAML 1.1 also reads `yes`, `no`,
/// `on` and `off` as booleans, which would turn a label `no` into `false`.
fn from_str<'de, T: Deserialize<'de>>(text: &'de str) -> Result<T, ConfigError> {
    let options = serde_saphyr::options! { strict_booleans: true };
    serde_saphyr::from_str_with_options(text, options).map_err(|e| yaml_error(&e))
}

fn yaml_error(error: &serde_saphyr::Error) -> ConfigError {
    let error = error.without_snippet();
    let message = DefaultMessageFormatter
        .format_message(error)
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>();
    ConfigError::parsing(error.location().map(|l| Position::of(&l)), message)
}

type Value<T> = Option<Spanned<T>>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    #[serde(rename = "version")]
    _version: IgnoredAny,
    #[serde(default)]
    queues: Vec<Spanned<RawQueue>>,
    #[serde(default)]
    priority_rules: Vec<RawPriorityRule>,
    #[serde(default)]
    defaults: Option<RawQueue>,
}

/// A queue, or `defaults` without `branch` and `profiles`.
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawQueue {
    branch: Value<String>,
    queue_conditions: Value<RawNode>,
    merge_conditions: Value<RawNode>,
    profiles: Value<Vec<Spanned<RawProfile>>>,
    strategy: Value<Strategy>,
    mode: Value<QueueMode>,
    window: Value<RawWindow>,
    testing_concurrency: Value<u32>,
    bisection_concurrency: Value<u32>,
    testing_timeout_minutes: Value<u32>,
    not_ready_timeout_hours: Value<u32>,
    merge_method: Value<MergeMethod>,
    merge_commit_title: Value<MergeCommitTitle>,
    trigger: Value<Trigger>,
    update_method: Value<UpdateMethod>,
    batching: Option<RawBatching>,
    failure_budget: Option<RawFailureBudget>,
    optimistic_merging: Value<bool>,
    pending_failure_depth: Value<u32>,
    direct_merge_mode: Value<DirectMergeMode>,
    optimization_mode: Value<OptimizationMode>,
    comments_enabled: Value<bool>,
    commands_enabled: Value<bool>,
    status_check_enabled: Value<bool>,
    label_commands: Option<RawLabelCommands>,
    state_labels_enabled: Value<bool>,
    resubmit_on_push: Value<bool>,
    keep_test_branches_minutes: Value<u32>,
    branch_prefix: Value<String>,
    branch_protection_injection: Value<BranchProtectionInjection>,
    /// `null` is a value here: it inherits the branch protection.
    #[serde(deserialize_with = "present")]
    required_statuses: Value<Option<Vec<String>>>,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawWindow {
    floor: Value<u32>,
    ceiling: Value<u32>,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawBatching {
    enabled: Value<bool>,
    size: Value<u32>,
    max_wait_minutes: Value<u32>,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawFailureBudget {
    retries: Value<u32>,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawLabelCommands {
    enabled: Value<bool>,
    enqueue_label: Value<String>,
}

/// A profile sets only the settings of a PR, never those of the train.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProfile {
    name: Spanned<String>,
    #[serde(default, deserialize_with = "present")]
    conditions: Value<RawNode>,
    #[serde(default)]
    merge_method: Value<MergeMethod>,
    #[serde(default)]
    merge_commit_title: Value<MergeCommitTitle>,
    #[serde(default)]
    batching: Option<RawBatching>,
    #[serde(default)]
    failure_budget: Option<RawFailureBudget>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPriorityRule {
    name: Spanned<String>,
    conditions: Spanned<RawNode>,
    priority: Spanned<RawPriority>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawPriority {
    Value(u8),
    Name(String),
}

fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

/// The queue and the position of its branch.
fn queue(
    raw: Spanned<RawQueue>,
    reader: Reader<'_>,
) -> Result<(QueueConfig, Option<Position>), ConfigError> {
    let queue_at = position(&raw);
    let mut raw = raw.value;
    let (branch_at, branch) = raw
        .branch
        .take()
        .map(|v| (position(&v), v.value))
        .ok_or_else(|| ConfigError::parsing(queue_at, "a queue needs a `branch`"))?;
    let pattern = branch_pattern(&branch, branch_at)?;
    let profiles = raw
        .profiles
        .take()
        .map(|v| v.value)
        .unwrap_or_default()
        .into_iter()
        .map(|raw| profile(raw, reader))
        .collect::<Result<Vec<_>, _>>()?;
    let mut names = HashSet::new();

    for (name, at) in &profiles {
        if !names.insert(name.name.clone()) {
            return Err(ConfigError::parsing(
                *at,
                format!("the profile `{}` is defined twice", name.name),
            ));
        }
    }

    let queue = QueueConfig {
        branch,
        pattern,
        queue_conditions: reader.conditions(raw.queue_conditions.take().as_ref())?,
        merge_conditions: reader.merge_conditions(raw.merge_conditions.take().as_ref())?,
        settings: layer(raw)?,
        profiles: profiles.into_iter().map(|(profile, _)| profile).collect(),
    };

    Ok((queue, branch_at))
}

/// The glob of `branch` when it holds one of `*?[`, which git refuses in a
/// branch name; `None` when it names one branch.
fn branch_pattern(branch: &str, at: Option<Position>) -> Result<Option<Glob>, ConfigError> {
    if !branch.contains(['*', '?', '[']) {
        return Ok(None);
    }

    Glob::new(branch)
        .map(Some)
        .map_err(|kind| ConfigError::parsing(at, format!("branch `{branch}`: {kind}")))
}

/// The profile and the position of its name.
fn profile(
    raw: Spanned<RawProfile>,
    reader: Reader<'_>,
) -> Result<(ProfileConfig, Option<Position>), ConfigError> {
    let raw = raw.value;
    let at = position(&raw.name);

    if let Some(method) = &raw.merge_method
        && method.value == MergeMethod::FastForward
    {
        return Err(ConfigError::parsing(
            position(method),
            "a profile cannot set merge_method `fast_forward`: it moves the target of the whole train",
        ));
    }

    let batching = raw.batching.unwrap_or_default();
    let settings = Layer {
        merge_method: setting(raw.merge_method),
        merge_commit_title: setting(raw.merge_commit_title),
        batching_enabled: setting(batching.enabled),
        batching_size: setting(batching.size),
        batching_max_wait_minutes: setting(batching.max_wait_minutes),
        failure_budget_retries: setting(raw.failure_budget.unwrap_or_default().retries),
        ..Layer::default()
    };
    let profile = ProfileConfig {
        name: raw.name.value,
        conditions: match raw.conditions.as_ref() {
            Some(node) => reader.selecting_conditions(node)?,
            None => Conditions::default(),
        },
        settings,
    };

    Ok((profile, at))
}

/// The settings and the conditions of `defaults`.
fn defaults(
    raw: RawQueue,
    reader: Reader<'_>,
) -> Result<(Layer, Conditions, Conditions), ConfigError> {
    let queue_only = [
        ("branch", raw.branch.as_ref().and_then(position)),
        ("profiles", raw.profiles.as_ref().and_then(position)),
    ];
    if let Some((key, at)) = queue_only.into_iter().find(|(_, at)| at.is_some()) {
        return Err(ConfigError::parsing(
            at,
            format!("`{key}` is not allowed in `defaults`"),
        ));
    }

    let mut raw = raw;
    let queue_conditions = reader.conditions(raw.queue_conditions.take().as_ref())?;
    let merge_conditions = reader.merge_conditions(raw.merge_conditions.take().as_ref())?;

    Ok((layer(raw)?, queue_conditions, merge_conditions))
}

#[allow(clippy::unnecessary_wraps, reason = "used with `Option::and_then`")]
fn position<T>(value: &Spanned<T>) -> Option<Position> {
    Some(Position::of(&value.referenced))
}

fn setting<T>(value: Value<T>) -> Option<Setting<T>> {
    value.map(|v| Setting {
        at: position(&v),
        value: v.value,
    })
}

fn layer(raw: RawQueue) -> Result<Layer, ConfigError> {
    let (window_floor, window_ceiling) = match (raw.testing_concurrency, raw.window) {
        (Some(concurrency), Some(_)) => {
            return Err(ConfigError::parsing(
                position(&concurrency),
                "`testing_concurrency` and `window` cannot both be set: \
                 `testing_concurrency: N` means `window: { floor: N, ceiling: N }`",
            ));
        }
        (Some(concurrency), None) => {
            let concurrency = setting(Some(concurrency));
            in_range(concurrency.as_ref(), "testing_concurrency", 1..=100)?;
            (concurrency.clone(), concurrency)
        }
        (None, window) => {
            let window = window.map(|w| w.value).unwrap_or_default();
            (setting(window.floor), setting(window.ceiling))
        }
    };
    let batching = raw.batching.unwrap_or_default();
    let label_commands = raw.label_commands.unwrap_or_default();
    Ok(Layer {
        strategy: setting(raw.strategy),
        mode: setting(raw.mode),
        window_floor,
        window_ceiling,
        bisection_concurrency: setting(raw.bisection_concurrency),
        testing_timeout_minutes: setting(raw.testing_timeout_minutes),
        not_ready_timeout_hours: setting(raw.not_ready_timeout_hours),
        merge_method: setting(raw.merge_method),
        merge_commit_title: setting(raw.merge_commit_title),
        trigger: setting(raw.trigger),
        update_method: setting(raw.update_method),
        batching_enabled: setting(batching.enabled),
        batching_size: setting(batching.size),
        batching_max_wait_minutes: setting(batching.max_wait_minutes),
        failure_budget_retries: setting(raw.failure_budget.unwrap_or_default().retries),
        optimistic_merging: setting(raw.optimistic_merging),
        pending_failure_depth: setting(raw.pending_failure_depth),
        direct_merge_mode: setting(raw.direct_merge_mode),
        optimization_mode: setting(raw.optimization_mode),
        comments_enabled: setting(raw.comments_enabled),
        commands_enabled: setting(raw.commands_enabled),
        status_check_enabled: setting(raw.status_check_enabled),
        label_commands_enabled: setting(label_commands.enabled),
        enqueue_label: setting(label_commands.enqueue_label),
        state_labels_enabled: setting(raw.state_labels_enabled),
        resubmit_on_push: setting(raw.resubmit_on_push),
        keep_test_branches_minutes: setting(raw.keep_test_branches_minutes),
        branch_prefix: setting(raw.branch_prefix),
        branch_protection_injection: setting(raw.branch_protection_injection),
        required_statuses: setting(raw.required_statuses),
    })
}

fn priority_rule(raw: RawPriorityRule, reader: Reader<'_>) -> Result<PriorityRule, ConfigError> {
    let priority = match raw.priority.value {
        RawPriority::Value(value) => Priority(value),
        RawPriority::Name(name) => Priority::from_name(&name).ok_or_else(|| {
            ConfigError::parsing(
                Some(Position::of(&raw.priority.referenced)),
                format!("unknown priority `{name}`: expected urgent, high, medium, low or 0-255"),
            )
        })?,
    };
    Ok(PriorityRule {
        name: raw.name.value,
        conditions: reader.selecting_conditions(&raw.conditions)?,
        priority,
    })
}

#[cfg(test)]
mod tests {
    use cihub_conditions::Context;

    use super::*;
    use crate::Source;

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

    fn error(yaml: &str) -> ConfigError {
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
            let error = error(&queue(&format!("    merge_conditions:\n{body}")));
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
        let e = error("version: 1\ndefaults:\n  merge_method: fast_forward\n");
        assert!(e.message.contains("needs strategy"), "{}", e.message);
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

        let e = error(&queue(
            "    window: { floor: 2 }\n    testing_concurrency: 3\n",
        ));

        assert_eq!(e.position, Some(at(5, 26)));
        assert!(e.message.contains("cannot both be set"));
    }

    #[test]
    fn required_statuses_null_inherits_and_absent_leaves_it_unset() {
        let config = RepoConfig::parse(&queue("    required_statuses: null\n")).unwrap();
        assert_eq!(
            config.queues[0]
                .settings
                .required_statuses
                .as_ref()
                .map(|s| &s.value),
            Some(&None)
        );
        let config = RepoConfig::parse(&queue("    required_statuses: []\n")).unwrap();
        assert_eq!(
            config.queues[0]
                .settings
                .required_statuses
                .as_ref()
                .map(|s| &s.value),
            Some(&Some(vec![]))
        );
        let config = RepoConfig::parse(&queue("")).unwrap();
        assert_eq!(config.queues[0].settings.required_statuses, None);
    }

    #[test]
    fn reports_condition_errors_at_their_key_or_value() {
        let conditions = |body: &str| error(&queue(&format!("    queue_conditions:\n{body}")));

        let e = conditions("      draft: false\n      lables: x\n");
        assert_eq!(e.position, Some(at(6, 7)));
        assert_eq!(e.message, "unknown attribute `lables`");

        let e = conditions("      checks:\n        green: [a]\n");
        assert_eq!(e.position, Some(at(6, 9)));
        assert_eq!(e.message, "unknown attribute `checks.green`");

        let e = conditions("      labels:\n        some: [a]\n");
        assert_eq!(e.position, Some(at(6, 9)));
        assert_eq!(
            e.message,
            "unknown key `some` under `labels`: expected all, any, none, all_match, any_match, none_match, count"
        );

        let e = conditions("      title:\n        regex: (\n");
        assert_eq!(e.position, Some(at(6, 16)));
        assert_eq!(
            e.message,
            "title.regex: invalid regular expression: unclosed group"
        );

        let e = conditions("      files:\n        count:\n          max: x\n");
        assert_eq!(e.position, Some(at(7, 16)));
        assert_eq!(
            e.message,
            "files.count.max: `x` is not an integer from 0 to 4294967295"
        );

        let e = conditions("      review_decision:\n        - approved\n        - ok\n");
        assert_eq!(e.position, Some(at(7, 11)));

        let e = conditions("      additions:\n        max: [1, 2]\n");
        assert_eq!(e.message, "additions.max: takes one value, not a list");

        let e = conditions("      additions:\n        min: 10\n        max: 5\n");
        assert_eq!(e.position, Some(at(7, 14)));
        assert_eq!(e.message, "additions.max: below `min`");

        let e = conditions("      labels:\n        count:\n          max: 1\n          min: 2\n");
        assert_eq!(e.position, Some(at(7, 16)));
        assert_eq!(e.message, "labels.count.max: below `min`");

        let e = conditions("      sender_permission:\n        min: admin\n        max: read\n");
        assert_eq!(e.position, Some(at(7, 14)));
        assert_eq!(e.message, "sender_permission.max: below `min`");

        let e = conditions("      pr_age: 2h\n");
        assert_eq!(e.position, Some(at(5, 15)));
        assert_eq!(e.message, "pr_age: expected a key (min, max), not a value");

        let e = conditions("      draft:\n");
        assert_eq!(e.message, "draft: missing value");

        let e = conditions("      draft: no\n");
        assert_eq!(e.message, "draft: `no` is not `true` or `false`");

        let config =
            RepoConfig::parse(&queue("    queue_conditions:\n      labels: [no, on]\n")).unwrap();
        assert_eq!(
            config.queue_conditions("main").0[1].to_string(),
            "labels: on"
        );

        let e = conditions("      labels: [a]\n      labels: [b]\n");
        assert_eq!(e.position.map(|p| p.line), Some(6));

        let e = conditions("      labels:\n        any:\n          - [a]\n");
        assert_eq!(e.position, Some(at(7, 13)));
        assert_eq!(e.message, "expected a value or a list of values");

        let e = conditions("      - draft: false\n");
        assert_eq!(e.message, "expected a map of conditions");

        let e = conditions("      any:\n        labels: [a]\n");
        assert_eq!(e.message, "`any` takes a list of condition maps");

        let e = conditions("      any:\n        - or: x\n");
        assert_eq!(e.position, Some(at(6, 11)));
        assert_eq!(e.message, "unknown attribute `or`");

        let e = conditions("      checks: [lint]\n");
        assert!(
            e.message.starts_with("expected a map of check states"),
            "{e}"
        );
    }

    #[test]
    fn refuses_empty_maps_and_a_second_form_of_a_key() {
        let conditions = |body: &str| error(&queue(&format!("    queue_conditions:\n{body}")));

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
            let e = conditions(body);
            assert!(e.message.contains(message), "{body}: {e}");
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
        let e = error(&queue(
            "    merge_conditions:\n      checks:\n        success: [ci]\n      labels: [ready]\n",
        ));
        assert_eq!(e.position, Some(at(7, 7)));

        let nested = "    merge_conditions:\n      checks:\n        success:\n          all: [a]\n          any: [b]\n        pending: [c]\n";
        let e = error(&queue(nested));
        assert_eq!(e.position, Some(at(8, 11)));
        assert!(
            e.message.ends_with("not `checks.success.any: b`"),
            "{}",
            e.message
        );

        let e = error(&queue(&nested.replace("          any: [b]\n", "")));
        assert_eq!(e.position, Some(at(8, 9)));
        assert!(
            e.message.ends_with("not `checks.pending: c`"),
            "{}",
            e.message
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
            let e = error(&yaml);
            assert_eq!(e.position, Some(position), "{yaml}");
            assert_eq!(e.message, message, "{yaml}");
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
        let e = error(&queue("    batching: { size: 50 }\n"));
        assert_eq!(e.position, Some(at(4, 23)));
        assert_eq!(e.message, "batching.size is 50, out of range 2–20");

        let e = error(&queue("    window: { floor: 30, ceiling: 20 }\n"));
        assert_eq!(e.position, Some(at(4, 22)));

        let e = error(&queue("    trigger: inplace\n"));
        assert_eq!(e.position, Some(at(4, 14)));

        let e = error(&queue("    strategy: fast\n"));
        assert_eq!(e.kind, ConfigErrorKind::Parsing);
        assert_eq!(e.position.map(|p| p.line), Some(4));

        let e = error(&queue("    colour: blue\n"));
        assert_eq!(e.position.map(|p| p.line), Some(4));
        assert!(e.message.contains("colour"), "{e}");

        let e = error(&queue("    required_statuses: [ci, ci]\n"));
        assert_eq!(e.kind, ConfigErrorKind::BadRequiredStatuses);
        assert_eq!(
            e.job_error(),
            cihub_domain::JobError::ConfigBadRequiredStatuses
        );
    }

    #[test]
    fn checks_the_version_first() {
        let e = error("queues: []\n");
        assert_eq!(e.kind, ConfigErrorKind::BadVersion);
        let e = error("version: 2\nqueues:\n  - colour: blue\n");
        assert_eq!(e.kind, ConfigErrorKind::BadVersion);
        assert_eq!(e.position, Some(at(1, 10)));
        assert_eq!(error("").kind, ConfigErrorKind::BadVersion);
    }

    #[test]
    fn checks_queues_and_defaults() {
        let e = error("version: 1\nqueues:\n  - strategy: batched\n");
        assert_eq!(e.message, "a queue needs a `branch`");
        assert_eq!(e.position.map(|p| p.line), Some(3));

        let e = error(&queue("    name: hotfix\n"));
        assert!(e.message.contains("name"), "{e}");
        assert_eq!(e.position.map(|p| p.line), Some(4));

        let e = error(&format!("{}  - branch: main\n", queue("")));
        assert_eq!(e.message, "the queue of `main` is defined twice");
        assert_eq!(e.position, Some(at(4, 13)));

        for (defaults, key, position) in [
            ("  branch: main\n", "branch", at(3, 11)),
            ("  profiles:\n    - name: a\n", "profiles", at(4, 5)),
        ] {
            let e = error(&format!("version: 1\ndefaults:\n{defaults}"));
            assert_eq!(e.message, format!("`{key}` is not allowed in `defaults`"));
            assert_eq!(e.position, Some(position), "{key}");
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

        let e = error("version: 1\nqueues:\n  - branch: \"release/[\"\n");
        assert!(e.message.starts_with("branch `release/[`"), "{e}");
        assert_eq!(e.position.map(|p| p.line), Some(3));
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

        let e = error(&profiles("        merge_method: fast_forward\n"));
        assert!(
            e.message.contains("cannot set merge_method `fast_forward`"),
            "{e}"
        );
        assert_eq!(e.position, Some(at(6, 23)));

        let e = error(&profiles("        strategy: sequential\n"));
        assert!(e.message.contains("strategy"), "{e}");

        let e = error(&profiles("        batching: { size: 50 }\n"));
        assert_eq!(e.message, "batching.size is 50, out of range 2–20");

        let e = error(&format!("{}      - name: a\n", profiles("")));
        assert_eq!(e.message, "the profile `a` is defined twice");
        assert_eq!(e.position.map(|p| p.line), Some(6));
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

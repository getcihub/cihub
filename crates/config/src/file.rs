//! The `.cihub.yaml` model, read with the position of each value.

use std::collections::HashSet;
use std::fmt;

use cihub_conditions::{Conditions, Expr, Glob};
use cihub_domain::{
    BranchProtectionInjection, DirectMergeMode, MergeCommitTitle, MergeMethod, OptimizationMode,
    Priority, QueueMode, Strategy, Trigger, UpdateMethod,
};
use serde::Deserialize;
use serde::de::{self, Deserializer, IgnoredAny, MapAccess, Visitor};
use serde_saphyr::{DefaultMessageFormatter, MessageFormatter, Spanned};

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
        let file: RawFile = serde_saphyr::from_str(text).map_err(|e| yaml_error(&e))?;
        let (defaults, queue_conditions, merge_conditions) = match file.defaults {
            Some(raw) => defaults(raw, text)?,
            None => Default::default(),
        };

        QueueSettings::resolve(&Layer::default(), &defaults)?;
        let mut branches = HashSet::new();
        let mut queues = Vec::with_capacity(file.queues.len());

        for raw in file.queues {
            let (queue, branch_at) = queue(raw, text)?;
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
            .map(|rule| priority_rule(rule, text))
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
    let probe: Option<Probe> = serde_saphyr::from_str(text).map_err(|e| yaml_error(&e))?;
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
    queue_conditions: Value<Vec<Spanned<RawExpr>>>,
    merge_conditions: Value<Vec<Spanned<RawExpr>>>,
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
    #[serde(default)]
    conditions: Vec<Spanned<RawExpr>>,
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
    conditions: Vec<Spanned<RawExpr>>,
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
    text: &str,
) -> Result<(QueueConfig, Option<Position>), ConfigError> {
    let queue_at = position(&raw);
    let mut raw = raw.value;
    let (branch_at, branch) = raw
        .branch
        .take()
        .map(|v| (position(&v), v.value))
        .ok_or_else(|| ConfigError::parsing(queue_at, "a queue needs a `branch`"))?;
    let pattern = branch_pattern(&branch, branch_at)?;
    let list = |value: Value<Vec<Spanned<RawExpr>>>| value.map(|v| v.value).unwrap_or_default();
    let profiles = raw
        .profiles
        .take()
        .map(|v| v.value)
        .unwrap_or_default()
        .into_iter()
        .map(|raw| profile(raw, text))
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
        queue_conditions: conditions(&list(raw.queue_conditions.take()), text)?,
        merge_conditions: merge_conditions(&list(raw.merge_conditions.take()), text)?,
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
    text: &str,
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
        conditions: conditions(&raw.conditions, text)?,
        settings,
    };

    Ok((profile, at))
}

/// The settings and the conditions of `defaults`.
fn defaults(raw: RawQueue, text: &str) -> Result<(Layer, Conditions, Conditions), ConfigError> {
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
    let list = |value: Value<Vec<Spanned<RawExpr>>>| value.map(|v| v.value).unwrap_or_default();
    let queue_conditions = conditions(&list(raw.queue_conditions.take()), text)?;
    let merge_conditions = merge_conditions(&list(raw.merge_conditions.take()), text)?;

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

fn priority_rule(raw: RawPriorityRule, text: &str) -> Result<PriorityRule, ConfigError> {
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
        conditions: conditions(&raw.conditions, text)?,
        priority,
    })
}

/// A condition string, or a block with one key.
enum RawExpr {
    Condition(String),
    Block(fn(Vec<Expr>) -> Expr, Vec<Spanned<RawExpr>>),
}

fn conditions(list: &[Spanned<RawExpr>], text: &str) -> Result<Conditions, ConfigError> {
    exprs(list, text).map(Conditions)
}

/// `merge_conditions` accept only `check-success = <name>` at the first
/// level for now.
fn merge_conditions(list: &[Spanned<RawExpr>], text: &str) -> Result<Conditions, ConfigError> {
    let conditions = conditions(list, text)?;
    for (node, expr) in list.iter().zip(&conditions.0) {
        let checked = matches!(expr, Expr::Condition(c) if c.required_check().is_some());
        if !checked {
            return Err(ConfigError::parsing(
                position(node),
                format!("merge_conditions accept only `check-success = <name>`, not `{expr}`"),
            ));
        }
    }
    Ok(conditions)
}

fn exprs(list: &[Spanned<RawExpr>], text: &str) -> Result<Vec<Expr>, ConfigError> {
    list.iter().map(|node| expr(node, text)).collect()
}

fn expr(node: &Spanned<RawExpr>, text: &str) -> Result<Expr, ConfigError> {
    match &node.value {
        RawExpr::Condition(source) => source.parse().map(Expr::Condition).map_err(|e| {
            let start = Position::of(&node.referenced);
            let quote = node
                .referenced
                .span()
                .byte_offset()
                .and_then(|offset| text.get(usize::try_from(offset).ok()?..))
                .is_some_and(|rest| rest.starts_with(['"', '\'']));
            let column_in_condition = u64::try_from(e.column).unwrap_or(u64::MAX);
            ConfigError::parsing(
                Some(Position {
                    line: start.line,
                    column: start.column + column_in_condition - 1 + u64::from(quote),
                }),
                e.kind.to_string(),
            )
        }),
        RawExpr::Block(block, list) => exprs(list, text).map(block),
    }
}

impl<'de> Deserialize<'de> for RawExpr {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(RawExprVisitor)
    }
}

struct RawExprVisitor;

impl<'de> Visitor<'de> for RawExprVisitor {
    type Value = RawExpr;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a condition, or a block with one key `and`, `or` or `not`")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<RawExpr, E> {
        Ok(RawExpr::Condition(v.to_owned()))
    }

    // `- #label = 0` is a YAML comment, so the element is null.
    fn visit_unit<E: de::Error>(self) -> Result<RawExpr, E> {
        Err(E::custom(
            "empty condition: quote a condition that starts with `#`",
        ))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<RawExpr, A::Error> {
        let Some(key) = map.next_key::<String>()? else {
            return Err(de::Error::invalid_length(0, &self));
        };

        let block: fn(_) -> _ = match key.as_str() {
            "and" => Expr::And,
            "or" => Expr::Or,
            "not" => Expr::Not,
            _ => return Err(de::Error::unknown_field(&key, &["and", "or", "not"])),
        };
        let list = map.next_value()?;

        if map.next_key::<String>()?.is_some() {
            return Err(de::Error::custom("a condition block has exactly one key"));
        }

        Ok(RawExpr::Block(block, list))
    }
}

#[cfg(test)]
mod tests {
    use cihub_conditions::Context;

    use super::*;
    use crate::Source;

    /// The example of the spec, section Configuration.
    const SPEC_EXAMPLE: &str = r#"version: 1
queues:
  - branch: main
    strategy: batched                     # sequential | pipelined | batched | parallel
    merge_method: squash                  # squash | merge_commit | rebase | fast_forward
    trigger: draft_pr                     # draft_pr | push
    queue_conditions:                     # admission : la PR elle-meme
      - review-decision = approved
      - -draft
      - -label = do-not-merge
      - check-success = lint
    merge_conditions:                     # sur le commit speculatif
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
    conditions: [ label = urgent ]
    priority: urgent                      # urgent | high | medium | low | 0-255
  - name: dependabot
    conditions:
      - author = dependabot[bot]            # une liste en ligne coupe le YAML sur les crochets
    priority: low
defaults:                                 # tout reglage d'une file ; chaque file peut le remplacer
  label_commands: { enqueue_label: ready-to-merge }
  merge_commit_title: pr_title            # pr_title | pr_title_and_number
  queue_conditions:                       # every queue adds its own after them
    - -label = do-not-merge
"#;

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
        assert_eq!(settings.batching_size.at, Some(at(16, 23)));
        assert_eq!(settings.batching_enabled.source, Source::Strategy);
        assert_eq!(settings.enqueue_label.value, "ready-to-merge");
        assert_eq!(settings.enqueue_label.source, Source::File);

        let ctx = Context {
            review_decision: cihub_domain::ReviewDecision::Approved,
            ..Context::default()
        };

        assert_eq!(main.queue_conditions.eval(&ctx), Ok(false));
    }

    #[test]
    fn merge_conditions_accept_only_required_checks() {
        let config = RepoConfig::parse(&queue(
            "    merge_conditions:\n      - check-success = build\n      - check-success = \"e2e tests\"\n",
        ))
        .unwrap();
        assert_eq!(config.merge_checks("main"), ["build", "e2e tests"]);
        assert_eq!(config.merge_checks("other"), [] as [String; 0]);
        for (body, shown) in [
            ("      - label = ready\n", "label = ready"),
            ("      - -check-success = build\n", "-check-success = build"),
            (
                "      - check-success ~= ^build\n",
                "check-success ~= ^build",
            ),
            (
                "      - or:\n          - check-success = a\n",
                "or: [check-success = a]",
            ),
        ] {
            let error = error(&queue(&format!("    merge_conditions:\n{body}")));
            assert_eq!(error.position, Some(at(5, 9)), "{body}");
            assert_eq!(
                error.message,
                format!("merge_conditions accept only `check-success = <name>`, not `{shown}`")
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
    fn reports_condition_errors_at_their_character() {
        let e = error(&queue(
            "    queue_conditions:\n      - -draft\n      - lable = x\n",
        ));
        assert_eq!(e.position, Some(at(6, 9)));
        assert_eq!(e.message, "unknown attribute `lable`");

        let e = error(&queue("    queue_conditions:\n      - label ~= (\n"));
        assert_eq!(e.position, Some(at(5, 18)));

        let e = error(&queue("    queue_conditions:\n      - \"#label >= x\"\n"));
        assert_eq!(e.position, Some(at(5, 20)));
        assert_eq!(e.message, "`x` is not an integer");

        let e = error(&queue(
            "    queue_conditions:\n      - or:\n          - label = a\n          - draft = true\n",
        ));

        assert_eq!(e.position, Some(at(7, 19)));
    }

    #[test]
    fn explains_a_condition_that_yaml_reads_as_a_comment() {
        let e = error(&queue("    queue_conditions:\n      - #label = 0\n"));
        assert!(
            e.message.contains("quote a condition that starts with `#`"),
            "{e}"
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
      - check-success = ci
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
    - -draft
  merge_conditions:
    - check-success = build
queues:
  - branch: main
    queue_conditions:
      - label = ready
    merge_conditions:
      - check-success = e2e
      - check-success = build
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

        assert_eq!(shown("main"), ["-draft", "label = ready"]);
        assert_eq!(shown("dev"), ["-draft"]);
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
          - author = dependabot[bot]
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
                "version: 1\npriority_rules:\n  - name: r\n    conditions: [draft]\n    priority: {priority}\n"
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

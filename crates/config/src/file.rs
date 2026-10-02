//! The `.cihub.yaml` model, read with the position of each value.

use std::collections::HashSet;
use std::fmt;

use cihub_conditions::{Conditions, Expr};
use cihub_domain::{
    BranchProtectionInjection, DirectMergeMode, MergeCommitTitle, MergeMethod, OptimizationMode,
    Priority, QueueMode, Strategy, Trigger, UpdateMethod,
};
use serde::Deserialize;
use serde::de::{self, Deserializer, IgnoredAny, MapAccess, Visitor};
use serde_saphyr::{DefaultMessageFormatter, MessageFormatter, Spanned};

use crate::settings::in_range;
use crate::{ConfigError, ConfigErrorKind, Layer, Position, QueueSettings, Setting};

/// Names of the configuration file at the root of the repository.
pub const FILE_NAMES: [&str; 2] = [".cihub.yaml", ".cihub.yml"];

/// Chooses the configuration file among the entries of the repository root.
///
/// # Errors
///
/// Both names exist: neither one wins.
pub fn find_config_file<'a>(
    root: impl IntoIterator<Item = &'a str>,
) -> Result<Option<&'static str>, ConfigError> {
    let root: HashSet<_> = root.into_iter().collect();
    match FILE_NAMES.map(|name| root.contains(name)) {
        [true, true] => Err(ConfigError::parsing(
            None,
            "both .cihub.yaml and .cihub.yml exist; keep one of them",
        )),
        [true, false] => Ok(Some(FILE_NAMES[0])),
        [false, true] => Ok(Some(FILE_NAMES[1])),
        [false, false] => Ok(None),
    }
}

#[derive(Clone, Debug)]
pub struct RepoConfig {
    /// Settings for every queue; each queue can replace them.
    pub defaults: Layer,
    /// In file order, which is the routing order between queues of a branch.
    pub queues: Vec<QueueConfig>,
    pub priority_rules: Vec<PriorityRule>,
}

#[derive(Clone, Debug)]
pub struct QueueConfig {
    pub name: String,
    pub branch: String,
    pub queue_conditions: Conditions,
    pub merge_conditions: Conditions,
    pub settings: Layer,
}

#[derive(Clone, Debug)]
pub struct PriorityRule {
    pub name: String,
    pub conditions: Conditions,
    pub priority: Priority,
}

impl RepoConfig {
    /// Parses `.cihub.yaml`. Each queue is also resolved without dashboard
    /// settings, so that values that do not go together fail here, with
    /// their position.
    ///
    /// # Errors
    ///
    /// The first error in the file.
    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        check_version(text)?;
        let file: RawFile = serde_saphyr::from_str(text).map_err(|e| yaml_error(&e))?;
        let defaults = file
            .defaults
            .map(defaults_layer)
            .transpose()?
            .unwrap_or_default();
        let mut names = HashSet::new();
        let mut queues = Vec::with_capacity(file.queues.len());
        for raw in file.queues {
            let (queue, name_at) = queue(raw, text)?;
            if !names.insert(queue.name.clone()) {
                return Err(ConfigError::parsing(
                    name_at,
                    format!("queue `{}` is defined twice", queue.name),
                ));
            }
            QueueSettings::resolve(&Layer::default(), &queue.settings, &defaults)?;
            queues.push(queue);
        }
        let priority_rules = file
            .priority_rules
            .into_iter()
            .map(|rule| priority_rule(rule, text))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            defaults,
            queues,
            priority_rules,
        })
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
    // The formatter leaves control characters as they are; the message ends
    // up in PR comments and logs.
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

/// A queue, or `defaults` without `name`, `branch` and conditions.
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawQueue {
    name: Value<String>,
    branch: Value<String>,
    queue_conditions: Value<Vec<Spanned<RawExpr>>>,
    merge_conditions: Value<Vec<Spanned<RawExpr>>>,
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

/// The queue and the position of its name.
fn queue(
    raw: Spanned<RawQueue>,
    text: &str,
) -> Result<(QueueConfig, Option<Position>), ConfigError> {
    let queue_at = position(&raw);
    let mut raw = raw.value;
    let required = |value: Value<String>, key: &str| {
        value
            .map(|v| (position(&v), v.value))
            .ok_or_else(|| ConfigError::parsing(queue_at, format!("a queue needs a `{key}`")))
    };
    let (name_at, name) = required(raw.name.take(), "name")?;
    let (_, branch) = required(raw.branch.take(), "branch")?;
    let list = |value: Value<Vec<Spanned<RawExpr>>>| value.map(|v| v.value).unwrap_or_default();
    let queue = QueueConfig {
        name,
        branch,
        queue_conditions: conditions(&list(raw.queue_conditions.take()), text)?,
        merge_conditions: conditions(&list(raw.merge_conditions.take()), text)?,
        settings: layer(raw)?,
    };
    Ok((queue, name_at))
}

fn defaults_layer(raw: RawQueue) -> Result<Layer, ConfigError> {
    let queue_only = [
        ("name", raw.name.as_ref().and_then(position)),
        ("branch", raw.branch.as_ref().and_then(position)),
        (
            "queue_conditions",
            raw.queue_conditions.as_ref().and_then(position),
        ),
        (
            "merge_conditions",
            raw.merge_conditions.as_ref().and_then(position),
        ),
    ];
    if let Some((key, at)) = queue_only.into_iter().find(|(_, at)| at.is_some()) {
        return Err(ConfigError::parsing(
            at,
            format!("`{key}` is not allowed in `defaults`"),
        ));
    }
    layer(raw)
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

fn exprs(list: &[Spanned<RawExpr>], text: &str) -> Result<Vec<Expr>, ConfigError> {
    list.iter().map(|node| expr(node, text)).collect()
}

fn expr(node: &Spanned<RawExpr>, text: &str) -> Result<Expr, ConfigError> {
    match &node.value {
        RawExpr::Condition(source) => source.parse().map(Expr::Condition).map_err(|e| {
            let start = Position::of(&node.referenced);
            // The scalar starts at its quote, if any.
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
    const SPEC_EXAMPLE: &str = r"version: 1
queues:
  - name: main
    branch: main
    strategy: batched                     # sequential | pipelined | batched | parallel
    merge_method: squash                  # squash | merge_commit | rebase | fast_forward
    trigger: draft_pr                     # draft_pr | push
    queue_conditions:                     # admission : la PR elle-meme
      - github-review-decision = approved
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
  - name: hotfix
    branch: main
    strategy: sequential
    merge_method: fast_forward
    queue_conditions:
      - label = hotfix
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
";

    fn error(yaml: &str) -> ConfigError {
        RepoConfig::parse(yaml).unwrap_err()
    }

    fn at(line: u64, column: u64) -> Position {
        Position { line, column }
    }

    fn queue(body: &str) -> String {
        format!("version: 1\nqueues:\n  - name: main\n    branch: main\n{body}")
    }

    #[test]
    fn parses_the_spec_example() {
        let config = RepoConfig::parse(SPEC_EXAMPLE).unwrap();
        let [main, hotfix] = &config.queues[..] else {
            panic!("expected two queues");
        };
        assert_eq!(
            (main.name.as_str(), hotfix.name.as_str()),
            ("main", "hotfix")
        );
        assert_eq!(main.queue_conditions.0.len(), 4);
        assert_eq!(
            config
                .priority_rules
                .iter()
                .map(|rule| rule.priority)
                .collect::<Vec<_>>(),
            [Priority::URGENT, Priority::LOW]
        );

        let settings =
            QueueSettings::resolve(&Layer::default(), &main.settings, &config.defaults).unwrap();
        assert_eq!(settings.batching_size.value, 8);
        assert_eq!(settings.batching_size.source, Source::File);
        assert_eq!(settings.batching_size.at, Some(at(17, 23)));
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
    fn testing_concurrency_sets_the_whole_window() {
        let config = RepoConfig::parse(&queue("    testing_concurrency: 3\n")).unwrap();
        let settings = QueueSettings::resolve(
            &Layer::default(),
            &config.queues[0].settings,
            &config.defaults,
        )
        .unwrap();
        assert_eq!(
            (settings.window_floor.value, settings.window_ceiling.value),
            (3, 3)
        );
        assert_eq!(settings.bisection_concurrency.value, 3);

        let e = error(&queue(
            "    window: { floor: 2 }\n    testing_concurrency: 3\n",
        ));
        assert_eq!(e.position, Some(at(6, 26)));
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
        assert_eq!(e.position, Some(at(7, 9)));
        assert_eq!(e.message, "unknown attribute `lable`");

        let e = error(&queue("    queue_conditions:\n      - label ~= (\n"));
        assert_eq!(e.position, Some(at(6, 18)));

        let e = error(&queue("    queue_conditions:\n      - \"#label >= x\"\n"));
        assert_eq!(e.position, Some(at(6, 20)));
        assert_eq!(e.message, "`x` is not an integer");

        let e = error(&queue(
            "    queue_conditions:\n      - or:\n          - label = a\n          - draft = true\n",
        ));
        assert_eq!(e.position, Some(at(8, 19)));
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
        assert_eq!(e.position, Some(at(5, 23)));
        assert_eq!(e.message, "batching.size is 50, out of range 2–20");

        let e = error(&queue("    window: { floor: 30, ceiling: 20 }\n"));
        assert_eq!(e.position, Some(at(5, 22)));

        let e = error(&queue("    trigger: inplace\n"));
        assert_eq!(e.position, Some(at(5, 14)));

        let e = error(&queue("    strategy: fast\n"));
        assert_eq!(e.kind, ConfigErrorKind::Parsing);
        assert_eq!(e.position.map(|p| p.line), Some(5));

        let e = error(&queue("    colour: blue\n"));
        assert_eq!(e.position.map(|p| p.line), Some(5));
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
        let e = error("version: 1\nqueues:\n  - branch: main\n");
        assert_eq!(e.message, "a queue needs a `name`");
        assert_eq!(e.position.map(|p| p.line), Some(3));

        let e = error(&format!("{}  - name: main\n    branch: dev\n", queue("")));
        assert_eq!(e.message, "queue `main` is defined twice");
        assert_eq!(e.position, Some(at(5, 11)));

        let e = error("version: 1\ndefaults:\n  branch: main\n");
        assert_eq!(e.message, "`branch` is not allowed in `defaults`");
        assert_eq!(e.position, Some(at(3, 11)));
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
        assert_eq!(find_config_file(["README.md"]), Ok(None));
        assert!(find_config_file([".cihub.yaml", ".cihub.yml"]).is_err());
    }
}

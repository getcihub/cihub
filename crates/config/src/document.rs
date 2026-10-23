//! The document of a file: what it sets, in the words of the file, and the
//! YAML of a document. Reading the YAML of a document gives the same
//! document; the comments and the anchors of a file are not in its document.

use std::fmt::Write as _;

use cihub_api_types::{
    ConditionBlock, ConditionDocument, ConfigDocument, DefaultsDocument, DiffLine, LineChange,
    PriorityRuleDocument, ProfileDocument, QueueDocument, RequiredStatusesDocument,
    SettingsDocument,
};
use cihub_conditions::{Attribute, Expr, Operator};
use cihub_domain::{
    BranchProtectionInjection, DirectMergeMode, MergeCommitTitle, MergeMethod, OptimizationMode,
    QueueMode, Strategy, Trigger, UpdateMethod,
};

use crate::file::merge_checks_of;
use crate::{ConfigError, Layer, RepoConfig, Setting};

/// The first line of a file that CIHub writes: the schema of this version,
/// which an editor reads.
const SCHEMA_LINE: &str = concat!(
    "# yaml-language-server: $schema=https://raw.githubusercontent.com/getcihub/cihub/v",
    env!("CARGO_PKG_VERSION"),
    "/schema/config/v1.json"
);

impl RepoConfig {
    /// What the file sets, in its order.
    pub fn document(&self) -> ConfigDocument {
        ConfigDocument {
            defaults: DefaultsDocument {
                settings: settings_document(&self.defaults),
                queue_conditions: conditions_document(&self.queue_conditions.0),
                merge_checks: merge_checks_of(&self.merge_conditions),
            },
            queues: self
                .queues
                .iter()
                .map(|queue| QueueDocument {
                    branch: queue.branch.clone(),
                    settings: settings_document(&queue.settings),
                    queue_conditions: conditions_document(&queue.queue_conditions.0),
                    merge_checks: merge_checks_of(&queue.merge_conditions),
                    profiles: queue
                        .profiles
                        .iter()
                        .map(|profile| ProfileDocument {
                            name: profile.name.clone(),
                            conditions: conditions_document(&profile.conditions.0),
                            settings: settings_document(&profile.settings),
                        })
                        .collect(),
                })
                .collect(),
            priority_rules: self
                .priority_rules
                .iter()
                .map(|rule| PriorityRuleDocument {
                    name: rule.name.clone(),
                    conditions: conditions_document(&rule.conditions.0),
                    priority: rule
                        .priority
                        .name()
                        .map_or_else(|| rule.priority.0.to_string(), ToOwned::to_owned),
                })
                .collect(),
        }
    }
}

fn value<T: Clone>(setting: Option<&Setting<T>>) -> Option<T> {
    setting.map(|setting| setting.value.clone())
}

/// The settings that `layer` sets. `testing_concurrency` sets both bounds
/// of the window from one place of the file, so it shows as itself.
fn settings_document(layer: &Layer) -> SettingsDocument {
    let (floor, ceiling) = (layer.window_floor.as_ref(), layer.window_ceiling.as_ref());
    let is_fixed = matches!((floor, ceiling), (Some(floor), Some(ceiling)) if floor == ceiling);
    let (window_floor, window_ceiling, testing_concurrency) = if is_fixed {
        (None, None, value(floor))
    } else {
        (value(floor), value(ceiling), None)
    };

    SettingsDocument {
        strategy: value(layer.strategy.as_ref()),
        mode: value(layer.mode.as_ref()),
        window_floor,
        window_ceiling,
        testing_concurrency,
        bisection_concurrency: value(layer.bisection_concurrency.as_ref()),
        testing_timeout_minutes: value(layer.testing_timeout_minutes.as_ref()),
        not_ready_timeout_hours: value(layer.not_ready_timeout_hours.as_ref()),
        merge_method: value(layer.merge_method.as_ref()),
        merge_commit_title: value(layer.merge_commit_title.as_ref()),
        trigger: value(layer.trigger.as_ref()),
        update_method: value(layer.update_method.as_ref()),
        batching_enabled: value(layer.batching_enabled.as_ref()),
        batching_size: value(layer.batching_size.as_ref()),
        batching_max_wait_minutes: value(layer.batching_max_wait_minutes.as_ref()),
        failure_budget_retries: value(layer.failure_budget_retries.as_ref()),
        optimistic_merging: value(layer.optimistic_merging.as_ref()),
        pending_failure_depth: value(layer.pending_failure_depth.as_ref()),
        direct_merge_mode: value(layer.direct_merge_mode.as_ref()),
        optimization_mode: value(layer.optimization_mode.as_ref()),
        comments_enabled: value(layer.comments_enabled.as_ref()),
        commands_enabled: value(layer.commands_enabled.as_ref()),
        status_check_enabled: value(layer.status_check_enabled.as_ref()),
        label_commands_enabled: value(layer.label_commands_enabled.as_ref()),
        enqueue_label: value(layer.enqueue_label.as_ref()),
        state_labels_enabled: value(layer.state_labels_enabled.as_ref()),
        resubmit_on_push: value(layer.resubmit_on_push.as_ref()),
        keep_test_branches_minutes: value(layer.keep_test_branches_minutes.as_ref()),
        branch_prefix: value(layer.branch_prefix.as_ref()),
        branch_protection_injection: value(layer.branch_protection_injection.as_ref()),
        required_statuses: layer
            .required_statuses
            .as_ref()
            .map(|setting| match &setting.value {
                None => RequiredStatusesDocument::BranchRules,
                Some(names) => RequiredStatusesDocument::Checks {
                    names: names.clone(),
                },
            }),
    }
}

/// The conditions of `exprs`. A key of the file with several values builds
/// one condition per value: they come back together, as the file writes
/// them.
fn conditions_document(exprs: &[Expr]) -> Vec<ConditionDocument> {
    let mut documents: Vec<ConditionDocument> = Vec::new();
    for expr in exprs {
        let document = condition_document(expr);
        if let (
            Some(ConditionDocument::Condition {
                attribute,
                operator,
                values,
            }),
            ConditionDocument::Condition {
                attribute: next_attribute,
                operator: next_operator,
                values: next_values,
            },
        ) = (documents.last_mut(), &document)
            && (attribute == next_attribute && operator == next_operator)
            && holds_per_value(attribute, operator.as_deref())
        {
            values.extend(next_values.iter().cloned());
            continue;
        }
        documents.push(document);
    }
    documents
}

/// Each value of the condition holds on its own: two conditions of the path
/// are one with both values.
fn holds_per_value(attribute: &str, operator: Option<&str>) -> bool {
    let operator = match operator {
        Some(path) => Operator::from_path(path),
        None => Some(Operator::Is),
    };
    match (Attribute::from_name(attribute), operator) {
        (Some(attribute), Some(operator)) => operator.holds_per_value(attribute),
        _ => false,
    }
}

fn condition_document(expr: &Expr) -> ConditionDocument {
    let (block, list) = match expr {
        Expr::Condition(condition) => {
            return ConditionDocument::Condition {
                attribute: condition.attribute().name().to_owned(),
                operator: condition.operator().key().map(ToOwned::to_owned),
                values: condition.values().to_vec(),
            };
        }
        Expr::All(list) => (ConditionBlock::All, list),
        Expr::Any(list) => (ConditionBlock::Any, list),
        Expr::None(list) => (ConditionBlock::None, list),
    };

    ConditionDocument::Block {
        block,
        conditions: conditions_document(list),
    }
}

/// A node of the YAML that CIHub writes.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Node {
    Scalar(String),
    List(Vec<Self>),
    Map(Vec<(String, Self)>),
}

/// The YAML of `document`, in the style of the documentation: each block on
/// its lines, the keys in the order of the schema.
///
/// # Errors
///
/// A condition names an attribute or an operator that the format does not
/// have: its key would not read back as the condition.
pub fn write_document(document: &ConfigDocument) -> Result<String, ConfigError> {
    check_conditions(document)?;

    let mut root = vec![("version".to_owned(), Node::Scalar("1".to_owned()))];
    if !document.queues.is_empty() {
        let queues = document.queues.iter().map(queue_node).collect();
        root.push(("queues".to_owned(), Node::List(queues)));
    }
    if !document.priority_rules.is_empty() {
        let rules = document.priority_rules.iter().map(rule_node).collect();
        root.push(("priority_rules".to_owned(), Node::List(rules)));
    }
    let defaults = defaults_entries(&document.defaults);
    if !defaults.is_empty() {
        root.push(("defaults".to_owned(), Node::Map(defaults)));
    }

    let mut lines = vec![SCHEMA_LINE.to_owned()];
    write_map(&root, 0, &mut lines);
    let mut text = lines.join("\n");
    text.push('\n');
    Ok(text)
}

/// Each condition of `document` names an attribute and an operator of the
/// format: the writer makes keys of them.
fn check_conditions(document: &ConfigDocument) -> Result<(), ConfigError> {
    let queues = document.queues.iter();
    let lists = std::iter::once(&document.defaults.queue_conditions)
        .chain(queues.clone().map(|queue| &queue.queue_conditions))
        .chain(
            queues
                .flat_map(|queue| &queue.profiles)
                .map(|profile| &profile.conditions),
        )
        .chain(document.priority_rules.iter().map(|rule| &rule.conditions));
    for conditions in lists {
        check_list(conditions)?;
    }
    Ok(())
}

fn check_list(conditions: &[ConditionDocument]) -> Result<(), ConfigError> {
    for condition in conditions {
        match condition {
            ConditionDocument::Condition {
                attribute,
                operator,
                ..
            } => {
                let known = Attribute::from_name(attribute).is_some()
                    && operator
                        .as_deref()
                        .is_none_or(|path| Operator::from_path(path).is_some());
                if !known {
                    let path = operator.as_ref().map_or_else(
                        || attribute.clone(),
                        |operator| format!("{attribute}.{operator}"),
                    );
                    return Err(ConfigError::parsing(
                        None,
                        format!("unknown condition `{path}`"),
                    ));
                }
            }
            ConditionDocument::Block { conditions, .. } => check_list(conditions)?,
        }
    }
    Ok(())
}

fn queue_node(queue: &QueueDocument) -> Node {
    let mut entries = vec![(
        "branch".to_owned(),
        Node::Scalar(text_scalar(&queue.branch)),
    )];
    entries.extend(settings_entries(&queue.settings));
    entries.extend(conditions_entries(
        &queue.queue_conditions,
        &queue.merge_checks,
    ));
    if !queue.profiles.is_empty() {
        let profiles = queue.profiles.iter().map(profile_node).collect();
        entries.push(("profiles".to_owned(), Node::List(profiles)));
    }
    Node::Map(entries)
}

fn profile_node(profile: &ProfileDocument) -> Node {
    let mut entries = vec![("name".to_owned(), Node::Scalar(text_scalar(&profile.name)))];
    if !profile.conditions.is_empty() {
        entries.push((
            "conditions".to_owned(),
            Node::Map(condition_map(&profile.conditions)),
        ));
    }
    entries.extend(settings_entries(&profile.settings));
    Node::Map(entries)
}

fn rule_node(rule: &PriorityRuleDocument) -> Node {
    Node::Map(vec![
        ("name".to_owned(), Node::Scalar(text_scalar(&rule.name))),
        (
            "conditions".to_owned(),
            Node::Map(condition_map(&rule.conditions)),
        ),
        (
            "priority".to_owned(),
            Node::Scalar(priority_scalar(&rule.priority)),
        ),
    ])
}

fn defaults_entries(defaults: &DefaultsDocument) -> Vec<(String, Node)> {
    let mut entries = settings_entries(&defaults.settings);
    entries.extend(conditions_entries(
        &defaults.queue_conditions,
        &defaults.merge_checks,
    ));
    entries
}

fn conditions_entries(
    queue_conditions: &[ConditionDocument],
    merge_checks: &[String],
) -> Vec<(String, Node)> {
    let mut entries = Vec::new();
    if !queue_conditions.is_empty() {
        entries.push((
            "queue_conditions".to_owned(),
            Node::Map(condition_map(queue_conditions)),
        ));
    }
    if !merge_checks.is_empty() {
        let checks = Node::Map(vec![("success".to_owned(), values_node(merge_checks))]);
        entries.push((
            "merge_conditions".to_owned(),
            Node::Map(vec![("checks".to_owned(), checks)]),
        ));
    }
    entries
}

/// A setting of one key, or the keys of a map (`window`, `batching`), with
/// their values when they are set.
type SettingKeys = (&'static str, Vec<(&'static str, Option<String>)>);

fn one(value: Option<String>) -> Vec<(&'static str, Option<String>)> {
    vec![("", value)]
}

/// The keys of the settings of `s`, in the order of the schema.
fn setting_keys(s: &SettingsDocument) -> Vec<SettingKeys> {
    let number = |value: Option<u32>| value.map(|value| value.to_string());
    let flag = |value: Option<bool>| value.map(|value| value.to_string());
    let name = |value: Option<&str>| value.map(ToOwned::to_owned);
    let text = |value: &Option<String>| value.as_deref().map(text_scalar);
    vec![
        ("strategy", one(name(s.strategy.map(Strategy::as_str)))),
        ("mode", one(name(s.mode.map(QueueMode::as_str)))),
        (
            "window",
            vec![
                ("floor", number(s.window_floor)),
                ("ceiling", number(s.window_ceiling)),
            ],
        ),
        ("testing_concurrency", one(number(s.testing_concurrency))),
        (
            "bisection_concurrency",
            one(number(s.bisection_concurrency)),
        ),
        (
            "testing_timeout_minutes",
            one(number(s.testing_timeout_minutes)),
        ),
        (
            "not_ready_timeout_hours",
            one(number(s.not_ready_timeout_hours)),
        ),
        (
            "merge_method",
            one(name(s.merge_method.map(MergeMethod::as_str))),
        ),
        (
            "merge_commit_title",
            one(name(s.merge_commit_title.map(MergeCommitTitle::as_str))),
        ),
        ("trigger", one(name(s.trigger.map(Trigger::as_str)))),
        (
            "update_method",
            one(name(s.update_method.map(UpdateMethod::as_str))),
        ),
        (
            "batching",
            vec![
                ("enabled", flag(s.batching_enabled)),
                ("size", number(s.batching_size)),
                ("max_wait_minutes", number(s.batching_max_wait_minutes)),
            ],
        ),
        (
            "failure_budget",
            vec![("retries", number(s.failure_budget_retries))],
        ),
        ("optimistic_merging", one(flag(s.optimistic_merging))),
        (
            "pending_failure_depth",
            one(number(s.pending_failure_depth)),
        ),
        (
            "direct_merge_mode",
            one(name(s.direct_merge_mode.map(DirectMergeMode::as_str))),
        ),
        (
            "optimization_mode",
            one(name(s.optimization_mode.map(OptimizationMode::as_str))),
        ),
        ("comments_enabled", one(flag(s.comments_enabled))),
        ("commands_enabled", one(flag(s.commands_enabled))),
        ("status_check_enabled", one(flag(s.status_check_enabled))),
        (
            "label_commands",
            vec![
                ("enabled", flag(s.label_commands_enabled)),
                ("enqueue_label", text(&s.enqueue_label)),
            ],
        ),
        ("state_labels_enabled", one(flag(s.state_labels_enabled))),
        ("resubmit_on_push", one(flag(s.resubmit_on_push))),
        (
            "keep_test_branches_minutes",
            one(number(s.keep_test_branches_minutes)),
        ),
        ("branch_prefix", one(text(&s.branch_prefix))),
        (
            "branch_protection_injection",
            one(name(
                s.branch_protection_injection
                    .map(BranchProtectionInjection::as_str),
            )),
        ),
    ]
}

fn settings_entries(settings: &SettingsDocument) -> Vec<(String, Node)> {
    let mut entries = Vec::new();
    for (key, values) in setting_keys(settings) {
        let map: Vec<(String, Node)> = values
            .into_iter()
            .filter_map(|(name, value)| value.map(|value| (name.to_owned(), Node::Scalar(value))))
            .collect();
        match map.as_slice() {
            [] => {}
            [(name, value)] if name.is_empty() => entries.push((key.to_owned(), value.clone())),
            _ => entries.push((key.to_owned(), Node::Map(map))),
        }
    }
    match &settings.required_statuses {
        Some(RequiredStatusesDocument::BranchRules) => entries.push((
            "required_statuses".to_owned(),
            Node::Scalar("null".to_owned()),
        )),
        Some(RequiredStatusesDocument::Checks { names }) => entries.push((
            "required_statuses".to_owned(),
            Node::List(
                names
                    .iter()
                    .map(|name| Node::Scalar(text_scalar(name)))
                    .collect(),
            ),
        )),
        None => {}
    }
    entries
}

/// A map of conditions. Two conditions that the map cannot hold apart, on
/// the same key, go in an `all` block, one map each.
fn condition_map(conditions: &[ConditionDocument]) -> Vec<(String, Node)> {
    let mut map = Vec::new();
    for condition in conditions {
        if !insert_condition(&mut map, condition) {
            let maps = conditions
                .iter()
                .map(|condition| Node::Map(condition_map(std::slice::from_ref(condition))))
                .collect();
            return vec![("all".to_owned(), Node::List(maps))];
        }
    }
    map
}

/// Puts `condition` in `map`; `false` when its key is already taken.
fn insert_condition(map: &mut Vec<(String, Node)>, condition: &ConditionDocument) -> bool {
    match condition {
        ConditionDocument::Condition {
            attribute,
            operator,
            values,
        } => {
            let path: Vec<&str> = attribute
                .split('.')
                .chain(operator.iter().flat_map(|operator| operator.split('.')))
                .collect();
            let leaf = Leaf {
                values,
                is_per_value: holds_per_value(attribute, operator.as_deref()),
            };
            insert_leaf(map, &path, &leaf)
        }
        ConditionDocument::Block { block, conditions } => {
            let key = match block {
                ConditionBlock::All => "all",
                ConditionBlock::Any => "any",
                ConditionBlock::None => "none",
            };
            if map.iter().any(|(name, _)| name == key) {
                return false;
            }
            let maps = conditions.iter().map(block_element).collect();
            map.push((key.to_owned(), Node::List(maps)));
            true
        }
    }
}

/// An element of a block: one map. An `all` block in a block is the map of
/// its conditions, as the file writes several conditions in one element.
fn block_element(condition: &ConditionDocument) -> Node {
    match condition {
        ConditionDocument::Block {
            block: ConditionBlock::All,
            conditions,
        } => Node::Map(condition_map(conditions)),
        _ => Node::Map(condition_map(std::slice::from_ref(condition))),
    }
}

/// The values of a condition, and whether each holds on its own.
struct Leaf<'a> {
    values: &'a [String],
    is_per_value: bool,
}

/// Puts the values of a condition at `path`, with those of a condition of
/// the same path when each value holds on its own; `false` when the key is
/// taken.
fn insert_leaf(map: &mut Vec<(String, Node)>, path: &[&str], leaf: &Leaf<'_>) -> bool {
    let Some((key, rest)) = path.split_first() else {
        return false;
    };
    let found = map.iter_mut().find(|(name, _)| name == key);

    match (found, rest.is_empty()) {
        (None, true) => {
            map.push(((*key).to_owned(), values_node(leaf.values)));
            true
        }
        (None, false) => {
            let mut inner = Vec::new();
            let is_inserted = insert_leaf(&mut inner, rest, leaf);
            map.push(((*key).to_owned(), Node::Map(inner)));
            is_inserted
        }
        (Some((_, Node::Map(inner))), false) => insert_leaf(inner, rest, leaf),
        (Some((_, node)), true) if leaf.is_per_value && !matches!(node, Node::Map(_)) => {
            let mut all = scalars_of(node);
            all.extend(leaf.values.iter().map(|value| value_scalar(value)));
            *node = Node::List(all.into_iter().map(Node::Scalar).collect());
            true
        }
        _ => false,
    }
}

fn scalars_of(node: &Node) -> Vec<String> {
    match node {
        Node::Scalar(scalar) => vec![scalar.clone()],
        Node::List(items) => items.iter().flat_map(scalars_of).collect(),
        Node::Map(_) => Vec::new(),
    }
}

/// One value as a scalar, several as a list.
fn values_node(values: &[String]) -> Node {
    match values {
        [value] => Node::Scalar(value_scalar(value)),
        values => Node::List(
            values
                .iter()
                .map(|value| Node::Scalar(value_scalar(value)))
                .collect(),
        ),
    }
}

/// A plain scalar keeps what YAML reads without quotes as text, in a block:
/// letters, digits, `_`, `.`, `-`, `/`, then `[`, `]`, `*` and `@` after the
/// first character, which would start a flow, an alias or a reserved token.
fn is_plain(text: &str) -> bool {
    let mut chars = text.chars();
    let is_word = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '/');
    chars.next().is_some_and(is_word)
        && chars.all(|c| is_word(c) || matches!(c, '-' | '[' | ']' | '*' | '@'))
        && !matches!(
            text.to_ascii_lowercase().as_str(),
            "null" | "true" | "false"
        )
}

/// A value of a condition: the condition reads the text of a number or of
/// `true` and `false`, so they stay plain.
fn value_scalar(text: &str) -> String {
    if is_plain(text) || text == "true" || text == "false" {
        text.to_owned()
    } else {
        quoted(text)
    }
}

/// A text setting or a name: a number would not read as text.
fn text_scalar(text: &str) -> String {
    let is_number = text.chars().next().is_some_and(|c| c.is_ascii_digit());
    if is_plain(text) && !is_number {
        text.to_owned()
    } else {
        quoted(text)
    }
}

/// A priority: a value from 0 to 255 is a number, a name is a text.
fn priority_scalar(text: &str) -> String {
    if text.parse::<u8>().is_ok() {
        text.to_owned()
    } else {
        text_scalar(text)
    }
}

fn quoted(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn write_map(entries: &[(String, Node)], indent: usize, lines: &mut Vec<String>) {
    let pad = " ".repeat(indent);
    for (key, node) in entries {
        match node {
            Node::Scalar(scalar) => lines.push(format!("{pad}{key}: {scalar}")),
            Node::List(items) if items.is_empty() => lines.push(format!("{pad}{key}: []")),
            Node::Map(map) if map.is_empty() => lines.push(format!("{pad}{key}: {{}}")),
            Node::List(items) => {
                lines.push(format!("{pad}{key}:"));
                write_list(items, indent + 2, lines);
            }
            Node::Map(map) => {
                lines.push(format!("{pad}{key}:"));
                write_map(map, indent + 2, lines);
            }
        }
    }
}

/// The items of a list; a map item starts on the line of its dash.
fn write_list(items: &[Node], indent: usize, lines: &mut Vec<String>) {
    let pad = " ".repeat(indent);
    for item in items {
        match item {
            Node::Map(map) if !map.is_empty() => {
                let first = lines.len();
                write_map(map, indent + 2, lines);
                if let Some(line) = lines.get_mut(first) {
                    line.replace_range(indent..indent + 2, "- ");
                }
            }
            Node::Scalar(scalar) => lines.push(format!("{pad}- {scalar}")),
            Node::Map(_) => lines.push(format!("{pad}- {{}}")),
            Node::List(_) => lines.push(format!("{pad}- []")),
        }
    }
}

/// The lines of `before`, then of `after`: those that stay, that go and that
/// come, from the longest common subsequence. A file holds a few hundred
/// lines at most.
pub fn diff_lines(before: &str, after: &str) -> Vec<DiffLine> {
    let before: Vec<&str> = before.lines().collect();
    let after: Vec<&str> = after.lines().collect();
    let width = after.len() + 1;
    let mut common = vec![0_u32; (before.len() + 1) * width];
    for i in (0..before.len()).rev() {
        for j in (0..after.len()).rev() {
            common[i * width + j] = if before[i] == after[j] {
                common[(i + 1) * width + j + 1] + 1
            } else {
                common[(i + 1) * width + j].max(common[i * width + j + 1])
            };
        }
    }

    let line = |change, text: &str| DiffLine {
        change,
        text: text.to_owned(),
    };
    let (mut i, mut j) = (0, 0);
    let mut diff = Vec::with_capacity(before.len().max(after.len()));
    while i < before.len() || j < after.len() {
        if i < before.len() && j < after.len() && before[i] == after[j] {
            diff.push(line(LineChange::Same, before[i]));
            (i, j) = (i + 1, j + 1);
        } else if j < after.len()
            && (i == before.len() || common[i * width + j + 1] >= common[(i + 1) * width + j])
        {
            diff.push(line(LineChange::Added, after[j]));
            j += 1;
        } else {
            diff.push(line(LineChange::Removed, before[i]));
            i += 1;
        }
    }
    diff
}

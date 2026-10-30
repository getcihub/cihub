//! The YAML shape of `.cihub.yaml`, read into the model with the position of
//! each value.

use std::collections::HashSet;

use cihub_conditions::{Conditions, Glob};
use cihub_domain::{
    BranchProtectionInjection, DirectMergeMode, MergeCommitTitle, MergeMethod, OptimizationMode,
    Priority, QueueMode, Strategy, Trigger, UpdateMethod,
};
use serde::Deserialize;
use serde::de::{Deserializer, IgnoredAny};
use serde_saphyr::{DefaultMessageFormatter, MessageFormatter, Spanned};

use crate::conditions::{RawNode, Reader};
use crate::settings::in_range;
use crate::{
    ConfigError, ConfigErrorKind, Layer, Position, PriorityRule, ProfileConfig, QueueConfig,
    QueueSettings, RepoConfig, Setting,
};

/// Reads `text`. Each queue is also resolved, so that values that do not go
/// together fail here, with their position.
pub(crate) fn repo_config(text: &str) -> Result<RepoConfig, ConfigError> {
    check_version(text)?;
    let file: RawFile = from_str(text)?;
    let reader = Reader { text };
    let defaults = match file.defaults {
        Some(raw) => defaults(raw, reader)?,
        None => Defaults::default(),
    };

    QueueSettings::resolve(&Layer::default(), &defaults.settings)?;
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

        queue.resolve(&defaults.settings)?;
        queues.push(queue);
    }

    let priority_rules = file
        .priority_rules
        .into_iter()
        .map(|rule| priority_rule(rule, reader))
        .collect::<Result<_, _>>()?;

    Ok(RepoConfig {
        defaults: defaults.settings,
        queue_conditions: defaults.queue_conditions,
        merge_conditions: defaults.merge_conditions,
        queues,
        priority_rules,
    })
}

fn check_version(text: &str) -> Result<(), ConfigError> {
    #[derive(Deserialize)]
    struct Probe {
        #[serde(default)]
        version: Option<Spanned<u32>>,
    }

    let probe: Option<Probe> = from_str(text)?;
    match probe.and_then(|probe| probe.version) {
        None => Err(ConfigError::new(
            ConfigErrorKind::BadVersion,
            None,
            "missing `version: 1`",
        )),
        Some(version) if version.value != 1 => Err(ConfigError::new(
            ConfigErrorKind::BadVersion,
            Some(Position::of_value(&version)),
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
    serde_saphyr::from_str_with_options(text, options).map_err(|error| yaml_error(&error))
}

fn yaml_error(error: &serde_saphyr::Error) -> ConfigError {
    let error = error.without_snippet();
    let message = DefaultMessageFormatter
        .format_message(error)
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>();
    let position = error.location().map(|location| Position::of(&location));

    ConfigError::parsing(position, message)
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

fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

/// The settings and the conditions of `defaults`.
#[derive(Default)]
struct Defaults {
    settings: Layer,
    queue_conditions: Conditions,
    merge_conditions: Conditions,
}

fn defaults(mut raw: RawQueue, reader: Reader<'_>) -> Result<Defaults, ConfigError> {
    let queue_only = [
        ("branch", raw.branch.as_ref().map(Position::of_value)),
        ("profiles", raw.profiles.as_ref().map(Position::of_value)),
    ];
    if let Some((key, at)) = queue_only.into_iter().find(|(_, at)| at.is_some()) {
        return Err(ConfigError::parsing(
            at,
            format!("`{key}` is not allowed in `defaults`"),
        ));
    }

    let queue_conditions = reader.conditions(raw.queue_conditions.take().as_ref())?;
    let merge_conditions = reader.merge_conditions(raw.merge_conditions.take().as_ref())?;

    Ok(Defaults {
        settings: layer(raw)?,
        queue_conditions,
        merge_conditions,
    })
}

/// The queue and the position of its branch.
fn queue(
    raw: Spanned<RawQueue>,
    reader: Reader<'_>,
) -> Result<(QueueConfig, Option<Position>), ConfigError> {
    let queue_at = Some(Position::of_value(&raw));
    let mut raw = raw.value;
    let (branch_at, branch) = raw
        .branch
        .take()
        .map(|branch| (Some(Position::of_value(&branch)), branch.value))
        .ok_or_else(|| ConfigError::parsing(queue_at, "a queue needs a `branch`"))?;
    let pattern = branch_pattern(&branch, branch_at)?;
    let raw_profiles = raw.profiles.take().map(|profiles| profiles.value);
    let mut names = HashSet::new();
    let mut profiles = Vec::new();

    for raw_profile in raw_profiles.unwrap_or_default() {
        let (profile, name_at) = profile(raw_profile, reader)?;
        if !names.insert(profile.name.clone()) {
            return Err(ConfigError::parsing(
                name_at,
                format!("the profile `{}` is defined twice", profile.name),
            ));
        }

        profiles.push(profile);
    }

    let queue = QueueConfig {
        branch,
        pattern,
        queue_conditions: reader.conditions(raw.queue_conditions.take().as_ref())?,
        merge_conditions: reader.merge_conditions(raw.merge_conditions.take().as_ref())?,
        settings: layer(raw)?,
        profiles,
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
    let name_at = Some(Position::of_value(&raw.name));

    if let Some(method) = &raw.merge_method
        && method.value == MergeMethod::FastForward
    {
        return Err(ConfigError::parsing(
            Some(Position::of_value(method)),
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

    Ok((profile, name_at))
}

fn setting<T>(value: Value<T>) -> Option<Setting<T>> {
    value.map(|value| Setting {
        at: Some(Position::of_value(&value)),
        value: value.value,
    })
}

fn layer(raw: RawQueue) -> Result<Layer, ConfigError> {
    let (window_floor, window_ceiling) = window(raw.testing_concurrency, raw.window)?;
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

type Window = (Option<Setting<u32>>, Option<Setting<u32>>);

/// The floor and the ceiling of the window: `testing_concurrency: N` is a
/// short form of `window: { floor: N, ceiling: N }`.
fn window(
    testing_concurrency: Value<u32>,
    window: Value<RawWindow>,
) -> Result<Window, ConfigError> {
    match (testing_concurrency, window) {
        (Some(concurrency), Some(_)) => Err(ConfigError::parsing(
            Some(Position::of_value(&concurrency)),
            "`testing_concurrency` and `window` cannot both be set: \
             `testing_concurrency: N` means `window: { floor: N, ceiling: N }`",
        )),
        (Some(concurrency), None) => {
            let concurrency = setting(Some(concurrency));
            in_range(concurrency.as_ref(), "testing_concurrency", 1..=100)?;
            Ok((concurrency.clone(), concurrency))
        }
        (None, window) => {
            let window = window.map(|window| window.value).unwrap_or_default();
            Ok((setting(window.floor), setting(window.ceiling)))
        }
    }
}

fn priority_rule(raw: RawPriorityRule, reader: Reader<'_>) -> Result<PriorityRule, ConfigError> {
    let priority_at = Some(Position::of_value(&raw.priority));
    let priority = match raw.priority.value {
        RawPriority::Value(value) => Priority(value),
        RawPriority::Name(name) => Priority::from_name(&name).ok_or_else(|| {
            ConfigError::parsing(
                priority_at,
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

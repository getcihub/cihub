//! Queue settings, their sources, and the strategy table.

use std::collections::HashSet;
use std::ops::RangeInclusive;

use cihub_domain::{
    BranchProtectionInjection, DirectMergeMode, MergeCommitTitle, MergeMethod, OptimizationMode,
    QueueMode, Strategy, Trigger, UpdateMethod,
};

use crate::{ConfigError, ConfigErrorKind, Position, TargetsSource};

/// Where the value of a setting comes from, from the weakest to the strongest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Source {
    Default,
    Strategy,
    File,
}

/// A value set by one source. `at` is its position when it comes from the
/// file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Setting<T> {
    pub value: T,
    pub at: Option<Position>,
}

impl<T> Setting<T> {
    pub fn new(value: T) -> Self {
        Self { value, at: None }
    }
}

/// The effective value of a setting and the source that gave it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sourced<T> {
    pub value: T,
    pub source: Source,
    pub at: Option<Position>,
}

macro_rules! settings {
    ($($(#[$meta:meta])* $field:ident: $ty:ty = $default:expr;)+) => {
        /// The settings that one source sets; `None` leaves the value to a
        /// weaker source.
        #[derive(Clone, Debug, Default, PartialEq, Eq)]
        pub struct Layer {
            $($(#[$meta])* pub $field: Option<Setting<$ty>>,)+
        }

        /// The settings of a queue after the merge of all sources.
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct QueueSettings {
            $($(#[$meta])* pub $field: Sourced<$ty>,)+
        }

        impl QueueSettings {
            fn merge(layers: &[(Source, &Layer)]) -> Self {
                Self {
                    $($field: pick(layers, |layer| layer.$field.as_ref(), || $default),)+
                }
            }
        }
    };
}

settings! {
    strategy: Strategy = Strategy::Pipelined;
    mode: QueueMode = QueueMode::Single;
    /// Required in `parallel` mode (`DECISIONS.md` D226).
    affected_targets: Option<TargetsSource> = None;
    window_floor: u32 = 2;
    window_ceiling: u32 = 20;
    /// Follows `window_ceiling` when no source sets it.
    bisection_concurrency: u32 = 20;
    testing_timeout_minutes: u32 = 300;
    not_ready_timeout_hours: u32 = 168;
    merge_method: MergeMethod = MergeMethod::Squash;
    merge_commit_title: MergeCommitTitle = MergeCommitTitle::PrTitle;
    trigger: Trigger = Trigger::DraftPr;
    update_method: UpdateMethod = UpdateMethod::Merge;
    batching_enabled: bool = false;
    /// Largest batch; a batch starts when full or after the wait.
    batching_size: u32 = 4;
    batching_max_wait_minutes: u32 = 5;
    failure_budget_retries: u32 = 1;
    optimistic_merging: bool = false;
    pending_failure_depth: u32 = 0;
    /// `off` by default, in every strategy (`DECISIONS.md` D153).
    direct_merge_mode: DirectMergeMode = DirectMergeMode::Off;
    optimization_mode: OptimizationMode = OptimizationMode::BisectionSkipRedundantTests;
    comments_enabled: bool = true;
    commands_enabled: bool = true;
    status_check_enabled: bool = true;
    label_commands_enabled: bool = true;
    enqueue_label: String = "cihub-merge-queue-submit".to_owned();
    state_labels_enabled: bool = false;
    /// A push on a PR in the queue submits it again at the end of the queue
    /// (`DECISIONS.md` D152).
    resubmit_on_push: bool = false;
    keep_test_branches_minutes: u32 = 0;
    /// Test branches are `<prefix>-merge/` and `<prefix>-temp/`.
    branch_prefix: String = "cihub".to_owned();
    branch_protection_injection: BranchProtectionInjection = BranchProtectionInjection::Both;
    /// `None` inherits the branch protection; an empty list requires no
    /// status.
    required_statuses: Option<Vec<String>> = None;
}

fn pick<T: Clone>(
    layers: &[(Source, &Layer)],
    get: impl Fn(&Layer) -> Option<&Setting<T>>,
    default: impl FnOnce() -> T,
) -> Sourced<T> {
    layers
        .iter()
        .find_map(|&(source, layer)| {
            get(layer).map(|setting| Sourced {
                value: setting.value.clone(),
                source,
                at: setting.at,
            })
        })
        .unwrap_or_else(|| Sourced {
            value: default(),
            source: Source::Default,
            at: None,
        })
}

/// The values that a strategy sets, as in the strategy table of the spec.
fn strategy_layer(strategy: Strategy) -> Layer {
    let (mode, floor, ceiling, batch_size, depth, optimistic) = match strategy {
        Strategy::Sequential => (QueueMode::Single, 1, 1, None, 0, false),
        Strategy::Pipelined => (QueueMode::Single, 2, 20, None, 0, false),
        Strategy::Batched => (QueueMode::Single, 2, 20, Some(4), 1, true),
        Strategy::Parallel => (QueueMode::Parallel, 2, 20, Some(4), 1, true),
    };
    Layer {
        mode: Some(Setting::new(mode)),
        window_floor: Some(Setting::new(floor)),
        window_ceiling: Some(Setting::new(ceiling)),
        batching_enabled: Some(Setting::new(batch_size.is_some())),
        batching_size: batch_size.map(Setting::new),
        pending_failure_depth: Some(Setting::new(depth)),
        optimistic_merging: Some(Setting::new(optimistic)),
        failure_budget_retries: Some(Setting::new(1)),
        ..Layer::default()
    }
}

impl QueueSettings {
    /// Merges the sources of one queue, from the strongest: the queue in the
    /// file, then `defaults` in the file (`DECISIONS.md` D134).
    ///
    /// # Errors
    ///
    /// A value out of range in any source, or effective values that do not
    /// go together.
    pub fn resolve(queue: &Layer, defaults: &Layer) -> Result<Self, ConfigError> {
        for layer in [queue, defaults] {
            layer.validate()?;
        }

        let mut sources = vec![(Source::File, queue), (Source::File, defaults)];
        let strategy_layer = strategy_layer(Self::merge(&sources).strategy.value);

        sources.push((Source::Strategy, &strategy_layer));
        let mut settings = Self::merge(&sources);
        if settings.bisection_concurrency.source == Source::Default {
            settings.bisection_concurrency.value = settings.window_ceiling.value;
        }

        settings.check()?;
        Ok(settings)
    }

    fn check(&self) -> Result<(), ConfigError> {
        let (floor, ceiling) = (&self.window_floor, &self.window_ceiling);
        if floor.value > ceiling.value {
            return Err(ConfigError::parsing(
                floor.at.or(ceiling.at),
                format!(
                    "window.floor ({}) is greater than window.ceiling ({})",
                    floor.value, ceiling.value
                ),
            ));
        }

        if self.mode.value == QueueMode::Parallel && self.affected_targets.value.is_none() {
            return Err(ConfigError::parsing(
                self.mode.at.or(self.strategy.at),
                "a queue in `parallel` mode needs an `affected_targets` block".to_owned(),
            ));
        }

        if self.trigger.value == Trigger::Inplace {
            return Err(ConfigError::parsing(
                self.trigger.at,
                "trigger `inplace` is not available yet".to_owned(),
            ));
        }

        if self.merge_method.value == MergeMethod::FastForward
            && self.strategy.value != Strategy::Sequential
        {
            return Err(ConfigError::parsing(
                self.merge_method.at.or(self.strategy.at),
                format!(
                    "merge_method `fast_forward` needs strategy `sequential`, not `{}`",
                    self.strategy.value
                ),
            ));
        }

        Ok(())
    }
}

impl Layer {
    fn validate(&self) -> Result<(), ConfigError> {
        for (setting, name, range) in [
            (&self.window_floor, "window.floor", 1..=100),
            (&self.window_ceiling, "window.ceiling", 1..=100),
            (
                &self.bisection_concurrency,
                "bisection_concurrency",
                1..=200,
            ),
            (
                &self.testing_timeout_minutes,
                "testing_timeout_minutes",
                10..=1440,
            ),
            (
                &self.not_ready_timeout_hours,
                "not_ready_timeout_hours",
                1..=720,
            ),
            (&self.batching_size, "batching.size", 2..=20),
            (
                &self.batching_max_wait_minutes,
                "batching.max_wait_minutes",
                1..=60,
            ),
            (
                &self.failure_budget_retries,
                "failure_budget.retries",
                0..=10,
            ),
            (&self.pending_failure_depth, "pending_failure_depth", 0..=10),
            (
                &self.keep_test_branches_minutes,
                "keep_test_branches_minutes",
                0..=1440,
            ),
        ] {
            in_range(setting.as_ref(), name, range)?;
        }

        if let Some(label) = &self.enqueue_label
            && label.value.trim().is_empty()
        {
            return Err(ConfigError::parsing(
                label.at,
                "label_commands.enqueue_label must not be empty",
            ));
        }

        if let Some(prefix) = &self.branch_prefix
            && (prefix.value.is_empty()
                || prefix
                    .value
                    .contains(|c: char| c == '/' || c.is_whitespace()))
        {
            return Err(ConfigError::parsing(
                prefix.at,
                format!(
                    "branch_prefix `{}` must be non-empty, without `/` or spaces",
                    prefix.value
                ),
            ));
        }

        if let Some(Setting {
            value: Some(names),
            at,
        }) = &self.required_statuses
        {
            let mut seen = HashSet::new();
            for name in names {
                let problem = if name.trim().is_empty() {
                    "an empty name"
                } else if !seen.insert(name) {
                    "a duplicate name"
                } else {
                    continue;
                };
                return Err(ConfigError {
                    kind: ConfigErrorKind::BadRequiredStatuses,
                    position: *at,
                    message: format!("required_statuses has {problem}: `{name}`"),
                });
            }
        }

        Ok(())
    }
}

pub(crate) fn in_range(
    setting: Option<&Setting<u32>>,
    name: &str,
    range: RangeInclusive<u32>,
) -> Result<(), ConfigError> {
    match setting {
        Some(setting) if !range.contains(&setting.value) => Err(ConfigError::parsing(
            setting.at,
            format!(
                "{name} is {}, out of range {}–{}",
                setting.value,
                range.start(),
                range.end()
            ),
        )),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(line: u64) -> Position {
        Position { line, column: 1 }
    }

    #[test]
    fn strategy_sets_its_defaults() {
        let defaults = Layer::default();
        let queue = Layer {
            strategy: Some(Setting::new(Strategy::Batched)),
            ..Layer::default()
        };
        let settings = QueueSettings::resolve(&queue, &defaults).unwrap();

        assert_eq!(settings.strategy.source, Source::File);
        assert_eq!(
            (
                settings.batching_enabled.value,
                settings.batching_size.value
            ),
            (true, 4)
        );
        assert_eq!(settings.batching_size.source, Source::Strategy);
        assert_eq!(settings.testing_timeout_minutes.source, Source::Default);
        assert_eq!(settings.bisection_concurrency.value, 20);
    }

    #[test]
    fn stronger_sources_win() {
        let defaults = Layer {
            testing_timeout_minutes: Some(Setting::new(60)),
            window_ceiling: Some(Setting::new(10)),
            ..Layer::default()
        };
        let queue = Layer {
            testing_timeout_minutes: Some(Setting::new(90)),
            ..Layer::default()
        };
        let settings = QueueSettings::resolve(&queue, &defaults).unwrap();

        assert_eq!(settings.testing_timeout_minutes.value, 90);
        assert_eq!(settings.testing_timeout_minutes.source, Source::File);
        assert_eq!(settings.window_ceiling.source, Source::File);
        assert_eq!(settings.bisection_concurrency.value, 10);
    }

    #[test]
    fn checks_values_together() {
        let queue = Layer {
            window_floor: Some(Setting {
                value: 30,
                at: Some(at(4)),
            }),
            ..Layer::default()
        };
        let error = QueueSettings::resolve(&queue, &Layer::default()).unwrap_err();

        assert_eq!(error.position, Some(at(4)));
        assert_eq!(
            error.message,
            "window.floor (30) is greater than window.ceiling (20)"
        );

        let queue = Layer {
            trigger: Some(Setting {
                value: Trigger::Inplace,
                at: Some(at(7)),
            }),
            ..Layer::default()
        };
        let error = QueueSettings::resolve(&queue, &Layer::default()).unwrap_err();

        assert_eq!(error.position, Some(at(7)));
        assert_eq!(error.message, "trigger `inplace` is not available yet");

        let fast_forward = |strategy| Layer {
            strategy: Some(Setting::new(strategy)),
            merge_method: Some(Setting {
                value: MergeMethod::FastForward,
                at: Some(at(8)),
            }),
            ..Layer::default()
        };
        let error = QueueSettings::resolve(&fast_forward(Strategy::Pipelined), &Layer::default())
            .unwrap_err();

        assert_eq!(error.position, Some(at(8)));
        assert_eq!(
            error.message,
            "merge_method `fast_forward` needs strategy `sequential`, not `pipelined`"
        );
        let settings =
            QueueSettings::resolve(&fast_forward(Strategy::Sequential), &Layer::default()).unwrap();

        assert_eq!(settings.merge_method.value, MergeMethod::FastForward);
    }

    #[test]
    fn validates_every_source() {
        let queue = Layer {
            batching_size: Some(Setting::new(50)),
            ..Layer::default()
        };
        let error = QueueSettings::resolve(&queue, &Layer::default()).unwrap_err();

        assert_eq!(error.message, "batching.size is 50, out of range 2–20");
        assert_eq!(error.position, None);

        let queue = Layer {
            required_statuses: Some(Setting::new(Some(vec!["ci".into(), "ci".into()]))),
            ..Layer::default()
        };
        let error = QueueSettings::resolve(&queue, &Layer::default()).unwrap_err();

        assert_eq!(error.kind, ConfigErrorKind::BadRequiredStatuses);
    }
}

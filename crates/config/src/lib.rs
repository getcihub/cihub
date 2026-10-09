//! Queue configuration: the `.cihub.yaml` model, the only source of the
//! settings of a queue (`DECISIONS.md` D134), the strategy table, and the merge
//! of the sources, with the source of each setting, and the affected targets
//! of a PR from its changed files. Pure: the caller reads the file and the
//! files of the PR.

mod error;
mod file;
mod settings;
mod targets;

pub use error::{ConfigError, ConfigErrorKind, Position};
pub use file::{FILE_NAMES, PriorityRule, QueueConfig, RepoConfig, find_config_file};
pub use settings::{Layer, QueueSettings, Setting, Source, Sourced};
pub use targets::{Target, TargetPattern, TargetsSource, targets_of};

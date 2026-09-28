//! Queue configuration: the `.cihub.yaml` model, the strategy table, and the
//! merge of the file with the dashboard, with the source of each setting.
//! Pure: the caller reads the file.

mod error;
mod file;
mod settings;

pub use error::{ConfigError, ConfigErrorKind, Position};
pub use file::{FILE_NAMES, PriorityRule, QueueConfig, RepoConfig, find_config_file};
pub use settings::{Layer, QueueSettings, Setting, Source, Sourced};

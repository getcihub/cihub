//! Queue configuration: the `.cihub.yaml` model, the only source of the
//! settings of a queue, the strategy table, and the merge of the sources, with
//! the source of each setting. Pure: the caller reads the file.

mod conditions;
mod error;
mod file;
mod settings;

pub use error::{ConfigError, ConfigErrorKind, Position};
pub use file::{
    FILE_NAMES, PriorityRule, ProfileConfig, QueueConfig, RepoConfig, find_config_file,
};
pub use settings::{Layer, QueueSettings, Setting, Source, Sourced};

//! Queue configuration: the `.cihub.yaml` model, the only source of the
//! settings of a queue, the strategy table, and the merge of the sources, with
//! the source of each setting; the document of a file, the YAML of a
//! document and the diff of two files, for the dashboard. Pure: the caller
//! reads and writes the file.

mod conditions;
mod document;
mod error;
mod file;
mod settings;

pub use document::{diff_lines, write_document};
pub use error::{ConfigError, ConfigErrorKind, Position};
pub use file::{
    FILE_NAMES, PriorityRule, ProfileConfig, QueueConfig, RepoConfig, find_config_file,
    merge_checks_of,
};
pub use settings::{Layer, QueueSettings, Setting, Source, Sourced};

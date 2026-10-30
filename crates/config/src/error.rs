use cihub_domain::JobError;
use serde_saphyr::Spanned;

/// 1-based line and character column in the configuration file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub line: u64,
    pub column: u64,
}

impl Position {
    pub(crate) fn of(location: &serde_saphyr::Location) -> Self {
        Self {
            line: location.line(),
            column: location.column(),
        }
    }

    /// The position of a value of the file.
    pub(crate) fn of_value<T>(value: &Spanned<T>) -> Self {
        Self::of(&value.referenced)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigErrorKind {
    /// Invalid YAML, unknown key, value out of range, or two settings that
    /// do not go together.
    Parsing,
    BadVersion,
    BadRequiredStatuses,
}

/// An error in the configuration, with the position of the value at fault
/// when it comes from the file.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{}{message}", prefix(*position))]
pub struct ConfigError {
    pub kind: ConfigErrorKind,
    pub position: Option<Position>,
    pub message: String,
}

impl ConfigError {
    pub(crate) fn new(
        kind: ConfigErrorKind,
        position: Option<Position>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            position,
            message: message.into(),
        }
    }

    pub(crate) fn parsing(position: Option<Position>, message: impl Into<String>) -> Self {
        Self::new(ConfigErrorKind::Parsing, position, message)
    }

    /// The error that the assembly job reports for this configuration.
    pub fn job_error(&self) -> JobError {
        match self.kind {
            ConfigErrorKind::Parsing => JobError::ConfigParsingFailure,
            ConfigErrorKind::BadVersion => JobError::ConfigBadVersion,
            ConfigErrorKind::BadRequiredStatuses => JobError::ConfigBadRequiredStatuses,
        }
    }
}

fn prefix(position: Option<Position>) -> String {
    position
        .map(|Position { line, column }| format!("line {line}, column {column}: "))
        .unwrap_or_default()
}

use std::fmt;

use cihub_domain::FailedReason;

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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigError {
    pub kind: ConfigErrorKind,
    pub position: Option<Position>,
    pub message: String,
}

impl ConfigError {
    pub(crate) fn parsing(position: Option<Position>, message: impl Into<String>) -> Self {
        Self {
            kind: ConfigErrorKind::Parsing,
            position,
            message: message.into(),
        }
    }

    /// The reason that fails a test run started with this configuration.
    pub fn reason(&self) -> FailedReason {
        match self.kind {
            ConfigErrorKind::Parsing => FailedReason::StartTestRunConfigParsingFailure,
            ConfigErrorKind::BadVersion => FailedReason::StartTestRunConfigBadVersion,
            ConfigErrorKind::BadRequiredStatuses => {
                FailedReason::StartTestRunConfigBadRequiredStatuses
            }
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.position {
            Some(Position { line, column }) => {
                write!(f, "line {line}, column {column}: {}", self.message)
            }
            None => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for ConfigError {}

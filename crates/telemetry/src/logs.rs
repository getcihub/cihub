use std::error::Error;

use time::macros::format_description;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::ParseError;
use tracing_subscriber::fmt::time::UtcTime;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogFormat {
    Json,
    Pretty,
}

#[derive(Debug, thiserror::Error)]
pub enum LogsError {
    #[error("invalid log filter")]
    InvalidFilter(#[source] ParseError),
    #[error("a global subscriber is already installed")]
    AlreadyInstalled(#[source] Box<dyn Error + Send + Sync>),
}

#[derive(Debug)]
pub struct Logs {
    filter: EnvFilter,
    format: LogFormat,
}

impl Logs {
    /// `filter` uses the syntax of `EnvFilter`: `info,cihub_github=debug`.
    ///
    /// # Errors
    ///
    /// `filter` is not a valid filter.
    pub fn new(filter: &str, format: LogFormat) -> Result<Self, LogsError> {
        let filter = EnvFilter::try_new(filter).map_err(LogsError::InvalidFilter)?;
        Ok(Self { filter, format })
    }

    pub fn format(&self) -> LogFormat {
        self.format
    }

    /// Installs the global subscriber (`specs/OBSERVABILITY.md`).
    ///
    /// # Errors
    ///
    /// A global subscriber is already installed.
    pub fn init(self) -> Result<(), LogsError> {
        let builder = tracing_subscriber::fmt().with_env_filter(self.filter);
        match self.format {
            LogFormat::Json => builder
                .json()
                .flatten_event(true)
                .with_current_span(false)
                .with_span_list(true)
                .with_timer(UtcTime::new(format_description!(
                    "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
                )))
                .try_init(),
            LogFormat::Pretty => builder
                .compact()
                .with_target(false)
                .with_timer(UtcTime::new(format_description!(
                    "[hour]:[minute]:[second].[subsecond digits:3]"
                )))
                .try_init(),
        }
        .map_err(LogsError::AlreadyInstalled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_filters_are_refused() {
        assert!(matches!(
            Logs::new("info,=[", LogFormat::Json),
            Err(LogsError::InvalidFilter(_))
        ));
        assert!(Logs::new("info,cihub_github=debug", LogFormat::Pretty).is_ok());
    }
}

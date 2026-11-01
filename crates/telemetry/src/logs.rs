use std::error::Error;

use sentry::ClientInitGuard;
use sentry::types::ParseDsnError;
use time::macros::format_description;
use tracing::Subscriber;
use tracing_subscriber::filter::ParseError;
use tracing_subscriber::fmt::format::JsonFields;
use tracing_subscriber::fmt::time::UtcTime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

use crate::error_reports::{self, ErrorReports};
use crate::flat_json::FlatJson;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogFormat {
    Json,
    Pretty,
}

#[derive(Debug, thiserror::Error)]
pub enum LogsError {
    #[error("invalid log filter")]
    InvalidFilter(#[source] ParseError),
    #[error("invalid Sentry DSN")]
    InvalidDsn(#[source] ParseDsnError),
    #[error("a global subscriber is already installed")]
    AlreadyInstalled(#[source] Box<dyn Error + Send + Sync>),
    #[cfg(any(test, feature = "capture"))]
    #[error("a captured log line is not a JSON object")]
    InvalidRecord(#[source] serde_json::Error),
}

#[derive(Debug)]
pub struct Logs {
    filter: EnvFilter,
    format: LogFormat,
    error_reports: Option<ErrorReports>,
}

/// Sends the error reports still in memory when it is dropped, at the end of
/// the process.
#[must_use]
pub struct ErrorReportsGuard {
    _client: Option<ClientInitGuard>,
}

impl Logs {
    /// `filter` uses the syntax of `EnvFilter`: `info,cihub_github=debug`.
    ///
    /// # Errors
    ///
    /// `filter` is not a valid filter.
    pub fn new(filter: &str, format: LogFormat) -> Result<Self, LogsError> {
        let filter = EnvFilter::try_new(filter).map_err(LogsError::InvalidFilter)?;
        Ok(Self {
            filter,
            format,
            error_reports: None,
        })
    }

    /// Also sends the `ERROR` records that the filter keeps to a Sentry
    /// server.
    #[must_use]
    pub fn with_error_reports(self, error_reports: ErrorReports) -> Self {
        Self {
            error_reports: Some(error_reports),
            ..self
        }
    }

    pub fn format(&self) -> LogFormat {
        self.format
    }

    /// Installs the global subscriber (`specs/OBSERVABILITY.md`), and the
    /// client of the error reports when they have a server.
    ///
    /// # Errors
    ///
    /// A global subscriber is already installed.
    pub fn init(self) -> Result<ErrorReportsGuard, LogsError> {
        let client = self.error_reports.map(ErrorReports::start);

        subscriber(self.filter, self.format, client.is_some())
            .try_init()
            .map_err(|err| LogsError::AlreadyInstalled(Box::new(err)))?;

        Ok(ErrorReportsGuard { _client: client })
    }
}

/// Built on the registry, as the layer of the error reports filters its own
/// records, which the subscriber of `tracing_subscriber::fmt` refuses.
fn subscriber(
    filter: EnvFilter,
    format: LogFormat,
    has_error_reports: bool,
) -> impl Subscriber + Send + Sync + 'static {
    let lines = match format {
        LogFormat::Json => tracing_subscriber::fmt::layer()
            .fmt_fields(JsonFields::new())
            .event_format(FlatJson::new())
            .boxed(),
        LogFormat::Pretty => tracing_subscriber::fmt::layer()
            .compact()
            .with_target(false)
            .with_timer(UtcTime::new(format_description!(
                "[hour]:[minute]:[second].[subsecond digits:3]"
            )))
            .boxed(),
    };
    tracing_subscriber::registry()
        .with(filter)
        .with(lines)
        .with(has_error_reports.then(error_reports::layer))
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

    #[test]
    fn every_format_takes_the_layer_of_the_error_reports() {
        for format in [LogFormat::Json, LogFormat::Pretty] {
            // `off`: the test writes nothing to the standard output.
            let subscriber = subscriber(EnvFilter::new("off"), format, true);

            tracing::subscriber::with_default(subscriber, || {
                let _span = tracing::info_span!("job", org_id = "org_1").entered();
                tracing::error!("job.failed");
            });
        }
    }
}

use std::borrow::Cow;

use sentry::ClientInitGuard;
use sentry::integrations::tracing::{ErrorLayer, SpanLayer};
use sentry::types::Dsn;
use tracing::{Metadata, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::filter::{FilterExt, LevelFilter, filter_fn};
use tracing_subscriber::registry::LookupSpan;

use crate::LogsError;

/// The version of CIHub in the error reports of the server and of the
/// dashboard, so that the Sentry server groups their errors by version.
pub const RELEASE: &str = concat!("cihub@", env!("CARGO_PKG_VERSION"));

/// The Sentry server that receives the errors of the process: sentry.io or a
/// server of its own, which the DSN names (`specs/OBSERVABILITY.md`).
#[derive(Debug)]
pub struct ErrorReports {
    dsn: Dsn,
    environment: Option<String>,
}

impl ErrorReports {
    /// # Errors
    ///
    /// `dsn` is not a valid DSN.
    pub fn new(dsn: &str, environment: Option<String>) -> Result<Self, LogsError> {
        let dsn = dsn.parse().map_err(LogsError::InvalidDsn)?;
        Ok(Self { dsn, environment })
    }

    pub(crate) fn start(self) -> ClientInitGuard {
        // reqwest uses rustls without a provider: ring is the provider of the
        // process. A second install changes nothing.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let mut options = sentry::ClientOptions::new();
        options.dsn = Some(self.dsn);
        options.release = Some(Cow::Borrowed(RELEASE));
        options.environment = self.environment.map(Cow::Owned);
        sentry::init(options)
    }
}

/// Checks a DSN that the process does not use itself, such as the one of
/// the dashboard.
///
/// # Errors
///
/// `dsn` is not a valid DSN.
pub fn check_dsn(dsn: &str) -> Result<(), LogsError> {
    dsn.parse::<Dsn>().map(drop).map_err(LogsError::InvalidDsn)
}

/// An `ERROR` record becomes a Sentry event, with the keys of its open spans
/// (`org_id`, `queue_id`...). The spans make transactions only to carry these
/// keys: without a sample rate, the client sends none.
pub(crate) fn layer<S>() -> impl Layer<S> + Send + Sync + 'static
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let errors = ErrorLayer::new().enable_span_attributes().with_filter(
        LevelFilter::ERROR.or(filter_fn(|metadata: &Metadata<'_>| metadata.is_span())),
    );
    SpanLayer::new().and_then(errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dsn_names_a_server_and_a_project() {
        assert!(ErrorReports::new("https://key@sentry.example.com/42", None).is_ok());
        assert!(check_dsn("https://key@o1.ingest.sentry.io/42").is_ok());
        for wrong in ["", "sentry.example.com", "https://sentry.example.com/42"] {
            assert!(
                matches!(check_dsn(wrong), Err(LogsError::InvalidDsn(_))),
                "{wrong}"
            );
        }
    }
}

use std::borrow::Cow;

use sentry::ClientInitGuard;
use sentry::integrations::tracing::{ErrorLayer, SpanLayer};
use sentry::types::Dsn;
use tracing::{Level, Metadata, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::filter::filter_fn;
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

/// An `ERROR` record becomes a Sentry event, with the keys of its open spans
/// (`org_id`, `queue_id`...). The spans make transactions only to carry these
/// keys: without a sample rate, the client sends none.
pub(crate) fn layer<S>() -> impl Layer<S> + Send + Sync + 'static
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    // `ErrorLayer` keeps the keys of the spans that its filter accepts.
    let errors = ErrorLayer::new()
        .enable_span_attributes()
        .with_filter(filter_fn(|metadata: &Metadata<'_>| {
            metadata.is_span() || *metadata.level() == Level::ERROR
        }));
    SpanLayer::new().and_then(errors)
}

#[cfg(test)]
mod tests {
    use sentry::protocol::Context;
    use tracing_subscriber::layer::SubscriberExt;

    use super::*;

    #[test]
    fn a_dsn_names_a_server_and_a_project() {
        assert!(ErrorReports::new("https://key@sentry.example.com/42", None).is_ok());
        for wrong in ["", "sentry.example.com", "https://sentry.example.com/42"] {
            assert!(
                matches!(
                    ErrorReports::new(wrong, None),
                    Err(LogsError::InvalidDsn(_))
                ),
                "{wrong}"
            );
        }
    }

    #[test]
    fn an_error_record_is_an_event_with_the_keys_of_its_spans() {
        let subscriber = tracing_subscriber::registry().with(layer());

        let events = sentry::test::with_captured_events(|| {
            tracing::subscriber::with_default(subscriber, || {
                let _job = tracing::info_span!("job", org_id = "org_1").entered();
                let _scan = tracing::info_span!("scan", queue_id = "queue_1").entered();
                tracing::warn!("job.retried");
                tracing::error!("job.failed");
            });
        });

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].message.as_deref(), Some("job.failed"));
        let Some(Context::Other(fields)) = events[0].contexts.get("Rust Tracing Fields") else {
            panic!("the event has no fields of spans");
        };
        assert_eq!(fields["job:org_id"], "org_1");
        assert_eq!(fields["scan:queue_id"], "queue_1");
    }
}

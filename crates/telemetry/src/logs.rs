use std::error::Error;
use std::fmt;

use serde_json::{Map, Value};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::fmt::format::{JsonFields, Writer};
use tracing_subscriber::fmt::time::UtcTime;
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields, FormattedFields};
use tracing_subscriber::registry::LookupSpan;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::ParseError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogFormat {
    /// One JSON object per line, for a log store.
    Json,
    /// One colored line of text, for a terminal.
    Pretty,
}

#[derive(Debug, thiserror::Error)]
pub enum LogsError {
    #[error("invalid log filter")]
    InvalidFilter(#[source] ParseError),
    #[error("a global subscriber is already installed")]
    AlreadyInstalled(#[source] Box<dyn Error + Send + Sync>),
}

/// The settings of the logs, checked before they are installed.
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

    /// Installs the global subscriber (`docs/OBSERVABILITY.md`).
    ///
    /// # Errors
    ///
    /// A global subscriber is already installed.
    pub fn init(self) -> Result<(), LogsError> {
        let builder = tracing_subscriber::fmt().with_env_filter(self.filter);
        match self.format {
            LogFormat::Json => builder
                .fmt_fields(JsonFields::new())
                .event_format(FlatJson)
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

/// Writes one JSON object per line, with the fields of the open spans at the
/// top level next to the fields of the event. The standard JSON format of
/// `tracing-subscriber` nests them in a `spans` list instead.
struct FlatJson;

impl<S, N> FormatEvent<S, N> for FlatJson
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        #[expect(
            clippy::disallowed_methods,
            reason = "the time of a log line is read where the line is written"
        )]
        let now = OffsetDateTime::now_utc();
        let mut record = Map::new();
        record.insert("timestamp".into(), timestamp(now).into());
        record.insert("level".into(), event.metadata().level().as_str().into());
        record.insert("target".into(), event.metadata().target().into());
        if let Some(scope) = ctx.event_scope() {
            for span in scope.from_root() {
                let extensions = span.extensions();
                let Some(fields) = extensions.get::<FormattedFields<N>>() else {
                    continue;
                };
                if let Ok(Value::Object(fields)) = serde_json::from_str::<Value>(fields) {
                    record.extend(fields);
                }
            }
        }
        event.record(&mut Fields(&mut record));
        writeln!(writer, "{}", Value::Object(record))
    }
}

fn timestamp(now: OffsetDateTime) -> String {
    let millis = now.replace_nanosecond(u32::from(now.millisecond()) * 1_000_000);
    millis.unwrap_or(now).format(&Rfc3339).unwrap_or_default()
}

struct Fields<'a>(&'a mut Map<String, Value>);

impl Visit for Fields<'_> {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.insert(field.name().into(), value.into());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.0.insert(field.name().into(), value.into());
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.0.insert(field.name().into(), value.into());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.0.insert(field.name().into(), value.into());
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.0.insert(field.name().into(), value.into());
    }

    /// Writes the chain of causes: `outer: inner: root`.
    fn record_error(&mut self, field: &Field, value: &(dyn Error + 'static)) {
        let mut text = value.to_string();
        let mut source = value.source();
        while let Some(cause) = source {
            text.push_str(": ");
            text.push_str(&cause.to_string());
            source = cause.source();
        }
        self.0.insert(field.name().into(), text.into());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.0
            .insert(field.name().into(), format!("{value:?}").into());
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::sync::{Arc, Mutex};

    use time::{Date, Month};
    use tracing::{info, info_span, warn};
    use tracing_subscriber::fmt::MakeWriter;

    use super::*;

    fn subscriber<W>(writer: W) -> impl Subscriber
    where
        W: for<'w> MakeWriter<'w> + Send + Sync + 'static,
    {
        tracing_subscriber::fmt()
            .fmt_fields(JsonFields::new())
            .event_format(FlatJson)
            .with_writer(writer)
            .finish()
    }

    #[derive(Clone, Default)]
    struct Buffer(Arc<Mutex<Vec<u8>>>);

    impl io::Write for Buffer {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn lines(write: impl FnOnce()) -> Vec<Map<String, Value>> {
        let buffer = Buffer::default();
        let make = buffer.clone();
        tracing::subscriber::with_default(subscriber(move || make.clone()), write);
        let text = String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap();
        text.lines()
            .map(|line| match serde_json::from_str(line).unwrap() {
                Value::Object(map) => map,
                other => panic!("not an object: {other}"),
            })
            .collect()
    }

    #[test]
    fn span_fields_are_at_the_top_level() {
        let lines = lines(|| {
            let _role = info_span!("role", role = "scheduler").entered();
            let _queue = info_span!("reconcile", queue_id = "queue_1").entered();
            info!(predecessors = 2_u64, "group formed");
        });
        let [line] = lines.as_slice() else {
            panic!("{lines:?}")
        };
        assert_eq!(line["message"], "group formed");
        assert_eq!(line["level"], "INFO");
        assert_eq!(line["target"], module_path!());
        assert_eq!(line["role"], "scheduler");
        assert_eq!(line["queue_id"], "queue_1");
        assert_eq!(line["predecessors"], 2);
        assert!(line.contains_key("timestamp"));
        assert!(!line.contains_key("spans"));
    }

    #[derive(Debug, thiserror::Error)]
    #[error("outer")]
    struct Outer(#[source] Root);

    #[derive(Debug, thiserror::Error)]
    #[error("root")]
    struct Root;

    #[test]
    fn errors_keep_their_causes() {
        let err = Outer(Root);
        let lines = lines(|| warn!(error = &err as &dyn Error, "github call retried"));
        assert_eq!(lines[0]["error"], "outer: root");
    }

    #[test]
    fn invalid_filters_are_refused() {
        assert!(matches!(
            Logs::new("info,=[", LogFormat::Json),
            Err(LogsError::InvalidFilter(_))
        ));
        assert!(Logs::new("info,cihub_github=debug", LogFormat::Pretty).is_ok());
    }

    #[test]
    fn timestamps_are_rfc_3339_in_milliseconds() {
        let now = Date::from_calendar_date(2026, Month::September, 29)
            .unwrap()
            .with_hms_nano(14, 3, 12, 481_123_456)
            .unwrap()
            .assume_utc();
        assert_eq!(timestamp(now), "2026-09-29T14:03:12.481Z");
    }
}

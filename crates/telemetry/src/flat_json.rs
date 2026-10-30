use std::fmt;

use serde_json::{Map, Value};
use time::format_description::BorrowedFormatItem;
use time::macros::format_description;
use tracing::{Event, Subscriber};
use tracing_subscriber::fmt::format::{JsonFields, Writer};
use tracing_subscriber::fmt::time::{FormatTime, UtcTime};
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields, FormattedFields};
use tracing_subscriber::registry::LookupSpan;

/// The `json` format of `specs/OBSERVABILITY.md`: one flat object per line,
/// with the fields of the open spans next to those of the event, because
/// the `json` parser of Loki skips the arrays.
pub(crate) struct FlatJson {
    timer: UtcTime<&'static [BorrowedFormatItem<'static>]>,
}

impl FlatJson {
    pub(crate) fn new() -> Self {
        let timer = UtcTime::new(format_description!(
            "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
        ));
        Self { timer }
    }
}

impl<S> FormatEvent<S, JsonFields> for FlatJson
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn format_event(
        &self,
        context: &FmtContext<'_, S, JsonFields>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let mut record = span_fields(context);
        let mut event_fields = String::new();
        context.format_fields(Writer::new(&mut event_fields), event)?;
        record.extend(object(&event_fields));
        let mut timestamp = String::new();
        self.timer.format_time(&mut Writer::new(&mut timestamp))?;

        let metadata = event.metadata();
        record.insert("timestamp".to_owned(), Value::from(timestamp));
        record.insert("level".to_owned(), Value::from(metadata.level().as_str()));
        record.insert("target".to_owned(), Value::from(metadata.target()));
        writeln!(writer, "{}", Value::Object(record))
    }
}

/// The fields of the open spans, from the root to the leaf: the span closest
/// to the event wins.
fn span_fields<S>(context: &FmtContext<'_, S, JsonFields>) -> Map<String, Value>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let mut fields = Map::new();
    let Some(scope) = context.event_scope() else {
        return fields;
    };

    for span in scope.from_root() {
        if let Some(written) = span.extensions().get::<FormattedFields<JsonFields>>() {
            fields.extend(object(written));
        }
    }
    fields
}

/// The fields that `JsonFields` wrote as one object; none when it wrote
/// nothing, as for a span without fields.
fn object(written: &str) -> Map<String, Value> {
    match serde_json::from_str(written) {
        Ok(Value::Object(fields)) => fields,
        _ => Map::new(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tracing::field::Empty;
    use tracing::{info, info_span};

    use crate::CapturedLogs;

    #[test]
    fn the_fields_of_the_spans_are_at_the_top_level() {
        let logs = CapturedLogs::default();

        tracing::subscriber::with_default(logs.subscriber(), || {
            let _role = info_span!("role", role = "worker", queue_id = "queue_root").entered();
            let job = info_span!("job", queue_id = "queue_leaf", pr_number = Empty).entered();
            job.record("pr_number", 4821_u64);
            info!(
                reason = "permission",
                pr_number = 7_u64,
                "submission.refused"
            );
        });

        let records = logs.records().unwrap();
        let [record] = records.as_slice() else {
            panic!("one record expected, got {records:?}");
        };
        assert_eq!(record["message"], json!("submission.refused"));
        assert_eq!(record["level"], json!("INFO"));
        assert_eq!(record["reason"], json!("permission"));
        assert_eq!(record["role"], json!("worker"));
        assert_eq!(
            record["queue_id"],
            json!("queue_leaf"),
            "the closest span wins"
        );
        assert_eq!(record["pr_number"], json!(7), "the event wins");
        assert!(record.values().all(|value| !value.is_array()));
    }

    #[test]
    fn a_record_has_its_time_in_milliseconds_its_level_and_its_target() {
        let logs = CapturedLogs::default();

        tracing::subscriber::with_default(logs.subscriber(), || {
            info!(duration_ms = 3_u64, "job.done");
        });

        let records = logs.records().unwrap();
        let [record] = records.as_slice() else {
            panic!("one record expected, got {records:?}");
        };
        let timestamp = record["timestamp"].as_str().unwrap();
        assert_eq!(
            timestamp.len(),
            "2026-09-29T14:03:12.481Z".len(),
            "{timestamp}"
        );
        assert!(timestamp.ends_with('Z'), "{timestamp}");
        assert_eq!(record["level"], json!("INFO"));
        assert_eq!(record["target"], json!(module_path!()));
        assert_eq!(record["message"], json!("job.done"));
        assert_eq!(record["duration_ms"], json!(3));
    }
}

use std::io;
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::{Map, Value};
use tracing::{Level, Subscriber};
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::fmt::format::JsonFields;

use crate::LogsError;
use crate::flat_json::FlatJson;

/// The lines of the `json` format, kept in memory, for a test that checks a
/// log (`specs/OBSERVABILITY.md`, rule 11). Install
/// [`CapturedLogs::subscriber`] with `tracing::subscriber::with_default`.
#[derive(Clone, Debug, Default)]
pub struct CapturedLogs(Arc<Mutex<Vec<u8>>>);

impl CapturedLogs {
    /// A subscriber of every level, in the `json` format of the processes.
    pub fn subscriber(&self) -> impl Subscriber + Send + Sync + 'static {
        tracing_subscriber::fmt()
            .with_max_level(Level::TRACE)
            .fmt_fields(JsonFields::new())
            .event_format(FlatJson::new())
            .with_writer(self.clone())
            .finish()
    }

    /// The records written so far, one object per line.
    ///
    /// # Errors
    ///
    /// A line is not a JSON object: the format is broken.
    pub fn records(&self) -> Result<Vec<Map<String, Value>>, LogsError> {
        let bytes = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        String::from_utf8_lossy(&bytes)
            .lines()
            .map(|line| serde_json::from_str(line).map_err(LogsError::InvalidRecord))
            .collect()
    }
}

impl<'a> MakeWriter<'a> for CapturedLogs {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

impl io::Write for CapturedLogs {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let mut buffer = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        buffer.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

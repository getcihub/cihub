//! Telemetry of the CIHub processes: the logs, then the
//! OpenTelemetry traces and the Prometheus registry when they arrive. Only the
//! binaries install it; the library crates write with the macros of `tracing`
//! and do not depend on this crate.

mod logs;

pub use logs::{LogFormat, Logs, LogsError};

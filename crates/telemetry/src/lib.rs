//! Telemetry of the CIHub processes: the logs and, when a Sentry server is
//! configured, the error reports; then the OpenTelemetry traces and the
//! Prometheus registry when they arrive. Only the
//! binaries install it; the library crates write with the macros of `tracing`
//! and depend on this crate only in their tests, with the feature `capture`,
//! to read their lines with `CapturedLogs`.

#[cfg(any(test, feature = "capture"))]
mod captured;
mod error_reports;
mod flat_json;
mod logs;

#[cfg(any(test, feature = "capture"))]
pub use captured::CapturedLogs;
pub use error_reports::{ErrorReports, RELEASE, check_dsn};
pub use logs::{ErrorReportsGuard, LogFormat, Logs, LogsError};

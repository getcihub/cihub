use axum::Json;
use axum::extract::State;
use cihub_analytics::{Health, KEPT, Period, Verdict, queue_health};
use cihub_api_types::ErrorCode;
use cihub_api_types::{
    CheckHealth, HealthBucket, HealthTotals, HealthVerdict, QueueHealth, ReasonCount, StateTime,
    TimeToMerge,
};
use cihub_domain::{ApiScope, QueueId};
use cihub_platform::{ApiError, ApiGrant, ApiPath, ApiQuery};
use serde::Deserialize;
use time::{Duration, OffsetDateTime};

use super::queues::begin_queue;
use super::{ApiState, QueueErrors, QueuePath};

/// The period of the Health of a queue.
#[derive(Clone, Copy, Default, Deserialize, utoipa::ToSchema)]
pub(super) enum HealthRange {
    /// The last 24 hours, by hour.
    #[default]
    #[serde(rename = "24h")]
    Day,
    /// The last 7 days, by UTC day.
    #[serde(rename = "7d")]
    Week,
    /// The last 30 days, by UTC day.
    #[serde(rename = "30d")]
    Month,
}

impl HealthRange {
    const fn period(self) -> Period {
        match self {
            Self::Day => Period::Last {
                bucket: Duration::HOUR,
                buckets: 24,
            },
            Self::Week => Period::Last {
                bucket: Duration::DAY,
                buckets: 7,
            },
            Self::Month => Period::Last {
                bucket: Duration::DAY,
                buckets: 30,
            },
        }
    }
}

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct HealthQuery {
    /// `24h` (by default), `7d` or `30d`; not with `from` and `to`.
    #[param(inline)]
    range: Option<HealthRange>,
    /// The start of a period instead of `range`, with `to` (RFC 3339): from
    /// its UTC hour, in the last 13 months.
    #[serde(default, with = "time::serde::rfc3339::option")]
    #[param(value_type = Option<String>, format = DateTime)]
    from: Option<OffsetDateTime>,
    /// The end of the period, with `from` (RFC 3339): to the end of its UTC
    /// hour, or of the current hour. Its buckets are hours up to 2 days, UTC
    /// days up to 90 days, and weeks from a Monday beyond.
    #[serde(default, with = "time::serde::rfc3339::option")]
    #[param(value_type = Option<String>, format = DateTime)]
    to: Option<OffsetDateTime>,
}

impl HealthQuery {
    fn period(&self, now: OffsetDateTime) -> Result<Period, ApiError> {
        let invalid = |message: &str| ApiError::new(ErrorCode::InvalidRequest, message.into());
        match (self.range, self.from, self.to) {
            (range, None, None) => Ok(range.unwrap_or_default().period()),
            (Some(_), _, _) => Err(invalid("`range` or `from` and `to`, not both")),
            (None, Some(from), Some(to)) => between(from, to, now).map_err(invalid),
            (None, _, _) => Err(invalid("`from` and `to` go together")),
        }
    }
}

fn between(
    from: OffsetDateTime,
    to: OffsetDateTime,
    now: OffsetDateTime,
) -> Result<Period, &'static str> {
    if to <= from {
        return Err("`to` is not after `from`");
    }
    if from >= now {
        return Err("`from` is in the future");
    }
    if from < now - KEPT {
        return Err("`from` is older than 13 months");
    }

    Ok(Period::Between { from, to })
}

/// Get the Health of a queue
///
/// What the queue did over the period: the PRs that merged and failed, their
/// time to merge, why the others left, and the checks of its runs, each
/// against the period before: the last 24 hours, 7 days or 30 days, or the
/// hours between two times. Read from the hourly aggregate of the facts of the
/// log, which arrive a few seconds after the log; the medians and the 90th
/// percentiles are within 5 %.
#[utoipa::path(
    get,
    path = "/orgs/{org}/repos/{repo}/queues/{queue}/health",
    tag = "Queues",
    params(QueuePath, HealthQuery),
    responses((status = 200, description = "The Health of the queue.", body = QueueHealth), QueueErrors),
    security(("token" = ["queues:read"]))
)]
pub(super) async fn get_health(
    grant: ApiGrant,
    State(state): State<ApiState>,
    ApiPath(path): ApiPath<QueuePath>,
    ApiQuery(query): ApiQuery<HealthQuery>,
) -> Result<Json<QueueHealth>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;
    let now = (state.clock)();
    let period = query.period(now)?;

    let (mut tx, queue) = begin_queue(&state, &grant, &path).await?;
    let queue: QueueId = queue.id.parse()?;
    let health = queue_health(&mut tx, queue, period, now).await?;

    Ok(Json(response(health)))
}

fn response(health: Health) -> QueueHealth {
    QueueHealth {
        from: health.from,
        to: health.to,
        verdict: match health.verdict {
            Verdict::Quiet => HealthVerdict::Quiet,
            Verdict::Healthy => HealthVerdict::Healthy,
            Verdict::ManyFailures => HealthVerdict::ManyFailures,
            Verdict::CheckPassesOnRetry { check, times } => {
                HealthVerdict::CheckPassesOnRetry { check, times }
            }
        },
        current: totals(health.current),
        previous: totals(health.previous),
        buckets: health
            .buckets
            .into_iter()
            .map(|bucket| HealthBucket {
                start: bucket.start,
                merged: bucket.merged,
                failed: bucket.failed,
                time_to_merge: bucket.time_to_merge.map(time_to_merge),
            })
            .collect(),
        time_in_states: health
            .time_in_states
            .into_iter()
            .map(|(state, median_seconds)| StateTime {
                state,
                median_seconds,
            })
            .collect(),
        left_without_merge: health
            .left_without_merge
            .into_iter()
            .map(|(conclusion, reason, count)| ReasonCount {
                conclusion,
                reason,
                description: reason.describe().to_owned(),
                count,
            })
            .collect(),
        checks: health
            .checks
            .into_iter()
            .map(|check| CheckHealth {
                name: check.name,
                kind: check.kind,
                required: check.required,
                runs: check.runs,
                failed: check.failed,
                passed_on_retry: check.passed_on_retry,
            })
            .collect(),
    }
}

fn totals(totals: cihub_analytics::Totals) -> HealthTotals {
    HealthTotals {
        merged: totals.merged,
        failed: totals.failed,
        cancelled: totals.cancelled,
        time_to_merge: totals.time_to_merge.map(time_to_merge),
    }
}

fn time_to_merge(time: cihub_analytics::TimeToMerge) -> TimeToMerge {
    TimeToMerge {
        median_seconds: time.median,
        p90_seconds: time.p90,
    }
}

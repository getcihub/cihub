use axum::Json;
use axum::extract::State;
use cihub_analytics::{Health, Period, Verdict, queue_health};
use cihub_api_types::{
    CheckHealth, HealthBucket, HealthTotals, HealthVerdict, QueueHealth, ReasonCount, StateTime,
    TimeToMerge,
};
use cihub_domain::{ApiScope, QueueId};
use cihub_platform::{ApiError, ApiGrant, ApiPath, ApiQuery};
use serde::Deserialize;
use time::Duration;

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
            Self::Day => Period {
                bucket: Duration::HOUR,
                buckets: 24,
            },
            Self::Week => Period {
                bucket: Duration::DAY,
                buckets: 7,
            },
            Self::Month => Period {
                bucket: Duration::DAY,
                buckets: 30,
            },
        }
    }
}

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct HealthQuery {
    /// `24h` (by default), `7d` or `30d`.
    #[serde(default)]
    #[param(inline)]
    range: HealthRange,
}

/// Get the Health of a queue
///
/// What the queue did over the period: the PRs that merged and failed, their
/// time to merge, why the others left, and the checks of its runs, each
/// against the period before. Read from the facts of the log, which arrive a
/// few seconds after the log.
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

    let (mut tx, queue) = begin_queue(&state, &grant, &path).await?;
    let queue: QueueId = queue.id.parse()?;
    let health = queue_health(&mut tx, queue, query.range.period(), (state.clock)()).await?;

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

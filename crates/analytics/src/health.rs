use cihub_domain::{CheckKind, ItemState, QueueId, Reason};
use sqlx::PgConnection;
use time::{Duration, OffsetDateTime};

use crate::AnalyticsError;
use crate::hourly::{LIVE_STATES, TIME_TO_MERGE, percentile};

/// A check that passed on a retry this many times in a period looks flaky.
const FLAKY_RETRIES: u64 = 3;

/// Above this share of failed PRs, a queue fails too often.
const MANY_FAILURES: f64 = 0.15;

/// A period of `buckets` buckets of `bucket` each; the last bucket holds
/// `now`. The buckets start on a multiple of their size since the Unix epoch:
/// an hour, or a day in UTC.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Period {
    pub bucket: Duration,
    pub buckets: u32,
}

impl Period {
    /// The start and the end of the period that holds `now`.
    fn bounds(self, now: OffsetDateTime) -> (OffsetDateTime, OffsetDateTime) {
        let size = self.bucket.whole_seconds().max(1);
        let elapsed = now.unix_timestamp().rem_euclid(size);
        let end =
            now.replace_nanosecond(0).unwrap_or(now) - Duration::seconds(elapsed) + self.bucket;
        (end - self.bucket * self.buckets, end)
    }
}

/// What a queue did over a period, and over the period before.
#[derive(Clone, Debug, PartialEq)]
pub struct Health {
    pub from: OffsetDateTime,
    pub to: OffsetDateTime,
    pub verdict: Verdict,
    pub current: Totals,
    pub previous: Totals,
    pub buckets: Vec<Bucket>,
    /// The median time that a merged PR spent in each state.
    pub time_in_states: Vec<(ItemState, u64)>,
    /// The end and the reason of the PRs that ended without a merge, the most
    /// frequent first.
    pub left_without_merge: Vec<(ItemState, Reason, u64)>,
    pub checks: Vec<CheckHealth>,
}

/// The first thing that a person should know about a queue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// No PR ended in the period.
    Quiet,
    Healthy,
    /// More than 15 % of the PRs that ended failed.
    ManyFailures,
    /// `check` failed, then passed on the next attempt, `times` times.
    CheckPassesOnRetry {
        check: String,
        times: u64,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Totals {
    pub merged: u64,
    pub failed: u64,
    pub cancelled: u64,
    pub time_to_merge: Option<TimeToMerge>,
}

/// From the submission to the merge, in seconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeToMerge {
    pub median: u64,
    pub p90: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bucket {
    pub start: OffsetDateTime,
    pub merged: u64,
    pub failed: u64,
    pub time_to_merge: Option<TimeToMerge>,
}

/// A check on the runs of the period that ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckHealth {
    pub name: String,
    pub kind: CheckKind,
    pub required: bool,
    pub runs: u64,
    pub failed: u64,
    /// The runs where it passed after it failed on the attempt before.
    pub passed_on_retry: u64,
}

/// The Health of `queue` over the `period` that holds `now`.
///
/// # Errors
///
/// Postgres fails, or a row does not match `cihub-domain`.
pub async fn queue_health(
    conn: &mut PgConnection,
    queue: QueueId,
    period: Period,
    now: OffsetDateTime,
) -> Result<Health, AnalyticsError> {
    let (from, to) = period.bounds(now);
    let before = from - (to - from);
    let queue = queue.to_string();

    let current = totals(conn, &queue, (from, to)).await?;
    let previous = totals(conn, &queue, (before, from)).await?;
    let buckets = buckets(conn, &queue, (from, to), period.bucket).await?;
    let time_in_states = time_in_states(conn, &queue, (from, to)).await?;
    let left_without_merge = left_without_merge(conn, &queue, (from, to)).await?;
    let checks = checks(conn, &queue, (from, to)).await?;

    Ok(Health {
        from,
        to,
        verdict: verdict(&current, &checks),
        current,
        previous,
        buckets,
        time_in_states,
        left_without_merge,
        checks,
    })
}

fn verdict(current: &Totals, checks: &[CheckHealth]) -> Verdict {
    let ended = current.merged + current.failed;
    if ended == 0 {
        return Verdict::Quiet;
    }

    let flaky = checks
        .iter()
        .filter(|check| check.passed_on_retry >= FLAKY_RETRIES)
        .max_by_key(|check| check.passed_on_retry);
    if let Some(check) = flaky {
        return Verdict::CheckPassesOnRetry {
            check: check.name.clone(),
            times: check.passed_on_retry,
        };
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "a share: the counts of a queue stay far below 2^52"
    )]
    let share = current.failed as f64 / ended as f64;
    if share > MANY_FAILURES {
        Verdict::ManyFailures
    } else {
        Verdict::Healthy
    }
}

/// A count of the aggregate, never negative.
fn count(value: i64) -> u64 {
    value.unsigned_abs()
}

/// A histogram of durations: its buckets in their order, and the count of
/// each.
type Histogram = Vec<(i16, u64)>;

fn time_to_merge(histogram: &[(i16, u64)]) -> Option<TimeToMerge> {
    Some(TimeToMerge {
        median: percentile(histogram, 0.5)?,
        p90: percentile(histogram, 0.9)?,
    })
}

// Every read below sums the hourly aggregate: its cost follows the length of
// the period, not the number of PRs of the queue.

async fn totals(
    conn: &mut PgConnection,
    queue: &str,
    (from, to): (OffsetDateTime, OffsetDateTime),
) -> Result<Totals, AnalyticsError> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT conclusion::text, sum(count)::bigint FROM an_hourly_conclusions
         WHERE queue_id = $1 AND hour >= $2 AND hour < $3
         GROUP BY conclusion",
    )
    .bind(queue)
    .bind(from)
    .bind(to)
    .fetch_all(&mut *conn)
    .await?;
    let histogram = histogram(conn, queue, TIME_TO_MERGE, (from, to)).await?;

    let of = |conclusion: ItemState| {
        rows.iter()
            .find(|(found, _)| found == conclusion.as_str())
            .map_or(0, |(_, total)| count(*total))
    };
    Ok(Totals {
        merged: of(ItemState::Merged),
        failed: of(ItemState::Failed),
        cancelled: of(ItemState::Cancelled),
        time_to_merge: time_to_merge(&histogram),
    })
}

async fn histogram(
    conn: &mut PgConnection,
    queue: &str,
    measure: &str,
    (from, to): (OffsetDateTime, OffsetDateTime),
) -> Result<Histogram, AnalyticsError> {
    let rows: Vec<(i16, i64)> = sqlx::query_as(
        "SELECT bucket, sum(count)::bigint FROM an_hourly_durations
         WHERE queue_id = $1 AND measure = $2 AND hour >= $3 AND hour < $4
         GROUP BY bucket
         ORDER BY bucket",
    )
    .bind(queue)
    .bind(measure)
    .bind(from)
    .bind(to)
    .fetch_all(&mut *conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(bucket, total)| (bucket, count(total)))
        .collect())
}

async fn buckets(
    conn: &mut PgConnection,
    queue: &str,
    (from, to): (OffsetDateTime, OffsetDateTime),
    size: Duration,
) -> Result<Vec<Bucket>, AnalyticsError> {
    let ended: Vec<(OffsetDateTime, String, i64)> = sqlx::query_as(
        "SELECT date_bin($4 * interval '1 second', hour, 'epoch'), conclusion::text,
                sum(count)::bigint
         FROM an_hourly_conclusions
         WHERE queue_id = $1 AND hour >= $2 AND hour < $3 AND conclusion IN ('merged', 'failed')
         GROUP BY 1, 2",
    )
    .bind(queue)
    .bind(from)
    .bind(to)
    .bind(size.whole_seconds())
    .fetch_all(&mut *conn)
    .await?;
    let durations: Vec<(OffsetDateTime, i16, i64)> = sqlx::query_as(
        "SELECT date_bin($4 * interval '1 second', hour, 'epoch'), bucket, sum(count)::bigint
         FROM an_hourly_durations
         WHERE queue_id = $1 AND measure = $5 AND hour >= $2 AND hour < $3
         GROUP BY 1, 2
         ORDER BY 1, 2",
    )
    .bind(queue)
    .bind(from)
    .bind(to)
    .bind(size.whole_seconds())
    .bind(TIME_TO_MERGE)
    .fetch_all(&mut *conn)
    .await?;

    let mut buckets = Vec::new();
    let mut start = from;
    while start < to {
        let of = |conclusion: ItemState| {
            ended
                .iter()
                .find(|(at, found, _)| *at == start && found == conclusion.as_str())
                .map_or(0, |(_, _, total)| count(*total))
        };
        let histogram: Histogram = durations
            .iter()
            .filter(|(at, _, _)| *at == start)
            .map(|(_, bucket, total)| (*bucket, count(*total)))
            .collect();
        buckets.push(Bucket {
            start,
            merged: of(ItemState::Merged),
            failed: of(ItemState::Failed),
            time_to_merge: time_to_merge(&histogram),
        });
        start += size;
    }
    Ok(buckets)
}

/// The median time in each live state of the merged PRs; none without a
/// merged PR.
async fn time_in_states(
    conn: &mut PgConnection,
    queue: &str,
    (from, to): (OffsetDateTime, OffsetDateTime),
) -> Result<Vec<(ItemState, u64)>, AnalyticsError> {
    let rows: Vec<(String, i16, i64)> = sqlx::query_as(
        "SELECT measure, bucket, sum(count)::bigint FROM an_hourly_durations
         WHERE queue_id = $1 AND measure <> $4 AND hour >= $2 AND hour < $3
         GROUP BY measure, bucket
         ORDER BY measure, bucket",
    )
    .bind(queue)
    .bind(from)
    .bind(to)
    .bind(TIME_TO_MERGE)
    .fetch_all(&mut *conn)
    .await?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    Ok(LIVE_STATES
        .iter()
        .map(|state| {
            let histogram: Histogram = rows
                .iter()
                .filter(|(measure, _, _)| measure == state.as_str())
                .map(|(_, bucket, total)| (*bucket, count(*total)))
                .collect();
            (*state, percentile(&histogram, 0.5).unwrap_or(0))
        })
        .collect())
}

async fn left_without_merge(
    conn: &mut PgConnection,
    queue: &str,
    (from, to): (OffsetDateTime, OffsetDateTime),
) -> Result<Vec<(ItemState, Reason, u64)>, AnalyticsError> {
    let rows: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT conclusion::text, reason::text, sum(count)::bigint FROM an_hourly_conclusions
         WHERE queue_id = $1 AND conclusion <> 'merged' AND hour >= $2 AND hour < $3
         GROUP BY conclusion, reason
         ORDER BY sum(count) DESC, conclusion::text, reason::text",
    )
    .bind(queue)
    .bind(from)
    .bind(to)
    .fetch_all(&mut *conn)
    .await?;
    rows.into_iter()
        .map(|(conclusion, reason, total)| Ok((read(&conclusion)?, read(&reason)?, count(total))))
        .collect()
}

async fn checks(
    conn: &mut PgConnection,
    queue: &str,
    (from, to): (OffsetDateTime, OffsetDateTime),
) -> Result<Vec<CheckHealth>, AnalyticsError> {
    type Row = (String, String, bool, i64, i64, i64);
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT name, kind::text, bool_or(required), sum(runs)::bigint, sum(failed)::bigint,
                sum(passed_on_retry)::bigint
         FROM an_hourly_checks
         WHERE queue_id = $1 AND hour >= $2 AND hour < $3
         GROUP BY name, kind
         ORDER BY name, kind",
    )
    .bind(queue)
    .bind(from)
    .bind(to)
    .fetch_all(&mut *conn)
    .await?;
    rows.into_iter()
        .map(|(name, kind, required, runs, failed, passed_on_retry)| {
            Ok(CheckHealth {
                kind: read(&kind)?,
                name,
                required,
                runs: count(runs),
                failed: count(failed),
                passed_on_retry: count(passed_on_retry),
            })
        })
        .collect()
}

fn read<T: std::str::FromStr>(value: &str) -> Result<T, AnalyticsError>
where
    T::Err: std::error::Error + Send + Sync + 'static,
{
    value
        .parse()
        .map_err(|source: T::Err| AnalyticsError::ReadValue {
            value: value.to_owned(),
            source: Box::new(source),
        })
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::*;

    fn check(name: &str, passed_on_retry: u64) -> CheckHealth {
        CheckHealth {
            name: name.into(),
            kind: CheckKind::Check,
            required: true,
            runs: 10,
            failed: passed_on_retry,
            passed_on_retry,
        }
    }

    fn totals(merged: u64, failed: u64) -> Totals {
        Totals {
            merged,
            failed,
            ..Totals::default()
        }
    }

    #[test]
    fn the_last_bucket_holds_now() {
        let hours = Period {
            bucket: Duration::HOUR,
            buckets: 24,
        };
        let days = Period {
            bucket: Duration::DAY,
            buckets: 7,
        };
        let now = datetime!(2026-10-14 09:41:12.5 UTC);

        assert_eq!(
            hours.bounds(now),
            (
                datetime!(2026-10-13 10:00 UTC),
                datetime!(2026-10-14 10:00 UTC)
            )
        );
        assert_eq!(
            days.bounds(now),
            (
                datetime!(2026-10-08 00:00 UTC),
                datetime!(2026-10-15 00:00 UTC)
            )
        );
    }

    #[test]
    fn a_queue_without_an_ended_pr_is_quiet() {
        assert_eq!(verdict(&totals(0, 0), &[check("test", 5)]), Verdict::Quiet);
    }

    #[test]
    fn the_check_that_passes_most_on_a_retry_comes_first() {
        let checks = [check("lint", 3), check("test", 5), check("build", 2)];

        assert_eq!(
            verdict(&totals(10, 9), &checks),
            Verdict::CheckPassesOnRetry {
                check: "test".into(),
                times: 5
            }
        );
    }

    #[test]
    fn a_queue_fails_too_often_above_15_percent() {
        let checks = [check("test", 2)];

        assert_eq!(verdict(&totals(17, 3), &checks), Verdict::Healthy);
        assert_eq!(verdict(&totals(16, 4), &checks), Verdict::ManyFailures);
    }
}

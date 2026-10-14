use cihub_domain::{CheckConclusion, CheckKind, ItemState, QueueId, Reason};
use sqlx::PgConnection;
use strum::IntoEnumIterator;
use time::{Duration, OffsetDateTime};

use crate::AnalyticsError;

/// A check that passed on a retry this many times in a period looks flaky.
const FLAKY_RETRIES: u64 = 3;

/// Above this share of failed PRs, a queue fails too often.
const MANY_FAILURES: f64 = 0.15;

/// The states that a merged PR passes through, in their order.
const LIVE_STATES: [ItemState; 5] = [
    ItemState::NotReady,
    ItemState::Pending,
    ItemState::Testing,
    ItemState::PendingFailure,
    ItemState::TestsPassed,
];

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

/// The number of rows that `count(*)` gives, never negative.
fn count(value: i64) -> u64 {
    value.unsigned_abs()
}

/// A duration in seconds; a clock that went back counts as 0.
fn seconds(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0)
}

fn time_to_merge(median: Option<i64>, p90: Option<i64>) -> Option<TimeToMerge> {
    Some(TimeToMerge {
        median: seconds(median?),
        p90: seconds(p90?),
    })
}

/// The time to merge of the merged PRs of `c`, in whole seconds.
const TIME_TO_MERGE: &str = "
    round(percentile_cont(0.5) WITHIN GROUP (ORDER BY extract(epoch FROM c.concluded_at - c.submitted_at))
          FILTER (WHERE c.conclusion = 'merged'))::bigint,
    round(percentile_cont(0.9) WITHIN GROUP (ORDER BY extract(epoch FROM c.concluded_at - c.submitted_at))
          FILTER (WHERE c.conclusion = 'merged'))::bigint";

async fn totals(
    conn: &mut PgConnection,
    queue: &str,
    (from, to): (OffsetDateTime, OffsetDateTime),
) -> Result<Totals, AnalyticsError> {
    let (merged, failed, cancelled, median, p90): (i64, i64, i64, Option<i64>, Option<i64>) =
        sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT count(*) FILTER (WHERE c.conclusion = 'merged'),
                    count(*) FILTER (WHERE c.conclusion = 'failed'),
                    count(*) FILTER (WHERE c.conclusion = 'cancelled'),
                    {TIME_TO_MERGE}
             FROM an_item_conclusions c
             WHERE c.queue_id = $1 AND c.concluded_at >= $2 AND c.concluded_at < $3"
        )))
        .bind(queue)
        .bind(from)
        .bind(to)
        .fetch_one(&mut *conn)
        .await?;
    Ok(Totals {
        merged: count(merged),
        failed: count(failed),
        cancelled: count(cancelled),
        time_to_merge: time_to_merge(median, p90),
    })
}

async fn buckets(
    conn: &mut PgConnection,
    queue: &str,
    (from, to): (OffsetDateTime, OffsetDateTime),
    size: Duration,
) -> Result<Vec<Bucket>, AnalyticsError> {
    type Row = (OffsetDateTime, i64, i64, Option<i64>, Option<i64>);
    let rows: Vec<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT b.start,
                count(c.item_id) FILTER (WHERE c.conclusion = 'merged'),
                count(c.item_id) FILTER (WHERE c.conclusion = 'failed'),
                {TIME_TO_MERGE}
         FROM generate_series($2::timestamptz, $3::timestamptz - $4 * interval '1 second',
                              $4 * interval '1 second') AS b (start)
         LEFT JOIN an_item_conclusions c
           ON c.queue_id = $1 AND c.concluded_at >= b.start
          AND c.concluded_at < b.start + $4 * interval '1 second'
         GROUP BY b.start
         ORDER BY b.start"
    )))
    .bind(queue)
    .bind(from)
    .bind(to)
    .bind(size.whole_seconds())
    .fetch_all(&mut *conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(start, merged, failed, median, p90)| Bucket {
            start,
            merged: count(merged),
            failed: count(failed),
            time_to_merge: time_to_merge(median, p90),
        })
        .collect())
}

async fn time_in_states(
    conn: &mut PgConnection,
    queue: &str,
    (from, to): (OffsetDateTime, OffsetDateTime),
) -> Result<Vec<(ItemState, u64)>, AnalyticsError> {
    let states: Vec<&str> = LIVE_STATES.iter().map(|state| state.as_str()).collect();
    // A merged PR that skipped a state spent no time in it: it counts as 0.
    let rows: Vec<(String, Option<i64>)> = sqlx::query_as(
        "SELECT s.state::text,
                round(percentile_cont(0.5) WITHIN GROUP (ORDER BY coalesce(t.seconds, 0)))::bigint
         FROM unnest($4::text[]) WITH ORDINALITY AS s (state, rank)
         CROSS JOIN an_item_conclusions c
         LEFT JOIN LATERAL (
             SELECT sum(extract(epoch FROM d.left_at - d.entered_at)) AS seconds
             FROM an_state_durations d
             WHERE d.item_id = c.item_id AND d.state::text = s.state) t ON true
         WHERE c.queue_id = $1 AND c.conclusion = 'merged'
           AND c.concluded_at >= $2 AND c.concluded_at < $3
         GROUP BY s.state, s.rank
         ORDER BY s.rank",
    )
    .bind(queue)
    .bind(from)
    .bind(to)
    .bind(&states)
    .fetch_all(&mut *conn)
    .await?;
    rows.into_iter()
        .map(|(state, median)| Ok((read(&state)?, seconds(median.unwrap_or_default()))))
        .collect()
}

async fn left_without_merge(
    conn: &mut PgConnection,
    queue: &str,
    (from, to): (OffsetDateTime, OffsetDateTime),
) -> Result<Vec<(ItemState, Reason, u64)>, AnalyticsError> {
    let rows: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT conclusion::text, reason::text, count(*) FROM an_item_conclusions
         WHERE queue_id = $1 AND conclusion <> 'merged'
           AND concluded_at >= $2 AND concluded_at < $3
         GROUP BY conclusion, reason
         ORDER BY count(*) DESC, conclusion::text, reason::text",
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
    let failing: Vec<&str> = CheckConclusion::iter()
        .filter(|conclusion| !conclusion.is_passing())
        .map(CheckConclusion::as_str)
        .collect();
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT k.name, k.kind::text, bool_or(k.required), count(*),
                count(*) FILTER (WHERE k.conclusion::text = ANY ($4)),
                count(*) FILTER (WHERE k.conclusion IS NOT NULL
                                   AND k.conclusion::text <> ALL ($4)
                                   AND EXISTS (
                    SELECT 1 FROM an_test_run_results p
                    JOIN an_check_results pk ON pk.run_id = p.run_id
                    WHERE p.group_id = r.group_id AND p.attempt = r.attempt - 1
                      AND pk.name = k.name AND pk.kind = k.kind
                      AND pk.conclusion::text = ANY ($4)))
         FROM an_check_results k
         JOIN an_test_run_results r ON r.run_id = k.run_id
         WHERE r.queue_id = $1 AND r.ended_at >= $2 AND r.ended_at < $3
         GROUP BY k.name, k.kind
         ORDER BY k.name, k.kind",
    )
    .bind(queue)
    .bind(from)
    .bind(to)
    .bind(&failing)
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

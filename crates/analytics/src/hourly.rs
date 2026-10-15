use std::collections::BTreeMap;

use cihub_domain::{CheckKind, ItemState, OrgId, QueueId, Reason};
use sqlx::PgConnection;
use time::{Duration, OffsetDateTime, UtcOffset};

use crate::AnalyticsError;

/// The states that a merged PR passes through, in their order.
pub(crate) const LIVE_STATES: [ItemState; 5] = [
    ItemState::NotReady,
    ItemState::Pending,
    ItemState::Testing,
    ItemState::PendingFailure,
    ItemState::TestsPassed,
];

/// The measure of the time to merge in `an_hourly_durations`; the other
/// measures are the live states.
pub(crate) const TIME_TO_MERGE: &str = "time_to_merge";

/// Each bucket of a histogram of durations is 10 % wider than the one
/// before it, so that a percentile read from it is within 5 % of the exact
/// one.
const GROWTH: f64 = 1.1;

/// The bucket of a duration: 0 for no time, then `b` for the durations from
/// `GROWTH^(b - 1)` to `GROWTH^b` seconds.
pub(crate) fn duration_bucket(seconds: i64) -> i16 {
    if seconds < 1 {
        return 0;
    }

    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        reason = "the logarithm of an i64 in base 1.1 stays below 460"
    )]
    let bucket = ((seconds as f64).ln() / GROWTH.ln()).floor() as i16;
    bucket + 1
}

/// The duration that a bucket stands for: the geometric middle of its
/// bounds, in seconds.
pub(crate) fn bucket_seconds(bucket: i16) -> u64 {
    if bucket <= 0 {
        return 0;
    }

    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a positive power of 1.1 below 460 fits in a u64"
    )]
    let seconds = GROWTH.powf(f64::from(bucket) - 0.5).round() as u64;
    seconds
}

/// The duration below which `share` of a histogram falls, from its buckets
/// in their order and the count of each. `None` for an empty histogram.
pub(crate) fn percentile(histogram: &[(i16, u64)], share: f64) -> Option<u64> {
    let total: u64 = histogram.iter().map(|(_, count)| count).sum();
    if total == 0 {
        return None;
    }

    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a rank between 1 and the count of a histogram"
    )]
    let rank = (share * total as f64).ceil().max(1.0) as u64;
    let mut seen = 0;
    histogram.iter().find_map(|(bucket, count)| {
        seen += count;
        (seen >= rank).then(|| bucket_seconds(*bucket))
    })
}

/// The start of the UTC hour of `at`.
pub(crate) fn hour_of(at: OffsetDateTime) -> OffsetDateTime {
    let utc = at.to_offset(UtcOffset::UTC);
    utc - Duration::minutes(i64::from(utc.minute()))
        - Duration::seconds(i64::from(utc.second()))
        - Duration::nanoseconds(i64::from(utc.nanosecond()))
}

#[derive(Default)]
struct CheckCounts {
    required: bool,
    runs: i64,
    failed: i64,
    passed_on_retry: i64,
}

/// What a page of facts adds to the hourly aggregate of its queue, by hour.
/// Only the facts that the page wrote count: a fact already written, after a
/// cursor read the log again, was counted then.
#[derive(Default)]
pub(crate) struct Increments {
    conclusions: BTreeMap<(OffsetDateTime, &'static str, &'static str), i64>,
    durations: BTreeMap<(OffsetDateTime, &'static str, i16), i64>,
    checks: BTreeMap<(OffsetDateTime, String, &'static str), CheckCounts>,
}

impl Increments {
    pub(crate) fn add_conclusion(
        &mut self,
        concluded_at: OffsetDateTime,
        conclusion: ItemState,
        reason: Reason,
    ) {
        let key = (hour_of(concluded_at), conclusion.as_str(), reason.as_str());
        *self.conclusions.entry(key).or_default() += 1;
    }

    /// Counts a duration of a merged PR in the hour of its merge.
    pub(crate) fn add_duration(
        &mut self,
        merged_at: OffsetDateTime,
        measure: &'static str,
        seconds: i64,
    ) {
        let key = (hour_of(merged_at), measure, duration_bucket(seconds));
        *self.durations.entry(key).or_default() += 1;
    }

    /// Counts a check of a run in the hour of the end of the run.
    pub(crate) fn add_check(
        &mut self,
        ended_at: OffsetDateTime,
        (name, kind): (&str, CheckKind),
        required: bool,
        (is_failed, is_passed_on_retry): (bool, bool),
    ) {
        let key = (hour_of(ended_at), name.to_owned(), kind.as_str());
        let counts = self.checks.entry(key).or_default();
        counts.required |= required;
        counts.runs += 1;
        counts.failed += i64::from(is_failed);
        counts.passed_on_retry += i64::from(is_passed_on_retry);
    }

    /// Adds the increments to the aggregate of `queue`, in the transaction of
    /// the facts.
    pub(crate) async fn write(
        &self,
        conn: &mut PgConnection,
        (org, queue): (OrgId, QueueId),
    ) -> Result<(), AnalyticsError> {
        let ids = (org.to_string(), queue.to_string());

        self.write_conclusions(conn, &ids).await?;
        self.write_durations(conn, &ids).await?;
        self.write_checks(conn, &ids).await
    }

    async fn write_conclusions(
        &self,
        conn: &mut PgConnection,
        (org, queue): &(String, String),
    ) -> Result<(), AnalyticsError> {
        if self.conclusions.is_empty() {
            return Ok(());
        }

        let (keys, counts): (Vec<_>, Vec<i64>) = self.conclusions.iter().unzip();
        let hours: Vec<OffsetDateTime> = keys.iter().map(|(hour, _, _)| *hour).collect();
        let conclusions: Vec<&str> = keys.iter().map(|(_, conclusion, _)| *conclusion).collect();
        let reasons: Vec<&str> = keys.iter().map(|(_, _, reason)| *reason).collect();
        sqlx::query(
            "INSERT INTO an_hourly_conclusions (org_id, queue_id, hour, conclusion, reason, count)
             SELECT $1, $2, r.hour, r.conclusion::item_state, r.reason::reason, r.count
             FROM unnest($3::timestamptz[], $4::text[], $5::text[], $6::bigint[])
                  AS r (hour, conclusion, reason, count)
             ON CONFLICT (queue_id, hour, conclusion, reason)
             DO UPDATE SET count = an_hourly_conclusions.count + excluded.count",
        )
        .bind(org)
        .bind(queue)
        .bind(&hours)
        .bind(&conclusions)
        .bind(&reasons)
        .bind(&counts)
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    async fn write_durations(
        &self,
        conn: &mut PgConnection,
        (org, queue): &(String, String),
    ) -> Result<(), AnalyticsError> {
        if self.durations.is_empty() {
            return Ok(());
        }

        let (keys, counts): (Vec<_>, Vec<i64>) = self.durations.iter().unzip();
        let hours: Vec<OffsetDateTime> = keys.iter().map(|(hour, _, _)| *hour).collect();
        let measures: Vec<&str> = keys.iter().map(|(_, measure, _)| *measure).collect();
        let buckets: Vec<i16> = keys.iter().map(|(_, _, bucket)| *bucket).collect();
        sqlx::query(
            "INSERT INTO an_hourly_durations (org_id, queue_id, hour, measure, bucket, count)
             SELECT $1, $2, r.hour, r.measure, r.bucket, r.count
             FROM unnest($3::timestamptz[], $4::text[], $5::smallint[], $6::bigint[])
                  AS r (hour, measure, bucket, count)
             ON CONFLICT (queue_id, hour, measure, bucket)
             DO UPDATE SET count = an_hourly_durations.count + excluded.count",
        )
        .bind(org)
        .bind(queue)
        .bind(&hours)
        .bind(&measures)
        .bind(&buckets)
        .bind(&counts)
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    async fn write_checks(
        &self,
        conn: &mut PgConnection,
        (org, queue): &(String, String),
    ) -> Result<(), AnalyticsError> {
        if self.checks.is_empty() {
            return Ok(());
        }

        let hours: Vec<OffsetDateTime> = self.checks.keys().map(|(hour, _, _)| *hour).collect();
        let names: Vec<&str> = self
            .checks
            .keys()
            .map(|(_, name, _)| name.as_str())
            .collect();
        let kinds: Vec<&str> = self.checks.keys().map(|(_, _, kind)| *kind).collect();
        let required: Vec<bool> = self.checks.values().map(|counts| counts.required).collect();
        let runs: Vec<i64> = self.checks.values().map(|counts| counts.runs).collect();
        let failed: Vec<i64> = self.checks.values().map(|counts| counts.failed).collect();
        let retried: Vec<i64> = self
            .checks
            .values()
            .map(|counts| counts.passed_on_retry)
            .collect();
        sqlx::query(
            "INSERT INTO an_hourly_checks (org_id, queue_id, hour, name, kind, required, runs,
                                           failed, passed_on_retry)
             SELECT $1, $2, r.hour, r.name, r.kind::check_kind, r.required, r.runs, r.failed,
                    r.passed_on_retry
             FROM unnest($3::timestamptz[], $4::text[], $5::text[], $6::boolean[], $7::bigint[],
                         $8::bigint[], $9::bigint[])
                  AS r (hour, name, kind, required, runs, failed, passed_on_retry)
             ON CONFLICT (queue_id, hour, name, kind)
             DO UPDATE SET required = an_hourly_checks.required OR excluded.required,
                           runs = an_hourly_checks.runs + excluded.runs,
                           failed = an_hourly_checks.failed + excluded.failed,
                           passed_on_retry = an_hourly_checks.passed_on_retry
                                             + excluded.passed_on_retry",
        )
        .bind(org)
        .bind(queue)
        .bind(&hours)
        .bind(&names)
        .bind(&kinds)
        .bind(&required)
        .bind(&runs)
        .bind(&failed)
        .bind(&retried)
        .execute(&mut *conn)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::*;

    #[test]
    fn a_bucket_stands_for_its_durations_within_5_percent() {
        for seconds in [1, 7, 59, 60, 600, 1_442, 3_600, 86_400, 2_592_000] {
            let found = bucket_seconds(duration_bucket(seconds));

            #[expect(clippy::cast_precision_loss, reason = "durations of a test")]
            let error = (found as f64 - seconds as f64).abs() / seconds as f64;
            assert!(error <= 0.05, "{seconds} s reads as {found} s");
        }
    }

    #[test]
    fn no_time_has_its_own_bucket() {
        assert_eq!(duration_bucket(0), 0);
        assert_eq!(bucket_seconds(0), 0);
        assert_eq!(duration_bucket(1), 1);
    }

    #[test]
    fn a_percentile_is_the_bucket_where_its_rank_falls() {
        let ten = duration_bucket(10);
        let hundred = duration_bucket(100);
        let histogram = [(0, 2), (ten, 5), (hundred, 3)];

        assert_eq!(percentile(&histogram, 0.2), Some(0));
        assert_eq!(percentile(&histogram, 0.5), Some(bucket_seconds(ten)));
        assert_eq!(percentile(&histogram, 0.9), Some(bucket_seconds(hundred)));
        assert_eq!(percentile(&[], 0.5), None);
    }

    #[test]
    fn an_hour_starts_on_the_hour_in_utc() {
        let at = datetime!(2026-10-14 11:41:12.5 +02:00);

        assert_eq!(hour_of(at), datetime!(2026-10-14 09:00 UTC));
    }
}

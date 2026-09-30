//! The budget of a connection: what its rate limit headers say, and the mode
//! that follows (`docs/RELIABILITY.md`, `AGENTS.md` D77). Pure: the caller
//! gives the time.

use std::collections::BTreeMap;

use cihub_domain::{ConnectionMode, JobClass};
use cihub_scm::ScmError;
use time::{Duration, OffsetDateTime};

/// Under this share of its budget, a connection is `throttled`.
const THROTTLE_BELOW: f64 = 0.2;
/// A `throttled` connection is `normal` again after this long above this share.
const RECOVER_ABOVE: f64 = 0.4;
const RECOVER_AFTER: Duration = Duration::minutes(5);
/// After this many failures in a row, a connection is `scm_unavailable`.
const FAILURES_TO_UNAVAILABLE: u32 = 3;
/// A `scm_unavailable` connection lets one call through this often.
const PROBE_EVERY: Duration = Duration::seconds(30);
/// The wait after a secondary rate limit without `retry-after`.
pub const SECONDARY_WAIT: Duration = Duration::minutes(1);

/// One rate limit of the SCM, such as the REST or the GraphQL one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rate {
    pub limit: u32,
    pub remaining: u32,
    pub reset_at: OffsetDateTime,
}

impl Rate {
    fn share(self) -> f64 {
        if self.limit == 0 {
            return 1.0;
        }
        f64::from(self.remaining) / f64::from(self.limit)
    }
}

/// What a call tells the budget.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Observation {
    /// The SCM answered, with this rate limit when the answer gives one.
    Answered {
        resource: String,
        rate: Option<Rate>,
    },
    /// The rate limit is spent until `until`; `secondary` for a limit of
    /// the SCM on bursts.
    RateLimited {
        until: OffsetDateTime,
        secondary: bool,
    },
    /// No answer, or a server error.
    Failed,
}

/// The budget of one connection, in the memory of a process.
#[derive(Clone, Debug, PartialEq)]
pub struct Budget {
    pub rates: BTreeMap<String, Rate>,
    pub paused_until: Option<OffsetDateTime>,
    pub failures: u32,
    pub mode: ConnectionMode,
    pub mode_reason: Option<String>,
    recovering_since: Option<OffsetDateTime>,
    next_probe: Option<OffsetDateTime>,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            rates: BTreeMap::new(),
            paused_until: None,
            failures: 0,
            mode: ConnectionMode::Normal,
            mode_reason: None,
            recovering_since: None,
            next_probe: None,
        }
    }
}

impl Budget {
    /// Whether a call of `class` may go out at `now`. A `throttled`
    /// connection keeps its budget for the classes before `speculate`; a
    /// `scm_unavailable` connection lets one probe through every 30 s.
    ///
    /// # Errors
    ///
    /// The error that the call returns in place of an answer.
    pub fn admit(&mut self, class: JobClass, now: OffsetDateTime) -> Result<(), ScmError> {
        if let Some(until) = self.paused_until.filter(|until| *until > now) {
            return Err(ScmError::RateLimited { until });
        }
        match self.mode {
            ConnectionMode::ScmUnavailable => {
                if self.next_probe.is_some_and(|probe| probe > now) {
                    return Err(ScmError::Unavailable("waiting for the next probe".into()));
                }
                self.next_probe = Some(now + PROBE_EVERY);
            }
            ConnectionMode::Throttled if class == JobClass::Speculate => {
                let until = self
                    .lowest()
                    .map_or(now + SECONDARY_WAIT, |rate| rate.reset_at);
                return Err(ScmError::Throttled { until });
            }
            ConnectionMode::Normal | ConnectionMode::Throttled | ConnectionMode::Paused => {}
        }
        Ok(())
    }

    /// Records what a call observed at `now`.
    pub fn observe(&mut self, observation: Observation, now: OffsetDateTime) {
        match observation {
            Observation::Answered { resource, rate } => {
                self.failures = 0;
                if self.mode == ConnectionMode::ScmUnavailable {
                    self.set_mode(ConnectionMode::Normal, None);
                }
                if let Some(rate) = rate {
                    self.rates.insert(resource, rate);
                }
                self.follow_budget(now);
            }
            Observation::RateLimited { until, secondary } => {
                self.paused_until = Some(self.paused_until.map_or(until, |p| p.max(until)));
                if secondary {
                    self.recovering_since = None;
                    self.set_mode(
                        ConnectionMode::Throttled,
                        Some("secondary rate limit".into()),
                    );
                }
            }
            Observation::Failed => {
                self.failures += 1;
                if self.failures >= FAILURES_TO_UNAVAILABLE
                    && self.mode != ConnectionMode::ScmUnavailable
                {
                    self.next_probe = Some(now + PROBE_EVERY);
                    let reason = format!("{} failures in a row", self.failures);
                    self.set_mode(ConnectionMode::ScmUnavailable, Some(reason));
                }
            }
        }
    }

    /// The rate limit with the smallest share left.
    pub fn lowest(&self) -> Option<Rate> {
        self.rates
            .values()
            .copied()
            .min_by(|a, b| a.share().total_cmp(&b.share()))
    }

    fn follow_budget(&mut self, now: OffsetDateTime) {
        let share = self.lowest().map_or(1.0, Rate::share);
        if share < THROTTLE_BELOW {
            self.recovering_since = None;
            self.set_mode(ConnectionMode::Throttled, Some("budget below 20 %".into()));
        } else if self.mode == ConnectionMode::Throttled {
            if share <= RECOVER_ABOVE {
                self.recovering_since = None;
            } else if now - *self.recovering_since.get_or_insert(now) >= RECOVER_AFTER {
                self.recovering_since = None;
                self.set_mode(ConnectionMode::Normal, None);
            }
        }
    }

    fn set_mode(&mut self, mode: ConnectionMode, reason: Option<String>) {
        self.mode = mode;
        self.mode_reason = reason;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_789_000_000 + seconds).unwrap()
    }

    fn answered(remaining: u32) -> Observation {
        Observation::Answered {
            resource: "core".into(),
            rate: Some(Rate {
                limit: 5_000,
                remaining,
                reset_at: at(3_600),
            }),
        }
    }

    #[test]
    fn a_rate_limit_pauses_every_class_until_its_end() {
        let mut budget = Budget::default();
        budget.observe(
            Observation::RateLimited {
                until: at(60),
                secondary: false,
            },
            at(0),
        );
        for class in [JobClass::Interactive, JobClass::Merge] {
            assert_eq!(
                budget.admit(class, at(59)),
                Err(ScmError::RateLimited { until: at(60) })
            );
        }
        assert_eq!(budget.admit(JobClass::Merge, at(60)), Ok(()));
        assert_eq!(budget.mode, ConnectionMode::Normal);
    }

    #[test]
    fn a_low_budget_defers_the_speculation_only() {
        let mut budget = Budget::default();
        budget.observe(answered(999), at(0));
        assert_eq!(budget.mode, ConnectionMode::Throttled);
        assert_eq!(
            budget.admit(JobClass::Speculate, at(1)),
            Err(ScmError::Throttled { until: at(3_600) })
        );
        for class in [JobClass::Interactive, JobClass::Merge, JobClass::Repair] {
            assert_eq!(budget.admit(class, at(1)), Ok(()));
        }
    }

    #[test]
    fn a_throttled_connection_recovers_after_five_minutes_above_forty_percent() {
        let mut budget = Budget::default();
        budget.observe(answered(500), at(0));
        budget.observe(answered(2_500), at(10));
        budget.observe(answered(1_500), at(100));
        budget.observe(answered(2_500), at(200));
        budget.observe(answered(2_500), at(499));
        assert_eq!(
            budget.mode,
            ConnectionMode::Throttled,
            "the dip restarted the wait"
        );
        budget.observe(answered(2_500), at(500));
        assert_eq!(budget.mode, ConnectionMode::Normal);
        assert_eq!(budget.mode_reason, None);
    }

    #[test]
    fn a_secondary_limit_throttles_the_connection() {
        let mut budget = Budget::default();
        budget.observe(
            Observation::RateLimited {
                until: at(60),
                secondary: true,
            },
            at(0),
        );
        assert_eq!(budget.mode, ConnectionMode::Throttled);
        assert_eq!(budget.mode_reason.as_deref(), Some("secondary rate limit"));
    }

    #[test]
    fn three_failures_let_only_a_probe_through_until_an_answer() {
        let mut budget = Budget::default();
        for n in 0..3 {
            assert_eq!(budget.mode, ConnectionMode::Normal);
            budget.observe(Observation::Failed, at(n));
        }
        assert_eq!(budget.mode, ConnectionMode::ScmUnavailable);
        assert!(matches!(
            budget.admit(JobClass::Interactive, at(10)),
            Err(ScmError::Unavailable(_))
        ));
        assert_eq!(budget.admit(JobClass::Repair, at(32)), Ok(()), "the probe");
        assert!(budget.admit(JobClass::Interactive, at(33)).is_err());
        budget.observe(answered(4_000), at(33));
        assert_eq!(budget.mode, ConnectionMode::Normal);
        assert_eq!(budget.failures, 0);
        assert_eq!(budget.admit(JobClass::Speculate, at(34)), Ok(()));
    }

    #[test]
    fn the_lowest_share_of_the_rate_limits_decides() {
        let mut budget = Budget::default();
        budget.observe(answered(4_000), at(0));
        let graphql = Rate {
            limit: 5_000,
            remaining: 100,
            reset_at: at(1_800),
        };
        budget.observe(
            Observation::Answered {
                resource: "graphql".into(),
                rate: Some(graphql),
            },
            at(1),
        );
        assert_eq!(budget.lowest(), Some(graphql));
        assert_eq!(budget.mode, ConnectionMode::Throttled);
    }
}

//! The reports of the CI on a commit: a check or a commit status, as the SCM
//! reports it on a PR and as the queue observes it on the test SHA of a run.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{OrgId, RunId};

closed_enum! {
    /// Kind of a report on a commit. Two reports of different kinds can have
    /// the same name.
    pub enum CheckKind {
        /// A check with a run: a GitHub check run, a GitLab pipeline job.
        Check,
        /// A commit status.
        Status,
    }
}

closed_enum! {
    /// Progress of a check or a commit status. The order of the variants is
    /// the order of the life of a check, as in the Postgres enum
    /// `check_status`: `CheckObservation::is_regressed_by` compares them.
    pub enum CheckStatus {
        /// The check waits to start; a commit status that is expected but
        /// not reported yet.
        Queued,
        /// The check runs; a commit status in `pending`.
        InProgress,
        /// The check ended, with a conclusion.
        Completed,
    }
}

closed_enum! {
    /// Terminal outcome of a check or a commit status.
    pub enum CheckConclusion {
        /// The check passed.
        Success,
        /// The check ended without a verdict; it counts as a pass.
        Neutral,
        /// The check did not run; it counts as a pass.
        Skipped,
        /// The check failed.
        Failure,
        /// The check could not run to its end: an error of the CI or of the
        /// commit status.
        Error,
        /// Someone or something cancelled the check.
        Cancelled,
        /// The check ran longer than its CI allows.
        TimedOut,
        /// The check waits for an action of a person on the SCM.
        ActionRequired,
        /// The SCM marked the check as too old to count.
        Stale,
    }
}

impl CheckConclusion {
    /// A required check passes on `success`, `neutral` or `skipped`; any other
    /// conclusion fails the run.
    pub const fn is_passing(self) -> bool {
        matches!(self, Self::Success | Self::Neutral | Self::Skipped)
    }
}

/// One check or commit status, as the SCM reports it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckReport {
    /// The name of the check, or the context of the commit status: what a
    /// required status names.
    pub name: String,
    /// Whether the report is a check or a commit status.
    pub kind: CheckKind,
    /// Where the check is in its life.
    pub status: CheckStatus,
    /// `None` while the check has not concluded.
    pub conclusion: Option<CheckConclusion>,
    /// The page of the check on the CI; `None` when the SCM gives none.
    pub details_url: Option<String>,
    /// A commit status gives the time of its state only: `started_at` while
    /// it is pending, `completed_at` once it concluded.
    #[serde(with = "time::serde::rfc3339::option")]
    pub started_at: Option<OffsetDateTime>,
    /// When the check concluded; `None` before its conclusion.
    #[serde(with = "time::serde::rfc3339::option")]
    pub completed_at: Option<OffsetDateTime>,
}

closed_enum! {
    /// Where a required check that has not concluded stands.
    pub enum AwaitedCheckStatus {
        /// The check has not reported on the head of the PR.
        Missing,
        /// The check reported that it waits to start.
        Queued,
        /// The check runs, or completed without a conclusion yet.
        Running,
    }
}

impl AwaitedCheckStatus {
    /// The status of a required check without a conclusion, from its report
    /// on the head of the PR, if any.
    pub fn of(report: Option<&CheckReport>) -> Self {
        match report.map(|report| report.status) {
            None => Self::Missing,
            Some(CheckStatus::Queued) => Self::Queued,
            Some(CheckStatus::InProgress | CheckStatus::Completed) => Self::Running,
        }
    }
}

/// Last report of a check run or commit status on the test SHA of a run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckObservation {
    /// The organization of the run.
    pub org_id: OrgId,
    /// The run whose test SHA the check reported on.
    pub run_id: RunId,
    /// The name of the check, or the context of the commit status.
    pub name: String,
    /// Whether the report is a check or a commit status.
    pub kind: CheckKind,
    /// The name is in the required statuses of the run.
    pub required: bool,
    /// Where the check is in its life.
    pub status: CheckStatus,
    /// `None` while the check has not concluded.
    pub conclusion: Option<CheckConclusion>,
    /// The page of the check on the CI; `None` when the SCM gives none.
    pub details_url: Option<String>,
    /// When the check started, as the SCM reports it.
    #[serde(with = "time::serde::rfc3339::option")]
    pub started_at: Option<OffsetDateTime>,
    /// When the check concluded, as the SCM reports it.
    #[serde(with = "time::serde::rfc3339::option")]
    pub completed_at: Option<OffsetDateTime>,
    /// When CIHub observed this report.
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
}

impl CheckObservation {
    /// Whether `report` says the same as this observation.
    pub fn reports_the_same(&self, report: &CheckReport) -> bool {
        self.status == report.status
            && self.conclusion == report.conclusion
            && self.details_url == report.details_url
            && self.started_at == report.started_at
            && self.completed_at == report.completed_at
    }

    /// Whether `report` would take this check back to an earlier status: a
    /// late delivery of the same execution, as a `queued` after the
    /// `in_progress`, or an `in_progress` after the `completed`. A check that
    /// starts again after its end, as a rerun, does not; without both times,
    /// or within the same second as the end, nothing tells it, and the check
    /// waits for its next conclusion.
    pub fn is_regressed_by(&self, report: &CheckReport) -> bool {
        let is_rerun = report
            .started_at
            .zip(self.completed_at)
            .is_some_and(|(started, ended)| started > ended);

        report.status < self.status && !is_rerun
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_check_goes_back_only_for_a_rerun() {
        let start = OffsetDateTime::UNIX_EPOCH;
        let at = |minutes| Some(start + time::Duration::minutes(minutes));
        let completed = CheckObservation {
            org_id: OrgId::from_ulid(ulid::Ulid(1)),
            run_id: RunId::from_ulid(ulid::Ulid(2)),
            name: "Smoke".into(),
            kind: CheckKind::Check,
            required: true,
            status: CheckStatus::Completed,
            conclusion: Some(CheckConclusion::Success),
            details_url: None,
            started_at: at(10),
            completed_at: at(20),
            observed_at: start,
        };
        let report = |status, started_at| CheckReport {
            name: "Smoke".into(),
            kind: CheckKind::Check,
            status,
            conclusion: None,
            details_url: None,
            started_at,
            completed_at: None,
        };

        assert!(completed.is_regressed_by(&report(CheckStatus::InProgress, at(10))));
        assert!(completed.is_regressed_by(&report(CheckStatus::Queued, None)));
        assert!(!completed.is_regressed_by(&report(CheckStatus::Queued, at(25))));
        assert!(!completed.is_regressed_by(&report(CheckStatus::Completed, at(10))));

        let running = CheckObservation {
            status: CheckStatus::InProgress,
            conclusion: None,
            completed_at: None,
            ..completed
        };

        assert!(running.is_regressed_by(&report(CheckStatus::Queued, at(5))));
        assert!(!running.is_regressed_by(&report(CheckStatus::InProgress, at(10))));
        assert!(!running.is_regressed_by(&report(CheckStatus::Completed, at(10))));
    }

    #[test]
    fn an_awaited_check_is_missing_queued_or_running() {
        let report = |status| CheckReport {
            name: "ci".into(),
            kind: CheckKind::Check,
            status,
            conclusion: None,
            details_url: None,
            started_at: None,
            completed_at: None,
        };

        let cases = [
            (None, AwaitedCheckStatus::Missing),
            (
                Some(report(CheckStatus::Queued)),
                AwaitedCheckStatus::Queued,
            ),
            (
                Some(report(CheckStatus::InProgress)),
                AwaitedCheckStatus::Running,
            ),
        ];

        for (report, want) in cases {
            assert_eq!(AwaitedCheckStatus::of(report.as_ref()), want);
        }
    }
}

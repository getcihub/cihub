use std::time::Duration;

use cihub_domain::{CheckConclusion, Permission, ReviewDecision};

use crate::{Attribute, EvalError};

/// What conditions are evaluated on: a PR, its checks, and its place in the
/// queue. The caller loads it; checks come from the PR head for
/// `queue_conditions`, and from the test SHA for `merge_conditions`.
#[derive(Clone, Debug, Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "one field per boolean attribute"
)]
pub struct Context {
    pub draft: bool,
    pub conflict: bool,
    pub from_fork: bool,
    pub behind_base: bool,
    pub number: u64,
    pub title: String,
    pub body: String,
    pub review_decision: ReviewDecision,
    pub approved_reviews_by: Vec<String>,
    pub changes_requested_reviews_by: Vec<String>,
    pub review_requested: Vec<String>,
    /// Loaded on demand: `None` when no condition uses it.
    pub review_threads_unresolved: Option<Vec<String>>,
    pub checks: Vec<Check>,
    pub labels: Vec<String>,
    pub author: String,
    pub sender: Option<String>,
    pub sender_permission: Option<Permission>,
    /// Loaded on demand: `None` when no condition uses it.
    pub files: Option<Vec<String>>,
    pub additions: u64,
    pub deletions: u64,
    pub base: String,
    pub head: String,
    /// 0 at the head of the queue; `None` when the PR is not in a queue.
    pub queue_position: Option<u64>,
    /// `None` when the PR is not in a queue.
    pub queue_age: Option<Duration>,
    pub pr_age: Duration,
}

/// Last report of one check run or commit status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Check {
    pub name: String,
    /// `None` while the check has not concluded.
    pub conclusion: Option<CheckConclusion>,
}

/// Value of an attribute, in the shape that its kind gives. A scalar string
/// is a list of at most one element.
pub(crate) enum Value<'a> {
    Flag(bool),
    Texts(Vec<&'a str>),
    /// `None` when absent; a comparison on it is false.
    Number(Option<i64>),
}

impl Context {
    pub(crate) fn value<'a>(&'a self, attribute: Attribute) -> Result<Value<'a>, EvalError> {
        let loaded = |list: &'a Option<Vec<String>>| {
            list.as_deref()
                .map(texts)
                .ok_or(EvalError::NotLoaded(attribute))
        };
        Ok(match attribute {
            Attribute::Draft => Value::Flag(self.draft),
            Attribute::Conflict => Value::Flag(self.conflict),
            Attribute::FromFork => Value::Flag(self.from_fork),
            Attribute::BehindBase => Value::Flag(self.behind_base),
            Attribute::Number => number(self.number),
            Attribute::Title => Value::Texts(vec![&self.title]),
            Attribute::Body => Value::Texts(vec![&self.body]),
            Attribute::ReviewDecision => Value::Number(Some(self.review_decision as i64)),
            Attribute::ApprovedReviewsBy => texts(&self.approved_reviews_by),
            Attribute::ChangesRequestedReviewsBy => texts(&self.changes_requested_reviews_by),
            Attribute::ReviewRequested => texts(&self.review_requested),
            Attribute::ReviewThreadsUnresolved => loaded(&self.review_threads_unresolved)?,
            Attribute::CheckSuccess => {
                self.checks_where(|conclusion| conclusion == Some(CheckConclusion::Success))
            }
            Attribute::CheckNeutral => {
                self.checks_where(|conclusion| conclusion == Some(CheckConclusion::Neutral))
            }
            Attribute::CheckSkipped => {
                self.checks_where(|conclusion| conclusion == Some(CheckConclusion::Skipped))
            }
            Attribute::CheckFailure => self.checks_where(|conclusion| {
                conclusion.is_some_and(|conclusion| !conclusion.is_passing())
            }),
            Attribute::CheckPending => self.checks_where(|conclusion| conclusion.is_none()),
            Attribute::Labels => texts(&self.labels),
            Attribute::Author => Value::Texts(vec![&self.author]),
            Attribute::Sender => Value::Texts(self.sender.as_deref().into_iter().collect()),
            Attribute::SenderPermission => {
                Value::Number(self.sender_permission.map(|permission| permission as i64))
            }
            Attribute::Files => loaded(&self.files)?,
            Attribute::Additions => number(self.additions),
            Attribute::Deletions => number(self.deletions),
            Attribute::Base => Value::Texts(vec![&self.base]),
            Attribute::Head => Value::Texts(vec![&self.head]),
            Attribute::QueuePosition => Value::Number(self.queue_position.map(saturate)),
            Attribute::QueueAge => Value::Number(self.queue_age.map(seconds)),
            Attribute::PrAge => Value::Number(Some(seconds(self.pr_age))),
        })
    }

    fn checks_where(&self, keep: impl Fn(Option<CheckConclusion>) -> bool) -> Value<'_> {
        Value::Texts(
            self.checks
                .iter()
                .filter(|check| keep(check.conclusion))
                .map(|check| check.name.as_str())
                .collect(),
        )
    }
}

fn texts(list: &[String]) -> Value<'_> {
    Value::Texts(list.iter().map(String::as_str).collect())
}

fn number(number: u64) -> Value<'static> {
    Value::Number(Some(saturate(number)))
}

fn seconds(duration: Duration) -> i64 {
    saturate(duration.as_secs())
}

pub(crate) fn saturate(number: impl TryInto<i64>) -> i64 {
    number.try_into().unwrap_or(i64::MAX)
}

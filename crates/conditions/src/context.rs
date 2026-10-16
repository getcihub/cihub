use std::fmt;
use std::time::Duration;

use cihub_domain::{CheckConclusion, Permission, ReviewDecision};

use crate::Attribute;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvalError {
    /// A condition uses an attribute that is loaded on demand, and the
    /// context does not have it.
    NotLoaded(Attribute),
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotLoaded(a) => write!(f, "attribute `{a}` is not loaded in the context"),
        }
    }
}

impl std::error::Error for EvalError {}

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
        use Attribute as A;
        use CheckConclusion as C;
        let loaded = |list: &'a Option<Vec<String>>| {
            list.as_deref()
                .map(texts)
                .ok_or(EvalError::NotLoaded(attribute))
        };
        Ok(match attribute {
            A::Draft => Value::Flag(self.draft),
            A::Conflict => Value::Flag(self.conflict),
            A::FromFork => Value::Flag(self.from_fork),
            A::BehindBase => Value::Flag(self.behind_base),
            A::Number => number(self.number),
            A::Title => Value::Texts(vec![&self.title]),
            A::Body => Value::Texts(vec![&self.body]),
            A::ReviewDecision => Value::Number(Some(self.review_decision as i64)),
            A::ApprovedReviewsBy => texts(&self.approved_reviews_by),
            A::ChangesRequestedReviewsBy => texts(&self.changes_requested_reviews_by),
            A::ReviewRequested => texts(&self.review_requested),
            A::ReviewThreadsUnresolved => loaded(&self.review_threads_unresolved)?,
            A::CheckSuccess => self.checks_where(|c| c == Some(C::Success)),
            A::CheckNeutral => self.checks_where(|c| c == Some(C::Neutral)),
            A::CheckSkipped => self.checks_where(|c| c == Some(C::Skipped)),
            A::CheckFailure => self.checks_where(|c| c.is_some_and(|c| !c.is_passing())),
            A::CheckPending => self.checks_where(|c| c.is_none()),
            A::Label => texts(&self.labels),
            A::Author => Value::Texts(vec![&self.author]),
            A::Sender => Value::Texts(self.sender.as_deref().into_iter().collect()),
            A::SenderPermission => Value::Number(self.sender_permission.map(|p| p as i64)),
            A::Files => loaded(&self.files)?,
            A::Additions => number(self.additions),
            A::Deletions => number(self.deletions),
            A::Base => Value::Texts(vec![&self.base]),
            A::Head => Value::Texts(vec![&self.head]),
            A::QueuePosition => match self.queue_position {
                Some(position) => number(position),
                None => Value::Number(Some(-1)),
            },
            A::QueueAge => Value::Number(self.queue_age.map(seconds)),
            A::PrAge => Value::Number(Some(seconds(self.pr_age))),
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

fn number(n: u64) -> Value<'static> {
    Value::Number(Some(saturate(n)))
}

fn seconds(d: Duration) -> i64 {
    saturate(d.as_secs())
}

fn saturate(n: u64) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

use time::OffsetDateTime;

/// Why a call to the SCM failed, in the terms of CIHub. The adapter keeps the
/// detail of the SCM in the message, for the operator.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ScmError {
    /// The rate limit of the connection is spent: no call before `until`.
    #[error("rate limit of the connection spent until {until}")]
    RateLimited { until: OffsetDateTime },
    /// The budget of the connection is low and keeps its calls for the
    /// classes before this one: try again after `until`.
    #[error("budget of the connection kept for other classes until {until}")]
    Throttled { until: OffsetDateTime },
    /// The SCM did not answer, or answered with a server error.
    #[error("SCM unavailable: {0}")]
    Unavailable(String),
    /// The connection cannot act any more: suspended, deleted, or its
    /// credentials are refused.
    #[error("access of the connection lost: {0}")]
    AccessLost(String),
    /// The object does not exist, or the connection cannot see it.
    #[error("not found: {0}")]
    NotFound(String),
    /// The SCM refuses the call: the rules of the repository, or a permission
    /// that the connection lacks. The message says which.
    #[error("refused by the SCM: {0}")]
    Forbidden(String),
    /// The SCM refuses the request as not valid.
    #[error("request refused as not valid: {0}")]
    Validation(String),
    /// The head of the PR is no longer the expected one.
    #[error("head of the PR changed")]
    HeadChanged,
    /// The SCM keeps a merge of the PR in progress.
    #[error("merge already in progress")]
    MergeInProgress,
    /// The base moved: the commit to land does not descend from it.
    #[error("not a fast forward of the base")]
    NotFastForward,
    /// The SCM refuses the merge for another reason.
    #[error("merge refused: {0}")]
    MergeRejected(String),
    /// The answer of the SCM does not match what the adapter expects.
    #[error("answer of the SCM not understood: {0}")]
    Protocol(String),
}

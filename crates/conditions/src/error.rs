use crate::{Attribute, Operator};

/// An error in a condition; `value` is the index of the faulty value, `None`
/// when the operator itself is faulty.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{kind}")]
pub struct ConditionError {
    pub value: Option<usize>,
    pub kind: ConditionErrorKind,
}

impl ConditionError {
    pub(crate) const fn at(index: usize, kind: ConditionErrorKind) -> Self {
        Self {
            value: Some(index),
            kind,
        }
    }

    pub(crate) const fn whole(kind: ConditionErrorKind) -> Self {
        Self { value: None, kind }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ConditionErrorKind {
    #[error("not allowed on this attribute ({})", attribute.kind())]
    OperatorNotAllowed { attribute: Attribute },
    /// A value where the attribute takes only keys (a duration).
    #[error("expected a key ({}), not a value", key_list(*attribute))]
    KeyExpected { attribute: Attribute },
    #[error("missing value")]
    MissingValue,
    #[error("takes one value, not a list")]
    OneValue,
    #[error("`{0}` is not `true` or `false`")]
    InvalidBoolean(String),
    #[error("`{0}` is not an integer from 0 to {max}", max = u32::MAX)]
    InvalidNumber(String),
    #[error("`{0}` is not a duration such as `30m`, `2h` or `7d`")]
    InvalidDuration(String),
    #[error("`{value}` is not valid: expected {}", attribute.kind())]
    InvalidChoice { attribute: Attribute, value: String },
    #[error("invalid regular expression: {0}")]
    InvalidRegex(String),
    #[error("invalid glob: {0}")]
    InvalidGlob(String),
}

fn key_list(attribute: Attribute) -> String {
    Operator::keys(attribute).collect::<Vec<_>>().join(", ")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum EvalError {
    /// A condition uses an attribute that is loaded on demand, and the
    /// context does not have it.
    #[error("attribute `{0}` is not loaded in the context")]
    NotLoaded(Attribute),
}

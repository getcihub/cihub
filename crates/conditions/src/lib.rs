//! Conditions of `queue_conditions`, `merge_conditions`, profiles and
//! `priority_rules`: their attributes, their operators, the check of their
//! values and their evaluation on a PR context. Reading the YAML is the role
//! of `cihub-config`. Pure: no I/O.

mod attribute;
mod condition;
mod context;
mod error;
mod expr;
mod glob;
mod operator;
mod parse;

pub use attribute::Attribute;
pub use condition::Condition;
pub use context::{Check, Context};
pub use error::{ConditionError, ConditionErrorKind, EvalError};
pub use expr::{Conditions, Expr};
pub use glob::Glob;
pub use operator::Operator;

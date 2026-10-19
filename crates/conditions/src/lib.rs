//! Conditions of `queue_conditions`, `merge_conditions`, profiles and
//! `priority_rules`: their attributes, their operators, the check of their
//! values and their evaluation on a PR context. Reading the YAML is the role
//! of `cihub-config`. Pure: no I/O.

mod attribute;
mod condition;
mod context;
mod expr;

pub use attribute::Attribute;
pub use condition::{Condition, ConditionError, ConditionErrorKind, Glob, Operator};
pub use context::{Check, Context, EvalError};
pub use expr::{Conditions, Expr};

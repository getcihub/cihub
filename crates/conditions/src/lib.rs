//! Condition language of `route_conditions`, `queue_conditions`,
//! `merge_conditions` and `priority_rules`: parsing and evaluation on a PR
//! context. Pure: no I/O.

mod attribute;
mod condition;
mod context;
mod expr;
mod glob;

pub use attribute::Attribute;
pub use condition::{Condition, Operator, ParseError, ParseErrorKind};
pub use context::{Check, Context, EvalError};
pub use expr::{Conditions, Expr};
pub use glob::PathGlob;

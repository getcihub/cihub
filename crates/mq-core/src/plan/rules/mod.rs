//! The rules of `plan`, grouped by subject. Each rule reads the snapshot and
//! gives at most one step.

pub(super) mod bisection;
pub(super) mod formation;
pub(super) mod intake;
pub(super) mod readiness;
pub(super) mod runs;

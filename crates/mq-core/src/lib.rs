//! Merge queue core: `apply` (replay of the log), queue order and merge
//! graph, `plan` (the reconciler) and `check` (consistency rules). Pure: no
//! I/O, no clock and no randomness; the caller gives the time and writes the
//! decisions. It does not know Postgres, the SCM or the jobs that make the SCM
//! converge to the desired state.

mod apply;
mod check;
pub mod graph;
mod ids;
mod plan;
mod snapshot;
mod surface;

pub use apply::{ApplyError, Outcome, apply};
pub use check::{Violation, check};
pub use plan::{CheckRead, Desired, Head, Merge, Plan, PlanError, Read, TestBranch, plan};
pub use snapshot::{
    Observed, PrObservation, Request, Snapshot, Subject, TargetBranch, TargetsUpload,
};
pub use surface::Surface;

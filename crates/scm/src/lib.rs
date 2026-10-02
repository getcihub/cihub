//! The contract between CIHub and an SCM (`AGENTS.md` D73, D76): the `Scm`
//! trait, the neutral values it reads and writes, its errors, and the neutral
//! events that an adapter reads in a webhook delivery (D75).
//!
//! No SCM appears here (D78): each adapter translates its own values. This
//! crate does no I/O and has no client; the adapters do.

#[cfg(any(test, feature = "sim"))]
pub mod contract;
mod error;
mod event;
mod scm;
#[cfg(any(test, feature = "sim"))]
pub mod sim;
mod types;

pub use error::ScmError;
pub use event::{SUBMIT_BOX, ScmDelivery, ScmEvent};
pub use scm::{MAX_PRS_PER_READ, Scm, ScmFuture};
pub use types::{
    CommitStatus, ConnectionRef, MergeIntoRef, PrMerge, PrSnapshot, RecentDelivery, RepoRef,
    RequiredChecks, ScmAccount, ScmRepo, ScmUser, SetupFinding, StatusUpdate,
};

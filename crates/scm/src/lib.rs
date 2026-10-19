//! The contract between CIHub and an SCM: the `Scm` trait, the neutral values
//! it reads and writes, its errors, and the neutral events that an adapter
//! reads in a webhook delivery.
//!
//! No SCM appears here: each adapter translates its own values. This
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
pub use scm::{MAX_OPEN_PRS, MAX_PRS_PER_READ, Scm, ScmFuture};
pub use types::{
    CommitStatus, ConnectionRef, MergeIntoRef, PrMerge, PrSnapshot, RecentDelivery, RepoRef,
    RequiredChecks, ScmAccount, ScmInstallation, ScmMember, ScmMembership, ScmPersonReach,
    ScmPersonRepo, ScmPersonToken, ScmProfile, ScmRepo, ScmUser, SetupFinding, SignInCode,
    SignInRequest, SignedIn, StatusUpdate, check_passed, code_challenge,
};

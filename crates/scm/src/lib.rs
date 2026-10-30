//! The contract between CIHub and an SCM: the calls that CIHub makes, the
//! neutral values that they read and write, their errors, and the neutral
//! events that an adapter reads in a webhook delivery.
//!
//! This crate does no I/O and has no client: an adapter (`cihub-github`)
//! makes the calls. No SCM name appears here: each adapter translates its own
//! values into these ones. The rules and the flows are in `specs/GITHUB.md`
//! and `specs/ARCHITECTURE.md`; this crate only names the pieces.
//!
//! # Components
//!
//! - [`Scm`]: every call of CIHub to its SCM. It is the sum of three traits,
//!   which an adapter implements:
//!   - [`RepoCalls`]: the calls on one repository, under the budget of its
//!     connection (PRs, branches, checks, comments, labels, merges).
//!   - [`AppCalls`]: the calls of the App itself (installations, webhook
//!     deliveries, repositories and members of a connection).
//!   - [`PersonCalls`]: the sign-in of a person, and the reads with their
//!     token.
//! - [`ScmFuture`]: the boxed answer of a call. The executors hold the SCM as
//!   `Arc<dyn Scm>`, which a trait with `async fn` does not allow.
//! - [`ScmError`]: why a call failed, in the terms of CIHub.
//! - [`ScmDelivery`] and [`ScmEvent`]: what a webhook delivery tells CIHub.
//! - `sim` (feature `sim`): an SCM in memory for the tests, with fault
//!   injection.
//! - `contract` (feature `sim`): the sequence of calls that the simulator and
//!   every adapter must pass, through `contract::run`.
//!
//! # Identifiers
//!
//! CIHub and the SCM each name things in their own way. An identifier of
//! CIHub is a typed identifier of `cihub_domain`; an identifier of the SCM is
//! opaque text that CIHub stores and compares, but never parses or builds.
//!
//! | Identifier | What it designates | Owner |
//! | --- | --- | --- |
//! | [`ConnectionRef::id`] | A connection of CIHub: its access to the repositories of one account ([`cihub_domain::ConnectionId`], `connection_…`). | CIHub |
//! | `external_id` of [`ConnectionRef`], [`ScmInstallation`] | The installation of the App behind a connection (`scm_connections.external_id`). | SCM |
//! | `connection_external_id` | The same installation, named from another value (a delivery, a membership, a repository that a person reaches). | SCM |
//! | `external_id` of [`RepoRef`], [`ScmRepo`]; `repo_external_id` | A repository (`repos.external_id`). It stays the same when the repository is renamed. | SCM |
//! | `external_id` of [`ScmUser`] | A user account (`users.external_id`). It never changes. | SCM |
//! | [`ScmUser::login`] | The name of a user. It can change: CIHub keys a person by `external_id`, and shows the login. | SCM |
//! | `external_id` of [`ScmAccount`] | The account (organization or user) where the App is installed (`scm_connections.account_external_id`). | SCM |
//! | [`ScmAccount::path`] | The name of that account in the addresses of the SCM. It can change. | SCM |
//! | `owner` and `name` of [`RepoRef`], [`ScmRepo`] | The path of a repository on the SCM, for the addresses and the messages. They can change. | SCM |
//! | PR `number` | A PR or an issue, unique only inside its repository: a PR is (repository, number). | SCM |
//! | `sha`, `head_sha`, `tested_sha`, `merged` commits | A commit, as its full hexadecimal hash. | git |
//! | Branch names (`branch`, `base_branch`, `head_branch`) | A branch, by its short name without `refs/heads/`. | git |
//! | `comment_id`, `comment_external_id` | A comment on a PR (`pr_comments.comment_external_id`), as [`RepoCalls::create_comment`] returns it. | SCM |
//! | [`RecentDelivery::delivery_id`] | A webhook delivery. Every attempt of the same delivery carries it, so CIHub stores a delivery once (`webhook_inbox.delivery_id`). | SCM |
//! | [`RecentDelivery::attempt_id`] | One attempt to send a delivery, to ask for it again ([`AppCalls::redeliver`]). | SCM |
//!
//! The tables that store them are in `specs/DATA_MODEL.md`; how the GitHub
//! adapter reads them is in `specs/GITHUB.md`.

#![deny(missing_docs)]

mod app;
#[cfg(any(test, feature = "sim"))]
pub mod contract;
mod error;
mod event;
mod person;
mod repo;
mod scm;
#[cfg(any(test, feature = "sim"))]
pub mod sim;

pub use app::{
    AppCalls, RecentDelivery, ScmAccount, ScmAccountProfile, ScmInstallation, ScmMember, ScmRepo,
};
pub use error::ScmError;
pub use event::{SUBMIT_BOX, ScmDelivery, ScmEvent};
pub use person::{
    PersonCalls, ScmMembership, ScmPersonReach, ScmPersonRepo, ScmPersonToken, ScmProfile,
    SignInCode, SignInRequest, SignedIn, code_challenge,
};
pub use repo::{
    BranchMove, CommitStatus, DraftPr, MAX_OPEN_PRS, MAX_PRS_PER_READ, MergeIntoRef, PrMerge,
    PrSnapshot, RepoCalls, RequiredChecks, SetupFinding, StatusUpdate,
};
pub use scm::{ConnectionRef, RepoRef, Scm, ScmFuture, ScmUser};

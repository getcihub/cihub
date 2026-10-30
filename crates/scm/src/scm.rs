use std::future::Future;
use std::pin::Pin;

use cihub_domain::ConnectionId;

use crate::{AppCalls, PersonCalls, RepoCalls, ScmError};

/// The answer of a call. A future is boxed because an executor holds an
/// `Arc<dyn Scm>`, and a trait with `async fn` cannot be used as `dyn`.
pub type ScmFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, ScmError>> + Send + 'a>>;

/// The calls of CIHub to its SCM: those on a repository, those of the App,
/// and those with the token of a person. Each call names its class, which
/// decides what the budget of the connection lets through. A write whose
/// result already exists succeeds without a change, so that a job can run
/// twice (principle III).
pub trait Scm: RepoCalls + AppCalls + PersonCalls {}

impl<T: RepoCalls + AppCalls + PersonCalls> Scm for T {}

/// The connection that a call uses: its quota, its budget and its credentials.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionRef {
    /// The connection in CIHub.
    pub id: ConnectionId,
    /// The installation of the App behind the connection, on the SCM.
    pub external_id: String,
}

/// A repository, as a call names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoRef {
    /// The connection whose credentials and budget the call uses.
    pub connection: ConnectionRef,
    /// The repository on the SCM; it stays the same across a rename.
    pub external_id: String,
    /// The account that holds the repository, as the paths of the SCM name it.
    pub owner: String,
    /// The name of the repository in its account.
    pub name: String,
}

/// A user account on the SCM.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScmUser {
    /// The account on the SCM; it never changes.
    pub external_id: String,
    /// The name of the account; it can change.
    pub login: String,
}

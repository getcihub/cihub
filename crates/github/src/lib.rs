//! The GitHub adapter (`AGENTS.md` D73): the GitHub App, its client and its
//! budget per connection, the `Scm` trait on GitHub, and the signature of
//! its webhooks. The only crate that knows GitHub (D78): it translates the
//! values of GitHub into the neutral ones of `cihub-scm`. It writes nothing
//! in Postgres; the caller writes the budgets that it reads here.

mod auth;
mod budget;
mod client;
mod scm;
mod signature;

pub use auth::{AppKey, KeyError};
pub use budget::{Budget, Observation, Rate};
pub use client::{ClientError, Clock, GithubApp, GithubConfig};
pub use signature::verify_signature;

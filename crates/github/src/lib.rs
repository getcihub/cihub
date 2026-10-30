//! The GitHub adapter: the GitHub App, its client and its budget per
//! connection, the `Scm` trait on GitHub, and the reading of its signed
//! webhook deliveries. The only crate that knows GitHub: it translates the
//! values of GitHub into the neutral ones of `cihub-scm`. It writes nothing in
//! Postgres; the caller writes the budgets that it reads here.

mod apps;
mod auth;
mod budget;
mod checks;
mod client;
mod git;
mod graphql;
mod issues;
mod people;
mod pulls;
mod rules;
mod scm;
mod signature;
mod translate;
mod webhook;

pub use auth::{AppKey, KeyError};
pub use budget::{Budget, Rate};
pub use client::{ClientError, Clock, GithubApp, GithubConfig, SignInClient};
pub use people::{SETUP_PATH, SIGN_IN_CALLBACK_PATH, SIGN_IN_PATH};
pub use translate::{TranslateError, translate};
pub use webhook::{ReceivedDelivery, WEBHOOK_PATH, WebhookError, receive};

//! The GitHub adapter: the GitHub App, its client and its budget per
//! connection, the `Scm` trait on GitHub, and the reading of its signed webhook
//! deliveries. The only crate that knows GitHub: it translates the values of
//! GitHub into the neutral ones of `cihub-scm`. It writes nothing in Postgres;
//! the caller writes the budgets that it reads here.

mod auth;
mod budget;
mod client;
mod scm;
mod signature;
mod translate;
mod webhook;

pub use auth::{AppKey, KeyError};
pub use budget::{Budget, Rate};
pub use client::{ClientError, Clock, GithubApp, GithubConfig};
pub use translate::{TranslateError, translate};
pub use webhook::{ReceivedDelivery, WEBHOOK_PATH, WebhookError, receive};

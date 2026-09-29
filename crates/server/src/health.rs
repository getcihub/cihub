use std::collections::BTreeSet;

use anyhow::Context;
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use crate::config::Role;

/// The body of the probes: only the identity of the binary and its roles
/// (`docs/SECURITY.md`).
#[derive(Clone, Serialize)]
struct Identity {
    name: &'static str,
    version: &'static str,
    roles: BTreeSet<Role>,
}

/// Serves `/healthz` and `/readyz` on the internal address until `cancel`.
pub async fn serve(
    listener: TcpListener,
    roles: BTreeSet<Role>,
    cancel: CancellationToken,
) -> anyhow::Result<()> {
    let identity = Identity {
        name: env!("CARGO_PKG_NAME"),
        version: env!("CARGO_PKG_VERSION"),
        roles,
    };
    let app = Router::new()
        .route("/healthz", get(probe))
        .route("/readyz", get(probe))
        .with_state(identity);
    axum::serve(listener, app)
        .with_graceful_shutdown(cancel.cancelled_owned())
        .await
        .context("internal address server failed")
}

async fn probe(State(identity): State<Identity>) -> Json<Identity> {
    Json(identity)
}

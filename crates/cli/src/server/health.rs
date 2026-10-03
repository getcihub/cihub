use std::collections::BTreeSet;
use std::time::Duration;

use anyhow::Context;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use cihub_store::PgPool;
use serde::Serialize;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use super::Role;

/// The body of the probes: only the identity of the binary and its roles
/// (`docs/SECURITY.md`).
#[derive(Clone, Serialize)]
struct Identity {
    name: &'static str,
    version: &'static str,
    roles: BTreeSet<Role>,
}

const READY_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone)]
struct Probes {
    identity: Identity,
    pool: PgPool,
}

/// Serves `/healthz` and `/readyz` on the internal address until `cancel`.
/// `/readyz` answers `503` while Postgres does not answer within
/// [`READY_TIMEOUT`].
pub async fn serve(
    listener: TcpListener,
    roles: BTreeSet<Role>,
    pool: PgPool,
    cancel: CancellationToken,
) -> anyhow::Result<()> {
    let identity = Identity {
        name: env!("CARGO_BIN_NAME"),
        version: env!("CARGO_PKG_VERSION"),
        roles,
    };
    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .with_state(Probes { identity, pool });
    axum::serve(listener, app)
        .with_graceful_shutdown(cancel.cancelled_owned())
        .await
        .context("internal address server failed")
}

async fn healthz(State(probes): State<Probes>) -> Json<Identity> {
    Json(probes.identity)
}

async fn readyz(State(probes): State<Probes>) -> (StatusCode, Json<Identity>) {
    let ping = tokio::time::timeout(READY_TIMEOUT, cihub_store::ping(&probes.pool));
    let status = if ping.await.unwrap_or(false) {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(probes.identity))
}

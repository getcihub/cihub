//! The HTTP server of the public address of the `web` role (`DECISIONS.md`
//! D82, D91). It serves the webhooks of the SCM: it checks the signature,
//! stores the delivery without routing it, and answers within 2 s (D83, D84);
//! and the API v1 under `/api/v1`, with the routes of the modules (D159). The
//! dashboard arrives with lot 10.
//!
//! It holds no queue logic and makes no decision on a delivery: the routing
//! loop reads it later. The adapter reads what is proper to its SCM.

mod api;

use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use cihub_api_types::API_PREFIX;
use cihub_domain::ScmInstanceId;
use cihub_github::{ReceivedDelivery, WEBHOOK_PATH, WebhookError};
use cihub_platform::{Delivery, ModuleApi, PlatformError, store_delivery};
use cihub_store::{PgPool, begin_system};
use secrecy::SecretString;
use time::OffsetDateTime;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, info, info_span, warn};

/// GitHub sends deliveries of at most 25 MB.
const MAX_DELIVERY_BYTES: usize = 25 * 1024 * 1024;

/// A delivery not stored within this time is refused: the SCM sees a failure,
/// and the sweep asks for it again (`DECISIONS.md` D88).
pub const ACK_WITHIN: Duration = Duration::from_secs(2);

/// The webhooks of the GitHub App of the instance.
pub struct GithubWebhooks {
    pub scm_instance_id: ScmInstanceId,
    pub secret: SecretString,
}

#[derive(Clone)]
struct Webhooks {
    pool: PgPool,
    now: fn() -> OffsetDateTime,
    github: Arc<GithubWebhooks>,
}

pub use api::{REQUESTS_PER_MINUTE, openapi};

/// The routes of the public address: the webhooks of the GitHub App, and the
/// API with the routes of `modules`.
pub fn router(
    pool: PgPool,
    now: fn() -> OffsetDateTime,
    github: GithubWebhooks,
    modules: Vec<ModuleApi>,
) -> Router {
    let webhooks = Webhooks {
        pool: pool.clone(),
        now,
        github: Arc::new(github),
    };
    Router::new()
        .route(WEBHOOK_PATH, post(github_webhook))
        .layer(DefaultBodyLimit::max(MAX_DELIVERY_BYTES))
        .with_state(webhooks)
        .nest(API_PREFIX, api::router(pool, now, modules))
}

/// Serves `router` on `listener` until `cancel`.
///
/// # Errors
///
/// The server fails.
pub async fn serve(
    listener: TcpListener,
    router: Router,
    cancel: CancellationToken,
) -> std::io::Result<()> {
    axum::serve(listener, router)
        .with_graceful_shutdown(cancel.cancelled_owned())
        .await
}

async fn github_webhook(
    State(webhooks): State<Webhooks>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    let received = match cihub_github::receive(&webhooks.github.secret, &headers, &body) {
        Ok(received) => received,
        Err(WebhookError::Signature) => {
            warn!("webhook signature refused");
            return StatusCode::UNAUTHORIZED;
        }
        Err(err) => {
            warn!(error = &err as &dyn Error, "webhook refused");
            return StatusCode::BAD_REQUEST;
        }
    };
    let span = info_span!("webhook", delivery_id = received.delivery_id);
    async move {
        let event = received.event.clone();
        match tokio::time::timeout(ACK_WITHIN, store(&webhooks, received)).await {
            Ok(Ok(true)) => {
                info!(event, "webhook stored");
                StatusCode::ACCEPTED
            }
            Ok(Ok(false)) => {
                info!(event, "webhook copy ignored");
                StatusCode::ACCEPTED
            }
            Ok(Err(err)) => {
                warn!(error = &err as &dyn Error, "webhook not stored");
                StatusCode::SERVICE_UNAVAILABLE
            }
            Err(_) => {
                warn!("webhook not stored in time");
                StatusCode::SERVICE_UNAVAILABLE
            }
        }
    }
    .instrument(span)
    .await
}

/// Stores `received` without routing it (`DECISIONS.md` D84): only the system
/// context sees it until the routing loop routes it.
async fn store(webhooks: &Webhooks, received: ReceivedDelivery) -> Result<bool, PlatformError> {
    let delivery = Delivery {
        scm_instance_id: webhooks.github.scm_instance_id,
        delivery_id: received.delivery_id,
        org_id: None,
        connection_id: None,
        queue_id: None,
        event: received.event,
        action: received.action,
        payload: received.payload,
        received_at: (webhooks.now)(),
    };
    let mut tx = begin_system(&webhooks.pool)
        .await
        .map_err(PlatformError::Store)?;
    let stored = store_delivery(&mut tx, &delivery).await?;
    tx.commit().await.map_err(PlatformError::Query)?;
    Ok(stored)
}

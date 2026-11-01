//! The HTTP server of the public address of the `web` role. It serves the
//! webhooks of the SCM: it checks the signature, stores the delivery without
//! routing it, and answers within 2 s; the API v1 under `/api/v1`, with the
//! routes of the modules; the sign-in and the sessions of the dashboard, and
//! its files, embedded at the build.
//!
//! It holds no queue logic and makes no decision on a delivery: the routing
//! loop reads it later. The adapter reads what is proper to its SCM.

mod api;
mod dashboard;
mod device;
mod session;
mod sign_in;
mod stream;

use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use cihub_api_types::API_PREFIX;
use cihub_domain::ConnectionKind;
use cihub_github::{
    ReceivedDelivery, SETUP_PATH, SIGN_IN_CALLBACK_PATH, SIGN_IN_PATH, WEBHOOK_PATH, WebhookError,
};
use cihub_platform::{Delivery, ModuleApi, PlatformError, QueueNewsFeed, store_delivery};
use cihub_scm::Scm;
use cihub_store::{DatabaseCipher, PgPool, begin_system};
use secrecy::SecretString;
use time::OffsetDateTime;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, info, info_span, warn};

/// GitHub sends deliveries of at most 25 MB.
const MAX_DELIVERY_BYTES: usize = 25 * 1024 * 1024;

const ACK_WITHIN: Duration = Duration::from_secs(2);

pub struct GithubWebhooks {
    pub secret: SecretString,
}

/// What the dashboard needs: the SCM where people sign in, the cipher that
/// seals their tokens, the root address of the instance, which the SCM sends
/// them back to, and the changes of the queues, which its pages follow.
pub struct Dashboard {
    pub scm: Arc<dyn Scm>,
    /// `None` when nobody signs in: the sign-in keeps a token, sealed.
    pub cipher: Option<Arc<DatabaseCipher>>,
    pub public_url: Option<String>,
    /// Proxies in front of the server that append to `X-Forwarded-For`; 0
    /// takes the address of the connection as the address of the client.
    pub trusted_proxy_hops: usize,
    pub news: QueueNewsFeed,
}

#[derive(Clone)]
struct Webhooks {
    pool: PgPool,
    now: fn() -> OffsetDateTime,
    github: Arc<GithubWebhooks>,
}

pub use api::{REQUESTS_PER_MINUTE, openapi};
pub use device::{
    DevicePoll, NewDeviceCode, answer_device_code, create_device_code, new_device_code_secret,
    new_user_code, poll_device_code,
};
pub use session::{
    CLI_SESSION_IDLE_LIFETIME, NewSession, SIGN_OUT_PATH, SessionClient, SessionGrant,
    authenticate_session, create_session, csrf_token, delete_session, delete_session_by_id,
    new_cli_session_secret, new_session_secret,
};

pub fn router(
    pool: PgPool,
    now: fn() -> OffsetDateTime,
    github: GithubWebhooks,
    modules: Vec<ModuleApi>,
    dashboard: Dashboard,
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
        .route(
            SIGN_OUT_PATH,
            post(session::sign_out).with_state(pool.clone()),
        )
        .merge(sign_in_routes(&pool, now, &dashboard))
        .nest(API_PREFIX, api::router(pool, now, modules, dashboard))
        .fallback(get(dashboard::serve))
}

fn sign_in_routes(pool: &PgPool, now: fn() -> OffsetDateTime, dashboard: &Dashboard) -> Router {
    let state = sign_in::SignIn {
        pool: pool.clone(),
        now,
        scm: Arc::clone(&dashboard.scm),
        cipher: dashboard.cipher.clone(),
        kind: ConnectionKind::GithubApp,
        public_url: dashboard.public_url.clone(),
    };
    Router::new()
        .route(SIGN_IN_PATH, get(sign_in::start))
        .route(SIGN_IN_CALLBACK_PATH, get(sign_in::callback))
        .route(SETUP_PATH, get(sign_in::installed))
        .with_state(state)
}

/// # Errors
///
/// The server fails.
pub async fn serve(
    listener: TcpListener,
    router: Router,
    cancel: CancellationToken,
) -> std::io::Result<()> {
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
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
            warn!("webhook.signature.refused");
            return StatusCode::UNAUTHORIZED;
        }
        Err(err) => {
            warn!(error = &err as &dyn Error, "webhook.refused");
            return StatusCode::BAD_REQUEST;
        }
    };
    let span = info_span!("webhook", delivery_id = received.delivery_id);
    async move {
        let event = received.event.clone();
        match tokio::time::timeout(ACK_WITHIN, store(&webhooks, received)).await {
            Ok(Ok(true)) => {
                info!(event, "webhook.stored");
                StatusCode::ACCEPTED
            }
            Ok(Ok(false)) => {
                info!(event, "webhook.copy_ignored");
                StatusCode::ACCEPTED
            }
            Ok(Err(err)) => {
                warn!(error = &err as &dyn Error, "webhook.not_stored");
                StatusCode::SERVICE_UNAVAILABLE
            }
            Err(_) => {
                warn!(reason = "timeout", "webhook.not_stored");
                StatusCode::SERVICE_UNAVAILABLE
            }
        }
    }
    .instrument(span)
    .await
}

async fn store(webhooks: &Webhooks, received: ReceivedDelivery) -> Result<bool, PlatformError> {
    let delivery = Delivery {
        delivery_id: received.delivery_id,
        org_id: None,
        connection_id: None,
        queue_id: None,
        event: received.event,
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

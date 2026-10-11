//! `GET /orgs/{org}/stream`: the changes of the queues of an organization, as
//! server-sent events, for the pages of the dashboard.

use std::time::Duration;

use axum::body::Body;
use axum::extract::State;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use cihub_api_types::QueueChanged;
use cihub_domain::{ApiScope, OrgId};
use cihub_platform::{ApiError, ApiGrant, ApiPath, QueueNews, QueueNewsFeed};
use cihub_store::PgPool;
use tokio::io::{AsyncWriteExt, DuplexStream};
use tokio::sync::broadcast::Receiver;
use tokio::sync::broadcast::error::RecvError;
use tokio::time::Interval;
use tokio_util::io::ReaderStream;
use tokio_util::sync::CancellationToken;

/// A ping keeps the proxies from closing a quiet stream.
const PING_EVERY: Duration = Duration::from_secs(15);

/// How long the browser waits before it opens the stream again.
const RETRY_MS: u64 = 1000;

/// What a stream buffers for a slow page before it waits.
const BUFFER_BYTES: usize = 16 * 1024;

#[derive(Clone)]
pub(crate) struct Streams {
    pub pool: PgPool,
    pub news: QueueNewsFeed,
}

/// The changes of the queues of the organization: an event `queue_changed`
/// for each, and a ping. The stream ends when the server stops, or when the
/// server lost changes: the browser opens it again and reads everything.
#[utoipa::path(
    get,
    path = "/orgs/{org}/stream",
    params(("org" = String, Path, description = "The slug of the organization")),
    responses((status = 200, content_type = "text/event-stream", body = QueueChanged)),
    security(("token" = ["queues:read"]))
)]
pub(crate) async fn stream(
    grant: ApiGrant,
    State(streams): State<Streams>,
    ApiPath(org): ApiPath<String>,
) -> Result<Response, ApiError> {
    grant.require(ApiScope::QueuesRead)?;

    // The organization of the URL: `404` where the caller has no role.
    let tx = grant.begin_in_org(&streams.pool, &org).await?;
    tx.rollback().await?;

    let (writer, reader) = tokio::io::duplex(BUFFER_BYTES);
    let news = streams.news.subscribe();
    let stopped = streams.news.stopped().clone();
    tokio::spawn(write_events(writer, news, stopped, grant.org_id));

    let headers = [
        (CONTENT_TYPE, "text/event-stream"),
        (CACHE_CONTROL, "no-cache"),
    ];

    Ok((headers, Body::from_stream(ReaderStream::new(reader))).into_response())
}

/// Writes the events of `org` until the server stops, the page leaves, or
/// changes are lost.
async fn write_events(
    mut writer: DuplexStream,
    mut news: Receiver<QueueNews>,
    stopped: CancellationToken,
    org: OrgId,
) {
    let mut ping = tokio::time::interval(PING_EVERY);
    let mut text = Some(format!("retry: {RETRY_MS}\n\n"));

    while let Some(next) = text {
        if writer.write_all(next.as_bytes()).await.is_err() {
            return;
        }
        text = next_text(&mut news, &mut ping, &stopped, org).await;
    }
}

/// The next text of the stream; `None` once it ends.
async fn next_text(
    news: &mut Receiver<QueueNews>,
    ping: &mut Interval,
    stopped: &CancellationToken,
    org: OrgId,
) -> Option<String> {
    loop {
        let received = tokio::select! {
            () = stopped.cancelled() => return None,
            _ = ping.tick() => return Some(": ping\n\n".to_owned()),
            received = news.recv() => received,
        };
        match received {
            Ok(QueueNews::Changed(change)) if change.org_id == org => {
                let data = QueueChanged {
                    queue_id: change.queue_id.to_string(),
                    seq: change.seq,
                };
                let data = serde_json::to_string(&data).ok()?;
                return Some(format!("event: queue_changed\ndata: {data}\n\n"));
            }
            Ok(QueueNews::Changed(_)) => {}
            // The page lagged behind, or the server lost changes: it reads
            // everything again on a new stream.
            Ok(QueueNews::Lost) | Err(RecvError::Lagged(_) | RecvError::Closed) => return None,
        }
    }
}

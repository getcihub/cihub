//! `GET /orgs/{org}/stream`: the changes of the queues of an organization, as
//! server-sent events, for the pages of the dashboard. A person hears only the
//! queues of the repositories that they read.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use axum::body::Body;
use axum::extract::State;
use axum::http::HeaderName;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use cihub_api_types::{ErrorCode, QueueChanged};
use cihub_domain::{ApiScope, OrgId, QueueId};
use cihub_platform::{
    ApiError, ApiGrant, ApiPath, GrantActor, QueueNews, QueueNewsFeed, error_responses,
};
use cihub_store::{PgPool, begin_org};
use tokio::io::{AsyncWriteExt, DuplexStream};
use tokio::sync::broadcast::Receiver;
use tokio::sync::broadcast::error::RecvError;
use tokio::time::{Instant, Interval};
use tokio_util::io::ReaderStream;
use tokio_util::sync::CancellationToken;
use utoipa::openapi::{self, RefOr};

/// A ping keeps the proxies from closing a quiet stream.
const PING_EVERY: Duration = Duration::from_secs(15);

/// A stream ends after this long, and the browser opens it again with its
/// session or token: a grant revoked meanwhile no longer hears the changes.
const STREAM_LIFETIME: Duration = Duration::from_mins(5);

/// A page that reads nothing for this long is gone: its stream ends.
const WRITE_WITHIN: Duration = Duration::from_secs(30);

/// How long the browser waits before it opens the stream again.
const RETRY_MS: u64 = 1000;

/// What a stream buffers for a slow page before it waits.
const BUFFER_BYTES: usize = 16 * 1024;

/// The streams that a token or a person keeps open at once, a few tabs.
const STREAMS_PER_CALLER: usize = 8;

/// The errors of the stream that a route of the platform does not answer.
pub(crate) struct StreamErrors;

impl utoipa::IntoResponses for StreamErrors {
    fn responses() -> BTreeMap<String, RefOr<openapi::Response>> {
        error_responses(&[ErrorCode::TooManyStreams])
    }
}

#[derive(Clone)]
pub(crate) struct Streams {
    pub pool: PgPool,
    pub news: QueueNewsFeed,
    pub open: Arc<Mutex<HashMap<GrantActor, usize>>>,
}

/// One open stream of a caller, given back when the stream ends.
struct Opened {
    open: Arc<Mutex<HashMap<GrantActor, usize>>>,
    caller: GrantActor,
}

impl Drop for Opened {
    fn drop(&mut self) {
        let mut open = self.open.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(count) = open.get_mut(&self.caller) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                open.remove(&self.caller);
            }
        }
    }
}

/// Follow the queues
///
/// The changes of the queues of the organization, as server-sent events: an
/// event `queue_changed` each time what the pages of a queue show changes,
/// whose data is a `QueueChanged`, and a comment `: ping` every 15 seconds.
/// The stream ends after 5 minutes, when the server stops, and when it lost
/// changes: open it again after `retry` milliseconds, and read the queues
/// again.
#[utoipa::path(
    get,
    path = "/orgs/{org}/stream",
    tag = "Live updates",
    params(("org" = String, Path, description = "The slug of the organization.", example = "acme")),
    responses(
        (
            status = 200,
            description = "The stream of the changes.",
            content_type = "text/event-stream",
            body = QueueChanged
        ),
        StreamErrors
    ),
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

    let opened = open_for(&streams, grant.actor)?;
    let (writer, reader) = tokio::io::duplex(BUFFER_BYTES);
    let news = streams.news.subscribe();
    let stopped = streams.news.stopped().clone();
    let mut audience = Audience {
        pool: streams.pool.clone(),
        org: grant.org_id,
        repos: grant.repo_filter().map(|repos| repos.into_iter().collect()),
        queues: HashMap::new(),
    };
    tokio::spawn(async move {
        write_events(writer, news, &stopped, &mut audience).await;
        drop(opened);
    });

    let headers = [
        (CONTENT_TYPE, "text/event-stream"),
        (CACHE_CONTROL, "no-cache"),
        // nginx would hold the events back.
        (HeaderName::from_static("x-accel-buffering"), "no"),
    ];

    Ok((headers, Body::from_stream(ReaderStream::new(reader))).into_response())
}

/// Counts one more stream of `caller`; `429` past [`STREAMS_PER_CALLER`].
fn open_for(streams: &Streams, caller: GrantActor) -> Result<Opened, ApiError> {
    let mut open = streams.open.lock().unwrap_or_else(PoisonError::into_inner);
    let count = open.entry(caller).or_default();

    if *count >= STREAMS_PER_CALLER {
        return Err(ApiError::new(
            ErrorCode::TooManyStreams,
            format!("at most {STREAMS_PER_CALLER} streams at once per token or person"),
        ));
    }

    *count += 1;

    Ok(Opened {
        open: Arc::clone(&streams.open),
        caller,
    })
}

/// Who a stream speaks to: the organization, and the repositories that a
/// person reads; `None` for every repository, for a token.
struct Audience {
    pool: PgPool,
    org: OrgId,
    repos: Option<HashSet<String>>,
    /// Whether the stream shows each queue that it met.
    queues: HashMap<QueueId, bool>,
}

impl Audience {
    /// Whether the stream shows the queue `queue_id`; `None` when Postgres
    /// fails, which ends the stream. A queue created after the stream opened
    /// is read once.
    async fn shows(&mut self, queue_id: QueueId) -> Option<bool> {
        let Some(repos) = &self.repos else {
            return Some(true);
        };
        if let Some(shown) = self.queues.get(&queue_id) {
            return Some(*shown);
        }

        let mut tx = begin_org(&self.pool, self.org).await.ok()?;
        let repo: Option<String> = sqlx::query_scalar("SELECT repo_id FROM queues WHERE id = $1")
            .bind(queue_id.to_string())
            .fetch_optional(&mut *tx)
            .await
            .ok()?;
        let shown = repo.is_some_and(|repo| repos.contains(&repo));
        self.queues.insert(queue_id, shown);
        Some(shown)
    }
}

/// Writes the events of the organization of `audience` until the server
/// stops, the page leaves or reads nothing, changes are lost, or the stream
/// is old enough.
async fn write_events(
    mut writer: DuplexStream,
    mut news: Receiver<QueueNews>,
    stopped: &CancellationToken,
    audience: &mut Audience,
) {
    let mut ping = tokio::time::interval(PING_EVERY);
    let end = Instant::now() + STREAM_LIFETIME;
    let mut text = Some(format!("retry: {RETRY_MS}\n\n"));

    while let Some(next) = text {
        let written = tokio::select! {
            () = stopped.cancelled() => return,
            written = tokio::time::timeout(WRITE_WITHIN, writer.write_all(next.as_bytes())) => written,
        };
        if !matches!(written, Ok(Ok(()))) {
            return;
        }
        text = next_text(&mut news, &mut ping, stopped, end, audience).await;
    }
}

/// The next text of the stream; `None` once it ends.
async fn next_text(
    news: &mut Receiver<QueueNews>,
    ping: &mut Interval,
    stopped: &CancellationToken,
    end: Instant,
    audience: &mut Audience,
) -> Option<String> {
    loop {
        let received = tokio::select! {
            () = stopped.cancelled() => return None,
            () = tokio::time::sleep_until(end) => return None,
            _ = ping.tick() => return Some(": ping\n\n".to_owned()),
            received = news.recv() => received,
        };
        match received {
            Ok(QueueNews::Changed(change)) if change.org_id == audience.org => {
                if !audience.shows(change.queue_id).await? {
                    continue;
                }
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

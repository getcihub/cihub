//! A fake GitHub that gives the recorded answers in their order. It serves
//! the paths of a GitHub Enterprise Server, so that `GithubApp` reaches it by
//! its address.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{Method, StatusCode, Uri};
use axum::response::Response;
use tokio::net::TcpListener;

use super::cassette::{Exchange, api_path};

#[derive(Default)]
struct Calls {
    left: VecDeque<Exchange>,
    mismatch: Option<String>,
}

pub struct Replay {
    /// The address to give to `GithubConfig::url`.
    pub url: String,
    calls: Arc<Mutex<Calls>>,
}

impl Replay {
    pub async fn start(exchanges: Vec<Exchange>) -> Self {
        let calls = Arc::new(Mutex::new(Calls {
            left: exchanges.into(),
            mismatch: None,
        }));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let router = Router::new()
            .fallback(answer)
            .with_state(Arc::clone(&calls));
        tokio::spawn(async move { axum::serve(listener, router).await });
        Self { url, calls }
    }

    /// # Panics
    ///
    /// When a call differed from the recording, or a recorded call did not
    /// come.
    pub fn finish(&self) {
        let calls = self.calls.lock().unwrap();
        assert_eq!(calls.mismatch, None);
        let left: Vec<_> = calls
            .left
            .iter()
            .map(|e| format!("{} {}", e.method, e.path))
            .collect();
        assert!(
            left.is_empty(),
            "recorded calls that did not come: {left:?}"
        );
    }
}

async fn answer(
    State(calls): State<Arc<Mutex<Calls>>>,
    method: Method,
    uri: Uri,
    _body: Bytes,
) -> Response {
    let full = uri.path_and_query().map_or(uri.path(), |p| p.as_str());
    let path = api_path(full).unwrap_or(full);
    let mut calls = calls.lock().unwrap();
    match calls.left.pop_front() {
        Some(exchange) if exchange.method == method.as_str() && exchange.path == path => {
            let mut response = Response::builder().status(exchange.status);
            for (name, value) in &exchange.headers {
                response = response.header(name, value);
            }
            let body = match (&exchange.body, &exchange.text) {
                (Some(body), _) => body.to_string(),
                (None, Some(text)) => text.clone(),
                (None, None) => String::new(),
            };
            response.body(Body::from(body)).unwrap()
        }
        expected => {
            let expected = expected.map(|e| format!("{} {}", e.method, e.path));
            let message = format!("replay: expected {expected:?}, got {method} {path}");
            calls.mismatch.get_or_insert(message.clone());
            Response::builder()
                .status(StatusCode::IM_A_TEAPOT)
                .body(Body::from(
                    serde_json::json!({ "message": message }).to_string(),
                ))
                .unwrap()
        }
    }
}

//! The files of the dashboard, embedded at the build (`build.rs`). A path of
//! the dashboard that is not a file, such as `/sign-in` or `/orgs/acme`, gets
//! `index.html`: the router of the page reads the path.

use std::fmt::Write;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::header::{
    CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE, X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS,
};
use axum::http::{StatusCode, Uri};
use axum::response::{IntoResponse, Response};

use crate::BrowserErrorReports;

include!(concat!(env!("OUT_DIR"), "/dashboard.rs"));

/// The paths that are never pages of the dashboard: the server, and the
/// assets, whose names change at each build.
const NOT_PAGES: [&str; 4] = ["/api/", "/auth/", "/webhooks/", "/assets/"];
const INDEX: &str = "/index.html";

/// `index.html`, with the settings of the error reports when the instance
/// has them; `None` without a build of the dashboard.
#[derive(Clone)]
pub(crate) struct Index(Option<Bytes>);

impl Index {
    pub(crate) fn new(error_reports: Option<&BrowserErrorReports>) -> Self {
        let index = file(INDEX).map(|body| match error_reports {
            None => Bytes::from_static(body),
            Some(error_reports) => Bytes::from(with_settings(body, error_reports)),
        });
        Self(index)
    }
}

pub(crate) async fn serve(State(index): State<Index>, uri: Uri) -> Response {
    let path = uri.path();
    if let Some(body) = file(path).filter(|_| path != INDEX) {
        return answer(path, Bytes::from_static(body));
    }

    let is_page = !NOT_PAGES.iter().any(|prefix| path.starts_with(prefix));
    match index.0 {
        Some(body) if is_page => answer(INDEX, body),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

/// The settings as `<meta>` elements at the end of `<head>`, where the
/// scripts of the dashboard read them before they start.
fn with_settings(index: &[u8], error_reports: &BrowserErrorReports) -> String {
    let settings = [
        ("sentry-dsn", Some(error_reports.dsn.as_str())),
        ("sentry-environment", error_reports.environment.as_deref()),
        ("sentry-release", Some(error_reports.release)),
    ];
    let mut meta = String::new();
    for (name, value) in settings
        .iter()
        .filter_map(|(name, value)| Some((name, (*value)?)))
    {
        let _ = write!(
            meta,
            r#"<meta name="{name}" content="{}" />"#,
            escape(value)
        );
    }
    String::from_utf8_lossy(index).replacen("</head>", &format!("{meta}</head>"), 1)
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn file(path: &str) -> Option<&'static [u8]> {
    FILES
        .iter()
        .find(|(name, _)| *name == path)
        .map(|(_, body)| *body)
}

/// Vite names each asset under `/assets/` by its content, so an asset never
/// changes; any other file is read again at each visit, so that a deploy
/// shows at once. No other site may frame a page, as its buttons act.
fn answer(path: &str, body: Bytes) -> Response {
    let cache = if path.starts_with("/assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    let headers = [
        (CONTENT_TYPE, content_type(path)),
        (CACHE_CONTROL, cache),
        (X_CONTENT_TYPE_OPTIONS, "nosniff"),
        (X_FRAME_OPTIONS, "DENY"),
        (CONTENT_SECURITY_POLICY, "frame-ancestors 'none'"),
    ];
    (headers, body).into_response()
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit_once('.').map(|(_, extension)| extension) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("json") => "application/json",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn status_of(path: &str) -> StatusCode {
        serve(State(Index::new(None)), path.parse().unwrap())
            .await
            .status()
    }

    #[tokio::test]
    async fn a_page_of_the_dashboard_gets_the_index_and_the_server_paths_do_not() {
        let has_build = file(INDEX).is_some();
        let page = if has_build {
            StatusCode::OK
        } else {
            StatusCode::NOT_FOUND
        };

        assert_eq!(status_of("/sign-in").await, page);
        assert_eq!(status_of("/orgs/acme").await, page);
        for path in [
            "/api/v1/nothing",
            "/auth/nothing",
            "/webhooks/nothing",
            "/assets/old.js",
        ] {
            assert_eq!(status_of(path).await, StatusCode::NOT_FOUND, "{path}");
        }
    }

    #[test]
    fn the_index_carries_the_settings_of_the_error_reports() {
        let index = b"<html><head><title>CIHub</title></head><body></body></html>";
        let error_reports = BrowserErrorReports {
            dsn: "https://key@sentry.example.com/2?a=1&b=\"".to_owned(),
            environment: None,
            release: "cihub@1.2.3",
        };

        let page = with_settings(index, &error_reports);

        assert_eq!(
            page,
            "<html><head><title>CIHub</title>\
             <meta name=\"sentry-dsn\" content=\"https://key@sentry.example.com/2?a=1&amp;b=&quot;\" />\
             <meta name=\"sentry-release\" content=\"cihub@1.2.3\" />\
             </head><body></body></html>"
        );
    }

    #[test]
    fn an_asset_has_the_type_of_its_extension() {
        assert_eq!(
            content_type("/assets/index-1.js"),
            "text/javascript; charset=utf-8"
        );
        assert_eq!(content_type("/index.html"), "text/html; charset=utf-8");
        assert_eq!(content_type("/assets/x"), "application/octet-stream");
    }
}

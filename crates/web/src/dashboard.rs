//! The files of the dashboard, embedded at the build (`build.rs`). A path of
//! the dashboard that is not a file, such as `/sign-in` or `/orgs/acme`, gets
//! `index.html`: the router of the page reads the path.

use axum::http::header::{
    CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE, X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS,
};
use axum::http::{StatusCode, Uri};
use axum::response::{IntoResponse, Response};

include!(concat!(env!("OUT_DIR"), "/dashboard.rs"));

/// The paths that are never pages of the dashboard: the server, and the
/// assets, whose names change at each build.
const NOT_PAGES: [&str; 4] = ["/api/", "/auth/", "/webhooks/", "/assets/"];
const INDEX: &str = "/index.html";

pub(crate) async fn serve(uri: Uri) -> Response {
    let path = uri.path();
    if let Some(body) = file(path) {
        return answer(path, body);
    }

    let is_page = !NOT_PAGES.iter().any(|prefix| path.starts_with(prefix));
    match file(INDEX) {
        Some(body) if is_page => answer(INDEX, body),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
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
fn answer(path: &str, body: &'static [u8]) -> Response {
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
        serve(path.parse().unwrap()).await.status()
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
    fn an_asset_has_the_type_of_its_extension() {
        assert_eq!(
            content_type("/assets/index-1.js"),
            "text/javascript; charset=utf-8"
        );
        assert_eq!(content_type("/index.html"), "text/html; charset=utf-8");
        assert_eq!(content_type("/assets/x"), "application/octet-stream");
    }
}

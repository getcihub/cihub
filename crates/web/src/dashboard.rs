//! The dashboard of `web/`, served on the paths that no other route takes
//! (`DECISIONS.md` D174). A release build embeds `web/dist`; a debug build
//! reads it from the disk at each request, so `mise run web-build` is enough
//! to see a change.

use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use rust_embed::{Embed, EmbeddedFile};

#[derive(Embed)]
#[folder = "../../web/dist"]
struct Assets;

const INDEX: &str = "index.html";

/// Vite names the files of this directory after their content: a file never
/// changes under the same name.
const HASHED_DIR: &str = "assets/";

pub(crate) async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if let Some(file) = Assets::get(path) {
        return asset(path, file);
    }
    // A path of the app has no file: the app reads it. A missing hashed file
    // is a build that no longer exists, not a page.
    if path.starts_with(HASHED_DIR) {
        return StatusCode::NOT_FOUND.into_response();
    }
    match Assets::get(INDEX) {
        Some(file) => asset(INDEX, file),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

fn asset(path: &str, file: EmbeddedFile) -> Response {
    let cache = if path.starts_with(HASHED_DIR) {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    let mime = file.metadata.mimetype().to_owned();
    (
        [
            (header::CONTENT_TYPE, mime.as_str()),
            (header::CACHE_CONTROL, cache),
        ],
        file.data,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

    use super::*;

    async fn get(path: &str) -> Response {
        serve(path.parse().unwrap()).await
    }

    fn header<'a>(response: &'a Response, name: &header::HeaderName) -> &'a str {
        response.headers()[name].to_str().unwrap()
    }

    #[tokio::test]
    async fn the_root_and_the_paths_of_the_app_serve_the_index() {
        for path in ["/", "/queues/01J0000000000000000000000"] {
            let response = get(path).await;
            assert_eq!(response.status(), StatusCode::OK, "{path}");
            assert_eq!(header(&response, &header::CONTENT_TYPE), "text/html");
            assert_eq!(header(&response, &header::CACHE_CONTROL), "no-cache");
        }
    }

    #[tokio::test]
    async fn a_hashed_file_is_cached_forever() {
        let path = Assets::iter()
            .find(|path| path.starts_with(HASHED_DIR))
            .unwrap();
        let response = get(&format!("/{path}")).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            header(&response, &header::CACHE_CONTROL),
            "public, max-age=31536000, immutable"
        );
    }

    #[tokio::test]
    async fn a_missing_hashed_file_is_not_found() {
        let response = get("/assets/index-missing.js").await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}

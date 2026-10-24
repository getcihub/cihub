//! The registration of the App from a manifest (`specs/GITHUB.md`): the
//! manifest that carries the addresses, the permissions and the events of the
//! App, the page of GitHub that creates it, the exchange of the code that
//! GitHub gives back for the keys of the new App, and its webhook secret.

use std::fmt::Write;

use reqwest::{Client, StatusCode, header};
use ring::rand::{SecureRandom, SystemRandom};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use time::OffsetDateTime;

use crate::auth::{AppKey, KeyError};
use crate::client::{CALL_TIMEOUT, JSON, api_url};
use crate::scm::encode;
use crate::{SETUP_PATH, SIGN_IN_CALLBACK_PATH, WEBHOOK_PATH};

/// The permissions of the App in `specs/GITHUB.md`, without the optional
/// `actions`.
const PERMISSIONS: [(&str, &str); 8] = [
    ("administration", "read"),
    ("checks", "read"),
    ("contents", "write"),
    ("issues", "write"),
    ("members", "read"),
    ("metadata", "read"),
    ("pull_requests", "write"),
    ("statuses", "write"),
];

/// The events of `specs/GITHUB.md`; GitHub sends those of the installation
/// without a subscription.
const EVENTS: [&str; 9] = [
    "branch_protection_rule",
    "check_run",
    "check_suite",
    "issue_comment",
    "pull_request",
    "pull_request_review",
    "push",
    "repository",
    "status",
];

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("the call to GitHub failed")]
    Http(#[source] reqwest::Error),
    #[error(
        "GitHub refused the code of the new App ({0}); it lasts one hour, create the App again"
    )]
    Refused(String),
    #[error("the answer of GitHub is not understood: {0}")]
    Protocol(String),
    #[error("the private key that GitHub gave is not valid")]
    Key(#[source] KeyError),
    #[error(
        "GitHub refused the webhook secret of the new App ({0}): set one by hand on the page of the App"
    )]
    HookRefused(String),
    #[error("the system has no random source")]
    Random,
}

/// The new App and its keys, which GitHub gives once.
#[derive(Debug)]
pub struct CreatedApp {
    pub id: u64,
    pub name: String,
    /// The page of the App on GitHub.
    pub html_url: String,
    pub client_id: String,
    pub client_secret: SecretString,
    pub webhook_secret: SecretString,
    /// The private key, in PEM.
    pub pem: SecretString,
}

/// The manifest of the App of the instance at `public_url`. GitHub sends the
/// person to `redirect_url` with a code once the App exists. The App is
/// private: any account that installs it gets an organization.
pub fn app_manifest(name: &str, public_url: &str, redirect_url: &str) -> Value {
    let public_url = public_url.trim_end_matches('/');
    let permissions: Map<String, Value> = PERMISSIONS
        .iter()
        .map(|(permission, access)| ((*permission).to_owned(), Value::from(*access)))
        .collect();

    json!({
        "name": name,
        "url": public_url,
        "hook_attributes": { "url": format!("{public_url}{WEBHOOK_PATH}"), "active": true },
        "redirect_url": redirect_url,
        "callback_urls": [format!("{public_url}{SIGN_IN_CALLBACK_PATH}")],
        "setup_url": format!("{public_url}{SETUP_PATH}"),
        "setup_on_update": true,
        "request_oauth_on_install": false,
        "public": false,
        "default_permissions": permissions,
        "default_events": EVENTS,
    })
}

/// The page of GitHub that creates an App from a manifest posted to it: under
/// the organization `owner`, or under the account of the person without one.
/// GitHub gives `state` back with the code.
pub fn new_app_url(github_url: &str, owner: Option<&str>, state: &str) -> String {
    let web_url = github_url.trim_end_matches('/');
    match owner {
        Some(owner) => format!(
            "{web_url}/organizations/{}/settings/apps/new?state={}",
            encode(owner),
            encode(state)
        ),
        None => format!("{web_url}/settings/apps/new?state={}", encode(state)),
    }
}

/// A new random secret, 32 bytes in hexadecimal: the `state` of the page that
/// creates the App, and the webhook secret of the App.
///
/// # Errors
///
/// The system has no random source.
pub fn new_secret() -> Result<SecretString, ManifestError> {
    let mut bytes = [0u8; 32];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| ManifestError::Random)?;
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        // Writing to a String does not fail.
        let _ = write!(hex, "{byte:02x}");
    }
    Ok(hex.into())
}

/// Exchanges the code that GitHub gave back for the keys of the new App, then
/// gives the App `webhook_secret`, with a call that the private key signs: the
/// secret is the one of CIHub, and the key is proven to work. The exchange
/// needs no authentication: the code, valid one hour, is the proof.
///
/// # Errors
///
/// A call fails, GitHub refuses the code or the secret, its answer is not
/// understood, or the key that it gives is not valid.
pub async fn register_app(
    github_url: &str,
    code: &str,
    webhook_secret: SecretString,
    now: OffsetDateTime,
) -> Result<CreatedApp, ManifestError> {
    #[derive(Deserialize)]
    struct Conversion {
        id: u64,
        name: String,
        html_url: String,
        client_id: String,
        client_secret: String,
        pem: String,
    }

    // reqwest uses rustls without a provider: ring is the provider of the
    // process. A second install changes nothing.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let http = Client::builder()
        .user_agent("cihub")
        .timeout(CALL_TIMEOUT)
        .build()
        .map_err(ManifestError::Http)?;
    let api_url = api_url(github_url.trim_end_matches('/'));

    let response = http
        .post(format!(
            "{api_url}/app-manifests/{}/conversions",
            encode(code)
        ))
        .header(header::ACCEPT, JSON)
        .send()
        .await
        .map_err(ManifestError::Http)?;
    let status = response.status();
    let body = response.bytes().await.map_err(ManifestError::Http)?;
    if status != StatusCode::CREATED {
        return Err(ManifestError::Refused(format!("HTTP {status}")));
    }
    let app: Conversion =
        serde_json::from_slice(&body).map_err(|err| ManifestError::Protocol(err.to_string()))?;

    let pem: SecretString = app.pem.into();
    let jwt = AppKey::from_pem(&pem)
        .and_then(|key| key.jwt(&app.id.to_string(), now))
        .map_err(ManifestError::Key)?;
    let hook = json!({ "secret": webhook_secret.expose_secret(), "content_type": "json" });
    let response = http
        .patch(format!("{api_url}/app/hook/config"))
        .header(header::ACCEPT, JSON)
        .bearer_auth(jwt)
        .header(header::CONTENT_TYPE, "application/json")
        .body(hook.to_string())
        .send()
        .await
        .map_err(ManifestError::Http)?;
    if response.status() != StatusCode::OK {
        return Err(ManifestError::HookRefused(format!(
            "HTTP {}",
            response.status()
        )));
    }

    Ok(CreatedApp {
        id: app.id,
        name: app.name,
        html_url: app.html_url,
        client_id: app.client_id,
        client_secret: app.client_secret.into(),
        webhook_secret,
        pem,
    })
}

#[cfg(test)]
mod tests {
    use axum::Router;
    use axum::extract::Path;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::{patch, post};
    use time::macros::datetime;

    use super::*;

    const TEST_KEY: &str = include_str!("../tests/test-only-key.pem");

    #[test]
    fn the_manifest_gives_the_addresses_of_the_instance() {
        let manifest = app_manifest(
            "Acme CIHub",
            "https://cihub.example.com/",
            "http://localhost:3999/callback",
        );

        assert_eq!(manifest["url"], "https://cihub.example.com");
        assert_eq!(
            manifest["hook_attributes"]["url"],
            "https://cihub.example.com/webhooks/github"
        );
        assert_eq!(
            manifest["callback_urls"][0],
            "https://cihub.example.com/auth/github/callback"
        );
        assert_eq!(
            manifest["setup_url"],
            "https://cihub.example.com/auth/github/installed"
        );
        assert_eq!(manifest["redirect_url"], "http://localhost:3999/callback");
        assert_eq!(manifest["public"], false);
        assert_eq!(manifest["request_oauth_on_install"], false);
    }

    #[test]
    fn the_manifest_asks_the_permissions_and_the_events_of_the_spec() {
        let manifest = app_manifest("Acme CIHub", "https://cihub.example.com", "http://x");

        assert_eq!(manifest["default_permissions"]["contents"], "write");
        assert_eq!(manifest["default_permissions"]["members"], "read");
        assert!(manifest["default_permissions"].get("actions").is_none());
        assert_eq!(manifest["default_events"].as_array().map(Vec::len), Some(9));
    }

    #[test]
    fn the_page_that_creates_the_app_names_its_owner() {
        assert_eq!(
            new_app_url("https://github.com", Some("acme"), "f00d"),
            "https://github.com/organizations/acme/settings/apps/new?state=f00d"
        );
        assert_eq!(
            new_app_url("https://ghe.example.com/", None, "f00d"),
            "https://ghe.example.com/settings/apps/new?state=f00d"
        );
    }

    #[test]
    fn each_secret_is_new() {
        let first = new_secret().unwrap();
        let second = new_secret().unwrap();

        assert_eq!(first.expose_secret().len(), 64);
        assert_ne!(first.expose_secret(), second.expose_secret());
    }

    #[tokio::test]
    async fn the_code_gives_the_keys_and_the_app_takes_the_secret_of_cihub() {
        let github = Router::new()
            .route(
                "/api/v3/app-manifests/{code}/conversions",
                post(|Path(code): Path<String>| async move {
                    if code != "c0de" {
                        return (StatusCode::NOT_FOUND, String::new());
                    }
                    let body = json!({
                        "id": 42,
                        "name": "Acme CIHub",
                        "html_url": "https://github.com/apps/acme-cihub",
                        "client_id": "Iv23li",
                        "client_secret": "client-secret",
                        "webhook_secret": "the-secret-of-github",
                        "pem": TEST_KEY,
                    });
                    (StatusCode::CREATED, body.to_string())
                }),
            )
            .route(
                "/api/v3/app/hook/config",
                patch(|headers: HeaderMap, body: String| async move {
                    let signed = headers
                        .get("authorization")
                        .and_then(|value| value.to_str().ok())
                        .is_some_and(|value| value.starts_with("Bearer ey"));
                    let hook: Value = serde_json::from_str(&body).unwrap();
                    if signed && hook["secret"] == "the-secret-of-cihub" {
                        StatusCode::OK
                    } else {
                        StatusCode::UNPROCESSABLE_ENTITY
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, github).await });
        let now = datetime!(2026-10-24 12:00 UTC);

        let app = register_app(&url, "c0de", "the-secret-of-cihub".into(), now)
            .await
            .unwrap();
        let refused = register_app(&url, "stale", "the-secret-of-cihub".into(), now)
            .await
            .unwrap_err();
        let hook_refused = register_app(&url, "c0de", "another".into(), now)
            .await
            .unwrap_err();

        assert_eq!(app.id, 42);
        assert_eq!(app.client_id, "Iv23li");
        assert_eq!(app.webhook_secret.expose_secret(), "the-secret-of-cihub");
        assert_eq!(app.pem.expose_secret(), TEST_KEY);
        assert!(matches!(refused, ManifestError::Refused(_)), "{refused}");
        assert!(
            matches!(hook_refused, ManifestError::HookRefused(_)),
            "{hook_refused}"
        );
    }
}

//! The sign-in to the dashboard through the SCM (`specs/GITHUB.md`): its start
//! sends the person to the SCM, and its callback creates the session. The
//! Setup URL of the App starts it again after an install or an update. A
//! failure brings the person back to the sign-in page with `?error=failed`,
//! and the log says why. A person without an organization gets a session too:
//! the dashboard then asks them to install the App.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::http::header::SET_COOKIE;
use axum::response::{AppendHeaders, IntoResponse, Redirect, Response};
use cihub_domain::{ConnectionKind, JobClass, SessionId, UserId};
use cihub_github::SIGN_IN_CALLBACK_PATH;
use cihub_platform::{PlatformError, record_sign_in};
use cihub_scm::{Scm, ScmError, SignInCode, SignInRequest, code_challenge};
use cihub_store::{
    DatabaseCipher, NewSession, PgPool, StoreError, begin_system, create_session, csrf_token,
    new_session_secret, new_sign_in_secret,
};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use time::{Duration, OffsetDateTime};
use tracing::{info, warn};

use crate::session::{CSRF_COOKIE, SESSION_COOKIE, cookie, is_same_text, set_cookie};

/// The cookie that keeps the `state` and the PKCE verifier of a sign-in in
/// progress, the installation that the Setup URL named, and the page to open
/// after it.
const SIGN_IN_COOKIE: &str = "__Host-cihub_sign_in";
const SIGN_IN_SECONDS: i64 = 600;
pub(crate) const SESSION_LIFETIME: Duration = Duration::days(14);
const FAILED: &str = "/sign-in?error=failed";
/// The page of a person whom the Setup URL brought back, when the installation
/// that it named is not one of their organizations.
const WELCOME: &str = "/welcome";

#[derive(Clone)]
pub(crate) struct SignIn {
    pub pool: PgPool,
    pub now: fn() -> OffsetDateTime,
    pub scm: Arc<dyn Scm>,
    pub cipher: Option<Arc<DatabaseCipher>>,
    pub kind: ConnectionKind,
    /// The root address of the instance; without it, nobody signs in.
    pub public_url: Option<String>,
}

#[derive(Debug, thiserror::Error)]
enum SignInFailure {
    #[error(
        "the instance has no public address or no database secret, or the App has no client for \
         the sign-in"
    )]
    NotConfigured,
    #[error("no sign-in in progress in this browser")]
    NotStarted,
    #[error("the SCM gave no code")]
    NoCode,
    #[error("the state of the callback is not the one of the sign-in")]
    OtherState,
    #[error("SCM call failed")]
    Scm(#[source] ScmError),
    #[error("store failed")]
    Store(#[source] StoreError),
    #[error("platform failed")]
    Platform(#[source] PlatformError),
    #[error("query failed")]
    Query(#[source] sqlx::Error),
}

#[derive(Deserialize)]
pub(crate) struct Start {
    /// The page of the dashboard to open once signed in.
    next: Option<String>,
}

/// Sends the person to the page of the SCM where they sign in.
pub(crate) async fn start(State(sign_in): State<SignIn>, Query(start): Query<Start>) -> Response {
    sign_in_again(&sign_in, "", page_after(start.next.as_deref()))
}

#[derive(Deserialize)]
pub(crate) struct Installed {
    installation_id: Option<String>,
    /// `install` or `update`.
    setup_action: Option<String>,
}

/// Signs again the person whom the SCM sends back after an install or an
/// update of the App. The installation that the address names proves
/// nothing: anyone can write it. Only the installations that the token of
/// the person lists count, and the one of the address chooses the page to
/// open once signed in.
pub(crate) async fn installed(
    State(sign_in): State<SignIn>,
    Query(installed): Query<Installed>,
) -> Response {
    let setup = setup_of(&installed).unwrap_or_default();

    sign_in_again(&sign_in, &setup, "/")
}

/// `<installation>-<action>`, with digits and `install` or `update` only, so
/// that the cookie keeps nothing else.
fn setup_of(installed: &Installed) -> Option<String> {
    let installation = installed.installation_id.as_deref()?;
    let action = installed.setup_action.as_deref().unwrap_or("install");
    let is_number = !installation.is_empty() && installation.bytes().all(|b| b.is_ascii_digit());
    let is_action = matches!(action, "install" | "update");

    (is_number && is_action).then(|| format!("{installation}-{action}"))
}

fn sign_in_again(sign_in: &SignIn, setup: &str, next: &str) -> Response {
    match address_of_sign_in(sign_in, setup, next) {
        Ok((address, kept)) => (
            AppendHeaders([(
                SET_COOKIE,
                set_cookie(SIGN_IN_COOKIE, &kept, SIGN_IN_SECONDS),
            )]),
            Redirect::to(&address),
        )
            .into_response(),
        Err(err) => failed(&err),
    }
}

/// The address of the sign-in on the SCM, and the value of the cookie that
/// keeps what the callback checks.
fn address_of_sign_in(
    sign_in: &SignIn,
    setup: &str,
    next: &str,
) -> Result<(String, String), SignInFailure> {
    let redirect_url = callback_url(sign_in)?;
    let state = new_sign_in_secret().map_err(SignInFailure::Store)?;
    let verifier = new_sign_in_secret().map_err(SignInFailure::Store)?;
    let request = SignInRequest {
        redirect_url,
        state: state.expose_secret().to_owned(),
        code_challenge: code_challenge(&verifier),
    };

    let address = sign_in
        .scm
        .sign_in_url(&request)
        .ok_or(SignInFailure::NotConfigured)?;
    let kept = format!(
        "{}.{}.{setup}.{next}",
        request.state,
        verifier.expose_secret(),
    );
    Ok((address, kept))
}

/// The address where the SCM sends the person back.
fn callback_url(sign_in: &SignIn) -> Result<String, SignInFailure> {
    let public_url = sign_in
        .public_url
        .as_deref()
        .ok_or(SignInFailure::NotConfigured)?;

    Ok(format!("{public_url}{SIGN_IN_CALLBACK_PATH}"))
}

#[derive(Deserialize)]
pub(crate) struct Callback {
    code: Option<String>,
    state: Option<String>,
}

/// Creates the session of the person that the SCM sends back, then opens the
/// page that the sign-in asked for.
pub(crate) async fn callback(
    State(sign_in): State<SignIn>,
    headers: HeaderMap,
    Query(callback): Query<Callback>,
) -> Response {
    let cleared = set_cookie(SIGN_IN_COOKIE, "", 0);
    let (secret, next) = match session_of_callback(&sign_in, &headers, callback).await {
        Ok(signed_in) => signed_in,
        Err(err) => return (AppendHeaders([(SET_COOKIE, cleared)]), failed(&err)).into_response(),
    };

    let max_age = SESSION_LIFETIME.whole_seconds();
    let cookies = [
        (SET_COOKIE, cleared),
        (
            SET_COOKIE,
            set_cookie(SESSION_COOKIE, secret.expose_secret(), max_age),
        ),
        (
            SET_COOKIE,
            set_cookie(CSRF_COOKIE, &csrf_token(&secret), max_age),
        ),
    ];
    (AppendHeaders(cookies), Redirect::to(&next)).into_response()
}

async fn session_of_callback(
    sign_in: &SignIn,
    headers: &HeaderMap,
    callback: Callback,
) -> Result<(SecretString, String), SignInFailure> {
    let kept = cookie(headers, SIGN_IN_COOKIE).ok_or(SignInFailure::NotStarted)?;
    let mut parts = kept.expose_secret().splitn(4, '.');
    let (Some(state), Some(verifier), Some(setup), Some(next)) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(SignInFailure::NotStarted);
    };
    let (Some(code), Some(sent_state)) = (callback.code, callback.state) else {
        return Err(SignInFailure::NoCode);
    };

    if !is_same_text(&sent_state, state) {
        return Err(SignInFailure::OtherState);
    }

    let code = SignInCode {
        code: code.into(),
        code_verifier: verifier.to_owned().into(),
        redirect_url: callback_url(sign_in)?,
    };
    let cipher = sign_in
        .cipher
        .as_deref()
        .ok_or(SignInFailure::NotConfigured)?;
    let (signed_in, token) = sign_in
        .scm
        .sign_in(&code, JobClass::Interactive)
        .await
        .map_err(SignInFailure::Scm)?;
    let now = (sign_in.now)();
    let user_id = record_sign_in(
        &sign_in.pool,
        sign_in.kind,
        cipher,
        (&signed_in, &token),
        now,
    )
    .await
    .map_err(SignInFailure::Platform)?;
    let next = match setup.split_once('-') {
        Some((installation, action)) => {
            page_of_installation(sign_in, user_id, installation, action).await?
        }
        None => next.to_owned(),
    };

    #[expect(
        clippy::disallowed_methods,
        reason = "a session of the dashboard has a new identifier"
    )]
    let id = SessionId::new();
    let secret = new_session_secret().map_err(SignInFailure::Store)?;
    let session = NewSession {
        id,
        user_id,
        created_at: now,
        expires_at: now + SESSION_LIFETIME,
    };
    let mut conn = sign_in.pool.acquire().await.map_err(SignInFailure::Query)?;

    create_session(&mut conn, &session, &secret)
        .await
        .map_err(SignInFailure::Store)?;
    info!(
        user_id = %user_id,
        organizations = signed_in.memberships.len(),
        "signed in"
    );
    Ok((secret, next))
}

/// The repositories of the organization of `installation` when the person
/// has a role in it, so that they follow its first reads; `/welcome`
/// otherwise.
async fn page_of_installation(
    sign_in: &SignIn,
    user_id: UserId,
    installation: &str,
    action: &str,
) -> Result<String, SignInFailure> {
    let mut tx = begin_system(&sign_in.pool)
        .await
        .map_err(SignInFailure::Store)?;
    let slug: Option<String> = sqlx::query_scalar(
        "SELECT o.slug FROM scm_connections c
         JOIN orgs o ON o.id = c.org_id
         JOIN memberships m ON m.org_id = c.org_id AND m.user_id = $3
         WHERE c.kind = $1::connection_kind AND c.external_id = $2",
    )
    .bind(sign_in.kind.as_str())
    .bind(installation)
    .bind(user_id.to_string())
    .fetch_optional(&mut *tx)
    .await
    .map_err(SignInFailure::Query)?;
    tx.commit().await.map_err(SignInFailure::Query)?;

    Ok(slug.map_or_else(
        || WELCOME.to_owned(),
        |slug| format!("/orgs/{slug}?connecting={action}"),
    ))
}

fn failed(err: &SignInFailure) -> Response {
    warn!(error = err as &dyn std::error::Error, "sign-in failed");
    Redirect::to(FAILED).into_response()
}

/// The page of the dashboard to open after the sign-in: a path of this site
/// only, so that a link cannot send the person elsewhere, and with the
/// characters that a cookie carries.
fn page_after(next: Option<&str>) -> &str {
    let is_safe = |path: &str| {
        path.starts_with('/')
            && !path.starts_with("//")
            && path
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"/-_.~?=&%".contains(&byte))
    };
    next.filter(|path| is_safe(path)).unwrap_or("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_setup_url_keeps_a_number_and_an_action_only() {
        let setup = |installation_id: Option<&str>, setup_action: Option<&str>| {
            setup_of(&Installed {
                installation_id: installation_id.map(str::to_owned),
                setup_action: setup_action.map(str::to_owned),
            })
        };

        assert_eq!(
            setup(Some("42"), Some("update")).as_deref(),
            Some("42-update")
        );
        assert_eq!(setup(Some("42"), None).as_deref(), Some("42-install"));
        assert_eq!(setup(Some("4.2"), Some("install")), None);
        assert_eq!(setup(Some("42"), Some("request")), None);
        assert_eq!(setup(Some(""), Some("install")), None);
        assert_eq!(setup(None, Some("install")), None);
    }

    #[test]
    fn the_page_after_the_sign_in_stays_on_this_site() {
        assert_eq!(page_after(Some("/orgs/acme")), "/orgs/acme");
        assert_eq!(
            page_after(Some("/q/web/main/121?tab=merged")),
            "/q/web/main/121?tab=merged"
        );
        for unsafe_next in [
            "https://evil.example",
            "//evil.example",
            "/a;b",
            "/a b",
            "a",
        ] {
            assert_eq!(page_after(Some(unsafe_next)), "/", "{unsafe_next}");
        }
        assert_eq!(page_after(None), "/");
    }
}

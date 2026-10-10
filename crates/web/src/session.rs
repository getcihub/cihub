//! The session of the dashboard on the server: its cookies, its CSRF token and
//! the sign-out (`specs/SECURITY.md`). The store keeps the session; this
//! module only reads and clears what the browser sends.

use axum::extract::State;
use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use cihub_api_types::ErrorCode;
use cihub_platform::ApiError;
use cihub_store::{PgPool, csrf_token, delete_session};
use secrecy::SecretString;

/// The cookie that holds the secret of the session; no script reads it.
pub(crate) const SESSION_COOKIE: &str = "__Host-cihub_session";
/// The cookie that gives the CSRF token to the scripts of the dashboard,
/// which send it back in [`CSRF_HEADER`].
pub(crate) const CSRF_COOKIE: &str = "__Host-cihub_csrf";
pub(crate) const CSRF_HEADER: &str = "x-csrf-token";
/// The attributes of both cookies. The prefix `__Host-` makes the browser
/// require `Secure` and `Path=/` and refuse `Domain`, so that a site on
/// another subdomain cannot plant a session.
const COOKIE_ATTRIBUTES: &str = "Path=/; Secure; SameSite=Lax";
pub const SIGN_OUT_PATH: &str = "/auth/sign-out";

/// The value of the cookie `name`, as a secret; `None` when the request
/// carries it twice, as nothing tells which one CIHub set.
pub(crate) fn cookie(headers: &HeaderMap, name: &str) -> Option<SecretString> {
    let mut values = headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .filter(|(key, _)| *key == name)
        .map(|(_, value)| value);
    let value = values.next()?;

    values
        .next()
        .is_none()
        .then(|| SecretString::from(value.to_owned()))
}

/// The `Set-Cookie` value of the cookie `name`; only the session cookie is
/// hidden from the scripts.
pub(crate) fn set_cookie(name: &str, value: &str, max_age_seconds: i64) -> String {
    let hidden = if name == SESSION_COOKIE {
        "; HttpOnly"
    } else {
        ""
    };
    format!("{name}={value}; {COOKIE_ATTRIBUTES}; Max-Age={max_age_seconds}{hidden}")
}

/// A request that can change a state needs the CSRF token: a page of
/// another site can make the browser send the cookie, not the header.
pub(crate) fn needs_csrf_token(method: &Method) -> bool {
    !matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}

pub(crate) fn has_csrf_token(headers: &HeaderMap, secret: &SecretString) -> bool {
    let Some(sent) = headers
        .get(CSRF_HEADER)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };

    is_same_text(sent, &csrf_token(secret))
}

/// Compares every byte, so that the time of the answer does not tell how
/// much of a guess was right.
fn is_same_text(left: &str, right: &str) -> bool {
    left.len() == right.len()
        && left
            .bytes()
            .zip(right.bytes())
            .fold(0, |differences, (a, b)| differences | (a ^ b))
            == 0
}

pub(crate) fn csrf_refused() -> ApiError {
    ApiError::new(
        ErrorCode::Forbidden,
        format!("the header `{CSRF_HEADER}` does not carry the CSRF token of the session"),
    )
}

/// Deletes the session of the request and clears its cookies. Without a
/// session, it only clears the cookies.
pub(crate) async fn sign_out(
    State(pool): State<PgPool>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    if let Some(secret) = cookie(&headers, SESSION_COOKIE) {
        if !has_csrf_token(&headers, &secret) {
            return Err(csrf_refused());
        }
        delete_session(&pool, &secret).await?;
    }

    let mut response = StatusCode::NO_CONTENT.into_response();
    for name in [SESSION_COOKIE, CSRF_COOKIE] {
        let cleared = HeaderValue::from_str(&set_cookie(name, "", 0))?;
        response.headers_mut().append(SET_COOKIE, cleared);
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cookie_is_found_among_the_others() {
        let mut headers = HeaderMap::new();
        headers.append(
            COOKIE,
            HeaderValue::from_static("theme=dark; __Host-cihub_session=chs_a"),
        );
        headers.append(COOKIE, HeaderValue::from_static("__Host-cihub_csrf=b"));

        let session = cookie(&headers, SESSION_COOKIE);
        let csrf = cookie(&headers, CSRF_COOKIE);

        assert!(
            session.is_some_and(|secret| secrecy::ExposeSecret::expose_secret(&secret) == "chs_a")
        );
        assert!(csrf.is_some());
        assert!(cookie(&headers, "other").is_none());
    }

    #[test]
    fn a_cookie_sent_twice_is_refused() {
        let mut headers = HeaderMap::new();
        headers.append(
            COOKIE,
            HeaderValue::from_static("__Host-cihub_session=a; __Host-cihub_session=b"),
        );

        assert!(cookie(&headers, SESSION_COOKIE).is_none());
    }

    #[test]
    fn only_the_session_cookie_is_hidden_from_the_scripts() {
        assert_eq!(
            set_cookie(SESSION_COOKIE, "chs_a", 60),
            "__Host-cihub_session=chs_a; Path=/; Secure; SameSite=Lax; Max-Age=60; HttpOnly"
        );
        assert_eq!(
            set_cookie(CSRF_COOKIE, "", 0),
            "__Host-cihub_csrf=; Path=/; Secure; SameSite=Lax; Max-Age=0"
        );
    }

    #[test]
    fn only_the_token_of_the_session_passes() {
        let secret = SecretString::from("chs_secret");
        let mut headers = HeaderMap::new();

        assert!(!has_csrf_token(&headers, &secret));

        headers.insert(CSRF_HEADER, HeaderValue::from_static("guess"));
        assert!(!has_csrf_token(&headers, &secret));

        let token = HeaderValue::from_str(&csrf_token(&secret)).unwrap();
        headers.insert(CSRF_HEADER, token);
        assert!(has_csrf_token(&headers, &secret));
        assert!(!needs_csrf_token(&Method::GET));
        assert!(needs_csrf_token(&Method::PATCH));
    }
}

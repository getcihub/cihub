//! The HTTP client of the App: the tokens of its installations, the budget of
//! each connection, and the errors of GitHub in the terms of CIHub.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError};

use cihub_domain::{ConnectionId, JobClass, Permission};
use cihub_scm::{ConnectionRef, ScmError};
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap};
use reqwest::{Client, Method, RequestBuilder, StatusCode};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};
use tokio::sync::Notify;

use crate::auth::AppKey;
use crate::budget::{Budget, Observation, Rate, SECONDARY_WAIT};

/// The REST API version of every call: GitHub Enterprise Server accepts it
/// too.
const API_VERSION: &str = "2022-11-28";
/// A token of an installation is renewed this long before its end.
const TOKEN_MARGIN: Duration = Duration::minutes(5);
/// A permission of a user on a repository is kept this long
/// (`DECISIONS.md` D157).
const PERMISSION_TTL: Duration = Duration::minutes(5);
/// Past this many kept permissions, the expired ones go.
const PERMISSIONS_KEPT: usize = 10_000;
/// A call gives up after this long, below the timeout of a job
/// (`DECISIONS.md` D69).
const CALL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
pub(crate) const JSON: &str = "application/vnd.github+json";

/// The time, read by the caller (`DECISIONS.md` D16).
pub type Clock = Arc<dyn Fn() -> OffsetDateTime + Send + Sync>;

/// How to reach a GitHub App (`DECISIONS.md` D79).
#[derive(Debug)]
pub struct GithubConfig {
    /// `https://github.com`, or the address of a GitHub Enterprise Server.
    pub url: String,
    pub app_id: String,
    pub private_key: SecretString,
}

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("the private key of the GitHub App is not valid")]
    Key(#[from] crate::auth::KeyError),
    #[error("the HTTP client cannot start")]
    Http(#[source] reqwest::Error),
}

/// An answer of GitHub that is not an error of the transport, the budget or
/// the access: the caller reads its status.
#[derive(Debug)]
pub(crate) struct Answer {
    pub status: StatusCode,
    pub body: Vec<u8>,
}

impl Answer {
    pub(crate) fn json<T: serde::de::DeserializeOwned>(&self) -> Result<T, ScmError> {
        serde_json::from_slice(&self.body).map_err(|err| ScmError::Protocol(err.to_string()))
    }

    /// The `message` of an error of GitHub, for the operator.
    pub(crate) fn message(&self) -> String {
        #[derive(Deserialize)]
        struct Message {
            message: String,
        }
        serde_json::from_slice::<Message>(&self.body).map_or_else(
            |_| format!("HTTP {}", self.status),
            |m| format!("HTTP {}: {}", self.status, m.message),
        )
    }
}

/// A request of the App to GitHub.
pub(crate) struct Request<'a> {
    pub connection: &'a ConnectionRef,
    pub class: JobClass,
    pub method: Method,
    /// After the address of the API, such as `/repos/o/r/pulls/1`.
    pub path: String,
    pub body: Option<Value>,
    pub accept: &'static str,
}

/// The GitHub App, shared by every executor of a process.
pub struct GithubApp {
    api_url: String,
    graphql_url: String,
    app_id: String,
    key: AppKey,
    http: Client,
    clock: Clock,
    tokens: Mutex<HashMap<String, (SecretString, OffsetDateTime)>>,
    permissions: Mutex<HashMap<PermissionKey, (Permission, OffsetDateTime)>>,
    budgets: Mutex<Budgets>,
    urgent: Arc<Notify>,
}

/// The budgets of the connections that this process called, and those that
/// changed since the last write.
#[derive(Default)]
struct Budgets {
    all: HashMap<ConnectionId, Budget>,
    changed: HashSet<ConnectionId>,
}

impl Budgets {
    /// Records what a call on `connection` observed, and returns `true` when
    /// the pause or the mode of the connection changed.
    fn observe(
        &mut self,
        connection: ConnectionId,
        observation: Observation,
        now: OffsetDateTime,
    ) -> bool {
        let budget = self.all.entry(connection).or_default();
        let before = (budget.mode, budget.paused_until);
        budget.observe(observation, now);
        self.changed.insert(connection);
        before != (budget.mode, budget.paused_until)
    }

    fn take_changed(&mut self) -> Vec<(ConnectionId, Budget)> {
        let Self { all, changed } = self;
        changed
            .drain()
            .filter_map(|id| all.get(&id).map(|budget| (id, budget.clone())))
            .collect()
    }
}

/// A user on a repository of a connection: a permission is not shared
/// between connections.
pub(crate) type PermissionKey = (ConnectionId, String, String);

impl GithubApp {
    /// The identifier of the App on GitHub.
    pub(crate) fn app_id(&self) -> &str {
        &self.app_id
    }

    /// The permission kept for `key`, when it has not expired.
    pub(crate) fn kept_permission(&self, key: &PermissionKey) -> Option<Permission> {
        let now = (self.clock)();
        let kept = self
            .permissions
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        kept.get(key)
            .filter(|(_, until)| *until > now)
            .map(|(permission, _)| *permission)
    }

    /// Keeps `permission` for `key` during [`PERMISSION_TTL`].
    pub(crate) fn keep_permission(&self, key: PermissionKey, permission: Permission) {
        let now = (self.clock)();
        let mut kept = self
            .permissions
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if kept.len() >= PERMISSIONS_KEPT {
            kept.retain(|_, (_, until)| *until > now);
        }
        kept.insert(key, (permission, now + PERMISSION_TTL));
    }

    /// # Errors
    ///
    /// The key is not valid, or the HTTP client cannot start.
    pub fn new(config: &GithubConfig, clock: Clock) -> Result<Self, ClientError> {
        // reqwest uses rustls without a provider (`DECISIONS.md` D80): ring is
        // the provider of the process. A second install changes nothing.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let url = config.url.trim_end_matches('/');
        let (api_url, graphql_url) = if url == "https://github.com" {
            (
                "https://api.github.com".to_owned(),
                "https://api.github.com/graphql".to_owned(),
            )
        } else {
            (format!("{url}/api/v3"), format!("{url}/api/graphql"))
        };
        let http = Client::builder()
            .user_agent("cihub")
            .timeout(CALL_TIMEOUT)
            .build()
            .map_err(ClientError::Http)?;
        Ok(Self {
            api_url,
            graphql_url,
            app_id: config.app_id.clone(),
            key: AppKey::from_pem(&config.private_key)?,
            http,
            clock,
            tokens: Mutex::new(HashMap::new()),
            permissions: Mutex::new(HashMap::new()),
            budgets: Mutex::new(Budgets::default()),
            urgent: Arc::new(Notify::new()),
        })
    }

    /// The budgets that changed since the last call, for the loop that
    /// writes them (`DECISIONS.md` D77).
    pub fn take_changed_budgets(&self) -> Vec<(ConnectionId, Budget)> {
        let mut budgets = self.budgets.lock().unwrap_or_else(PoisonError::into_inner);
        budgets.take_changed()
    }

    /// Notified when a budget pauses its connection or changes its mode: the
    /// loop that writes the budgets writes it at once (`DECISIONS.md` D77).
    pub fn budget_urgent(&self) -> Arc<Notify> {
        Arc::clone(&self.urgent)
    }

    fn admit(&self, connection: ConnectionId, class: JobClass) -> Result<(), ScmError> {
        let mut budgets = self.budgets.lock().unwrap_or_else(PoisonError::into_inner);
        let budget = budgets.all.entry(connection).or_default();
        budget.admit(class, (self.clock)())
    }

    fn observe(&self, connection: ConnectionId, observation: Observation, now: OffsetDateTime) {
        let mut budgets = self.budgets.lock().unwrap_or_else(PoisonError::into_inner);
        if budgets.observe(connection, observation, now) {
            self.urgent.notify_one();
        }
    }

    /// Sends `request` with the token of its installation.
    ///
    /// # Errors
    ///
    /// The budget refuses the call, GitHub cannot be reached or fails, the
    /// rate limit is spent, or the installation lost its access.
    pub(crate) async fn send(&self, request: Request<'_>) -> Result<Answer, ScmError> {
        let url = format!("{}{}", self.api_url, request.path);
        self.send_to(request, url).await
    }

    /// Sends a GraphQL query of `query` with `variables`.
    pub(crate) async fn graphql(
        &self,
        connection: &ConnectionRef,
        class: JobClass,
        query: &str,
        variables: Value,
    ) -> Result<Answer, ScmError> {
        let request = Request {
            connection,
            class,
            method: Method::POST,
            path: String::new(),
            body: Some(serde_json::json!({ "query": query, "variables": variables })),
            accept: JSON,
        };
        self.send_to(request, self.graphql_url.clone()).await
    }

    /// Sends `method path` as the App, with its JWT: a call on the whole
    /// instance, outside the budget of a connection. Returns the answer and
    /// the path of its next page, when the answer has one.
    pub(crate) async fn send_as_app(
        &self,
        method: Method,
        path: &str,
    ) -> Result<(Answer, Option<String>), ScmError> {
        let now = (self.clock)();
        let (status, headers, body) = fetch(self.as_app(method, path, now)?).await?;
        let answer = Answer { status, body };
        if let (_, Some(error)) = classify(&answer, &headers, now) {
            return Err(error);
        }
        let next = headers
            .get("link")
            .and_then(|link| link.to_str().ok())
            .and_then(|link| next_page(link, path));
        Ok((answer, next))
    }

    /// `method path` with the JWT of the App, signed at `now`.
    fn as_app(
        &self,
        method: Method,
        path: &str,
        now: OffsetDateTime,
    ) -> Result<RequestBuilder, ScmError> {
        let jwt = self
            .key
            .jwt(&self.app_id, now)
            .map_err(|err| ScmError::Protocol(err.to_string()))?;
        Ok(self
            .http
            .request(method, format!("{}{path}", self.api_url))
            .header(AUTHORIZATION, format!("Bearer {jwt}"))
            .header(ACCEPT, JSON)
            .header("X-GitHub-Api-Version", API_VERSION))
    }

    async fn send_to(&self, request: Request<'_>, url: String) -> Result<Answer, ScmError> {
        let connection = request.connection.id;
        self.admit(connection, request.class)?;
        let token = self.token(request.connection).await?;
        let mut builder = self
            .http
            .request(request.method, url)
            .header(AUTHORIZATION, format!("Bearer {}", token.expose_secret()))
            .header(ACCEPT, request.accept)
            .header("X-GitHub-Api-Version", API_VERSION);
        if let Some(body) = &request.body {
            builder = builder
                .header(CONTENT_TYPE, "application/json")
                .body(body.to_string());
        }
        let answer = fetch(builder).await;
        let now = (self.clock)();
        let (status, headers, body) = match answer {
            Ok(answer) => answer,
            Err(err) => {
                self.observe(connection, Observation::Failed, now);
                return Err(err);
            }
        };
        let answer = Answer { status, body };
        let (observation, error) = classify(&answer, &headers, now);
        self.observe(connection, observation, now);
        if status == StatusCode::UNAUTHORIZED {
            // The token is refused: the next call asks for a new one, which
            // tells whether the installation still exists.
            self.forget_token(&request.connection.external_id);
        }
        match error {
            Some(error) => Err(error),
            None => Ok(answer),
        }
    }

    /// Pauses `connection` after a GraphQL answer that says its rate limit
    /// is spent, until the reset of that limit.
    pub(crate) fn graphql_limit_reached(&self, connection: ConnectionId) -> ScmError {
        let now = (self.clock)();
        let reset_at = {
            let budgets = self.budgets.lock().unwrap_or_else(PoisonError::into_inner);
            budgets
                .all
                .get(&connection)
                .and_then(|budget| budget.rates.get("graphql"))
                .map(|rate| rate.reset_at)
        };
        let until = reset_at.map_or(now + SECONDARY_WAIT, |reset_at| reset_at.max(now));
        self.observe(
            connection,
            Observation::RateLimited {
                until,
                secondary: false,
            },
            now,
        );
        ScmError::RateLimited { until }
    }

    fn forget_token(&self, installation: &str) {
        let mut tokens = self.tokens.lock().unwrap_or_else(PoisonError::into_inner);
        tokens.remove(installation);
    }

    /// The token of the installation of `connection`, from the cache while
    /// it lives more than 5 minutes.
    async fn token(&self, connection: &ConnectionRef) -> Result<SecretString, ScmError> {
        #[derive(Deserialize)]
        struct Token {
            token: String,
            expires_at: String,
        }
        let installation = &connection.external_id;
        let now = (self.clock)();
        {
            let tokens = self.tokens.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some((token, expires_at)) = tokens.get(installation)
                && *expires_at - TOKEN_MARGIN > now
            {
                return Ok(token.clone());
            }
        }
        let path = format!("/app/installations/{installation}/access_tokens");
        let (status, _, body) = fetch(self.as_app(Method::POST, &path, now)?).await?;
        let answer = Answer { status, body };
        match status {
            StatusCode::CREATED => {}
            StatusCode::NOT_FOUND | StatusCode::FORBIDDEN | StatusCode::UNAUTHORIZED => {
                return Err(ScmError::AccessLost(answer.message()));
            }
            status if status.is_server_error() => {
                return Err(ScmError::Unavailable(answer.message()));
            }
            _ => return Err(ScmError::Protocol(answer.message())),
        }
        let token: Token = answer.json()?;
        let expires_at = OffsetDateTime::parse(&token.expires_at, &Rfc3339)
            .map_err(|err| ScmError::Protocol(err.to_string()))?;
        let token = SecretString::from(token.token);
        let mut tokens = self.tokens.lock().unwrap_or_else(PoisonError::into_inner);
        tokens.insert(installation.clone(), (token.clone(), expires_at));
        Ok(token)
    }
}

impl std::fmt::Debug for GithubApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GithubApp")
            .field("api_url", &self.api_url)
            .field("app_id", &self.app_id)
            .finish_non_exhaustive()
    }
}

/// Sends `request` and reads its whole answer. A failure of the transport is
/// `Unavailable`.
async fn fetch(request: RequestBuilder) -> Result<(StatusCode, HeaderMap, Vec<u8>), ScmError> {
    let unavailable = |err: reqwest::Error| ScmError::Unavailable(err.to_string());
    let response = request.send().await.map_err(unavailable)?;
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.bytes().await.map_err(unavailable)?;
    Ok((status, headers, body.to_vec()))
}

/// The path of the `next` page in a `Link` header, from the path of the call
/// on: the header gives a full address.
fn next_page(link: &str, path: &str) -> Option<String> {
    let base = path.split('?').next()?;
    link.split(',')
        .find(|part| part.contains(r#"rel="next""#))
        .and_then(|part| part.split_once('<')?.1.split_once('>'))
        .and_then(|(url, _)| url.find(base).map(|start| url[start..].to_owned()))
}

/// What an answer tells the budget, and the error it is for CIHub when it is
/// a failure of the server or of the rate limit.
fn classify(
    answer: &Answer,
    headers: &HeaderMap,
    now: OffsetDateTime,
) -> (Observation, Option<ScmError>) {
    let header = |name: &str| {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse::<i64>().ok())
    };
    let reset_at = header("x-ratelimit-reset")
        .and_then(|reset| OffsetDateTime::from_unix_timestamp(reset).ok());
    let status = answer.status;
    if status.is_server_error() {
        return (
            Observation::Failed,
            Some(ScmError::Unavailable(answer.message())),
        );
    }
    if status == StatusCode::FORBIDDEN || status == StatusCode::TOO_MANY_REQUESTS {
        let limited = if header("x-ratelimit-remaining") == Some(0) {
            Some((reset_at.unwrap_or(now + SECONDARY_WAIT), false))
        } else if let Some(seconds) = header("retry-after") {
            Some((now + Duration::seconds(seconds), true))
        } else if answer.message().contains("secondary rate limit") {
            Some((now + SECONDARY_WAIT, true))
        } else {
            None
        };
        if let Some((until, secondary)) = limited {
            return (
                Observation::RateLimited { until, secondary },
                Some(ScmError::RateLimited { until }),
            );
        }
    }
    let rate = match (
        header("x-ratelimit-limit"),
        header("x-ratelimit-remaining"),
        reset_at,
    ) {
        (Some(limit), Some(remaining), Some(reset_at)) => Some(Rate {
            limit: u32::try_from(limit).unwrap_or(u32::MAX),
            remaining: u32::try_from(remaining).unwrap_or(0),
            reset_at,
        }),
        _ => None,
    };
    let resource = headers
        .get("x-ratelimit-resource")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("core")
        .to_owned();
    (Observation::Answered { resource, rate }, None)
}

#[cfg(test)]
mod tests {
    use reqwest::header::HeaderValue;

    use super::*;

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_789_000_000 + seconds).unwrap()
    }

    fn answer(status: u16, message: &str) -> Answer {
        Answer {
            status: StatusCode::from_u16(status).unwrap(),
            body: serde_json::json!({ "message": message })
                .to_string()
                .into_bytes(),
        }
    }

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.insert(*name, HeaderValue::from_str(value).unwrap());
        }
        map
    }

    #[test]
    fn a_spent_primary_limit_waits_for_its_reset() {
        let reset = at(900).unix_timestamp().to_string();
        let spent = headers(&[
            ("x-ratelimit-limit", "5000"),
            ("x-ratelimit-remaining", "0"),
            ("x-ratelimit-reset", &reset),
        ]);
        for status in [403, 429] {
            let (observation, error) = classify(&answer(status, "API rate limit"), &spent, at(0));
            assert_eq!(
                observation,
                Observation::RateLimited {
                    until: at(900),
                    secondary: false
                }
            );
            assert_eq!(error, Some(ScmError::RateLimited { until: at(900) }));
        }
    }

    #[test]
    fn a_secondary_limit_waits_for_retry_after_or_a_minute() {
        let retry = headers(&[("x-ratelimit-remaining", "12"), ("retry-after", "30")]);
        let (_, error) = classify(&answer(403, "slow down"), &retry, at(0));
        assert_eq!(error, Some(ScmError::RateLimited { until: at(30) }));
        let message = answer(403, "You have exceeded a secondary rate limit");
        let (observation, _) = classify(&message, &headers(&[]), at(0));
        assert_eq!(
            observation,
            Observation::RateLimited {
                until: at(60),
                secondary: true
            }
        );
    }

    #[test]
    fn a_forbidden_answer_without_a_limit_goes_to_the_caller() {
        let (observation, error) = classify(&answer(403, "protected branch"), &headers(&[]), at(0));
        assert_eq!(error, None);
        assert!(matches!(
            observation,
            Observation::Answered { rate: None, .. }
        ));
    }

    #[test]
    fn the_next_page_comes_from_the_link_header() {
        let link = r#"<https://api.github.com/app/hook/deliveries?per_page=100&cursor=v1_9>; rel="next", <https://api.github.com/app/hook/deliveries?per_page=100>; rel="first""#;
        assert_eq!(
            next_page(link, "/app/hook/deliveries?per_page=100"),
            Some("/app/hook/deliveries?per_page=100&cursor=v1_9".into())
        );
        let last = r#"<https://api.github.com/app/hook/deliveries?per_page=100>; rel="first""#;
        assert_eq!(next_page(last, "/app/hook/deliveries?per_page=100"), None);
    }

    #[test]
    fn a_server_error_is_a_failure() {
        let (observation, error) = classify(&answer(502, "bad gateway"), &headers(&[]), at(0));
        assert_eq!(observation, Observation::Failed);
        assert!(matches!(error, Some(ScmError::Unavailable(_))));
    }

    #[test]
    fn a_pause_or_a_change_of_mode_is_urgent_and_every_call_is_a_change() {
        let connection = ConnectionId::from_ulid(ulid::Ulid(1));
        let mut budgets = Budgets::default();
        let answered = Observation::Answered {
            resource: "core".into(),
            rate: None,
        };
        assert!(!budgets.observe(connection, answered.clone(), at(0)));
        assert_eq!(budgets.take_changed().len(), 1);
        assert!(budgets.take_changed().is_empty());
        let limited = Observation::RateLimited {
            until: at(60),
            secondary: false,
        };
        assert!(budgets.observe(connection, limited.clone(), at(1)));
        assert!(!budgets.observe(connection, limited, at(2)));
        let changed = budgets.take_changed();
        assert_eq!(changed[0].1.paused_until, Some(at(60)));
        assert!(!budgets.observe(connection, Observation::Failed, at(3)));
        assert!(!budgets.observe(connection, Observation::Failed, at(4)));
        assert!(budgets.observe(connection, Observation::Failed, at(5)));
    }

    #[test]
    fn an_answer_gives_its_rate_limit() {
        let reset = at(900).unix_timestamp().to_string();
        let rate = headers(&[
            ("x-ratelimit-limit", "5000"),
            ("x-ratelimit-remaining", "4321"),
            ("x-ratelimit-reset", &reset),
            ("x-ratelimit-resource", "graphql"),
        ]);
        let (observation, error) = classify(&answer(200, ""), &rate, at(0));
        assert_eq!(error, None);
        assert_eq!(
            observation,
            Observation::Answered {
                resource: "graphql".into(),
                rate: Some(Rate {
                    limit: 5_000,
                    remaining: 4_321,
                    reset_at: at(900)
                })
            }
        );
    }
}

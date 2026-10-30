//! The HTTP client of the App: the tokens of its installations, the budget of
//! each connection, and the errors of GitHub in the terms of CIHub.

use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use cihub_domain::{ConnectionId, JobClass, Permission};
use cihub_scm::{ConnectionRef, RepoRef, ScmError};
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap};
use reqwest::{Client, Method, RequestBuilder, StatusCode};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};
use tokio::sync::Notify;

use crate::auth::AppKey;
use crate::budget::{Budget, Observation, Rate, SECONDARY_WAIT};

/// The REST API version of every call: GitHub Enterprise Server accepts it
/// too.
const API_VERSION: &str = "2022-11-28";
/// A token of an installation is renewed this long before it expires.
const TOKEN_MARGIN: Duration = Duration::minutes(5);
/// How long a permission that GitHub gave is kept.
const PERMISSION_TTL: Duration = Duration::minutes(5);
/// Above this number of kept permissions, the expired ones are removed.
const PERMISSIONS_KEPT: usize = 10_000;
/// The longest wait for an answer of GitHub.
pub(crate) const CALL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
/// The media type of the REST API of GitHub.
pub(crate) const JSON: &str = "application/vnd.github+json";
/// The media type that gives the content of a file as it is.
pub(crate) const RAW: &str = "application/vnd.github.raw+json";
/// The largest page that GitHub gives.
pub(crate) const PER_PAGE: usize = 100;

/// The address of github.com, whose API has its own host.
const GITHUB_COM: &str = "https://github.com";

/// The address of the REST API of `url`, github.com or a GitHub Enterprise
/// Server, without a final `/`.
pub(crate) fn api_url(url: &str) -> String {
    if url == GITHUB_COM {
        "https://api.github.com".to_owned()
    } else {
        format!("{url}/api/v3")
    }
}

/// Encodes a part of a path or of a query, keeping `/`, which the branch
/// names of GitHub contain.
pub(crate) fn encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                encoded.push(char::from(byte));
            }
            _ => {
                let _ = write!(encoded, "%{byte:02X}");
            }
        }
    }
    encoded
}

/// The path of `rest` in `repo`, such as `/repos/o/r/pulls/1`.
pub(crate) fn repo_path(repo: &RepoRef, rest: &str) -> String {
    format!(
        "/repos/{}/{}{rest}",
        encode(&repo.owner),
        encode(&repo.name)
    )
}

/// The time, read by the caller.
pub type Clock = Arc<dyn Fn() -> OffsetDateTime + Send + Sync>;

/// How to reach a GitHub App.
#[derive(Debug)]
pub struct GithubConfig {
    /// `https://github.com`, or the address of a GitHub Enterprise Server.
    pub url: String,
    pub app_id: String,
    pub private_key: SecretString,
    /// Without it, nobody signs in to the dashboard; the API tokens still
    /// work.
    pub sign_in: Option<SignInClient>,
}

/// The OAuth client of the App, with which a person signs in.
#[derive(Clone, Debug)]
pub struct SignInClient {
    pub client_id: String,
    pub client_secret: SecretString,
}

/// Why the client of the App cannot start.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("the private key of the GitHub App is not valid")]
    Key(#[from] crate::auth::KeyError),
    #[error("the HTTP client cannot start")]
    Http(#[source] reqwest::Error),
}

/// Who makes a call, which decides its token and whether a budget counts it.
#[derive(Clone, Copy)]
pub(crate) enum Auth<'a> {
    /// The token of the installation of a connection: the budget of the
    /// connection admits and counts the call.
    Installation(&'a ConnectionRef, JobClass),
    /// The JWT of the App, outside the budget of any connection.
    App,
    /// The token of a person, outside the budget of any connection.
    Person(&'a SecretString),
}

/// An answer of GitHub, read in full.
#[derive(Debug)]
pub(crate) struct Answer {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
    /// The path of the next page, from the `Link` header.
    pub next: Option<String>,
}

impl Answer {
    /// The body, read as JSON.
    pub(crate) fn json<T: DeserializeOwned>(&self) -> Result<T, ScmError> {
        serde_json::from_slice(&self.body).map_err(|err| ScmError::Protocol(err.to_string()))
    }

    /// The status and the `message` of the body, to describe an error.
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

    /// The answer when its status is one of `statuses`, its error otherwise.
    pub(crate) fn expect_status(self, statuses: &[StatusCode]) -> Result<Self, ScmError> {
        if statuses.contains(&self.status) {
            Ok(self)
        } else {
            Err(self.error())
        }
    }

    /// The error of an answer that the caller does not expect.
    pub(crate) fn error(&self) -> ScmError {
        let message = self.message();
        match self.status {
            StatusCode::NOT_FOUND => ScmError::NotFound(message),
            // GitHub says this when the App lacks the permission of the call,
            // or when the installation does not cover the repository.
            StatusCode::FORBIDDEN if message.contains("not accessible by integration") => {
                ScmError::Forbidden(format!(
                    "the GitHub App lacks a permission for this call, or its installation does \
                     not cover the repository: {message}"
                ))
            }
            StatusCode::FORBIDDEN => ScmError::Forbidden(message),
            StatusCode::UNPROCESSABLE_ENTITY | StatusCode::CONFLICT => {
                ScmError::Validation(message)
            }
            _ => ScmError::Protocol(message),
        }
    }
}

/// The GitHub App, shared by every executor of a process.
pub struct GithubApp {
    api_url: String,
    graphql_url: String,
    /// The address of the pages of GitHub, where a person signs in.
    pub(crate) web_url: String,
    pub(crate) sign_in: Option<SignInClient>,
    /// The page that installs the App, read once: its slug does not change
    /// while the server runs.
    pub(crate) install_url: OnceLock<String>,
    app_id: String,
    key: AppKey,
    pub(crate) http: Client,
    clock: Clock,
    tokens: Mutex<HashMap<String, (SecretString, OffsetDateTime)>>,
    permissions: Mutex<HashMap<PermissionKey, (Permission, OffsetDateTime)>>,
    budgets: Mutex<Budgets>,
    urgent: Arc<Notify>,
}

/// The budgets of the connections, and those that changed since their last
/// write.
#[derive(Default)]
struct Budgets {
    all: HashMap<ConnectionId, Budget>,
    changed: HashSet<ConnectionId>,
}

impl Budgets {
    /// Updates the budget of `connection`; `true` when its pause or its mode
    /// changes.
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

    /// The budgets that changed since the last call.
    fn take_changed(&mut self) -> Vec<(ConnectionId, Budget)> {
        let Self { all, changed } = self;
        changed
            .drain()
            .filter_map(|id| all.get(&id).map(|budget| (id, budget.clone())))
            .collect()
    }
}

/// A connection, a repository `<owner>/<name>` and a login.
pub(crate) type PermissionKey = (ConnectionId, String, String);

impl GithubApp {
    /// The id of the App.
    pub(crate) fn app_id(&self) -> &str {
        &self.app_id
    }

    /// The permission kept for `key`, if it has not expired.
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

    /// The App that `config` describes, which reads the time with `clock`.
    ///
    /// # Errors
    ///
    /// The key is not valid, or the HTTP client cannot start.
    pub fn new(config: &GithubConfig, clock: Clock) -> Result<Self, ClientError> {
        // reqwest uses rustls without a provider: ring is the provider of the
        // process. A second install changes nothing.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let url = config.url.trim_end_matches('/');
        let api_url = api_url(url);
        let graphql_url = if url == GITHUB_COM {
            format!("{api_url}/graphql")
        } else {
            format!("{url}/api/graphql")
        };
        let http = Client::builder()
            .user_agent("cihub")
            .timeout(CALL_TIMEOUT)
            .build()
            .map_err(ClientError::Http)?;
        Ok(Self {
            api_url,
            graphql_url,
            web_url: url.to_owned(),
            sign_in: config.sign_in.clone(),
            install_url: OnceLock::new(),
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
    /// writes them.
    pub fn take_changed_budgets(&self) -> Vec<(ConnectionId, Budget)> {
        let mut budgets = self.budgets.lock().unwrap_or_else(PoisonError::into_inner);
        budgets.take_changed()
    }

    /// Notified when a budget pauses its connection or changes its mode: the
    /// loop that writes the budgets writes it at once.
    pub fn budget_urgent(&self) -> Arc<Notify> {
        Arc::clone(&self.urgent)
    }

    /// Whether the budget of `connection` lets a call of `class` go now.
    fn admit(&self, connection: ConnectionId, class: JobClass) -> Result<(), ScmError> {
        let mut budgets = self.budgets.lock().unwrap_or_else(PoisonError::into_inner);
        let budget = budgets.all.entry(connection).or_default();
        budget.admit(class, (self.clock)())
    }

    /// Updates the budget of `connection`, and wakes the loop that writes the
    /// budgets when the change is urgent.
    fn observe(&self, connection: ConnectionId, observation: Observation, now: OffsetDateTime) {
        let mut budgets = self.budgets.lock().unwrap_or_else(PoisonError::into_inner);
        if budgets.observe(connection, observation, now) {
            self.urgent.notify_one();
        }
    }

    /// A call to the REST API at `path`, such as `/repos/o/r/pulls/1`.
    pub(crate) async fn send(
        &self,
        auth: Auth<'_>,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<Answer, ScmError> {
        self.send_accepting(auth, method, path, body, JSON).await
    }

    /// [`Self::send`], with the media type `accept` in place of JSON.
    pub(crate) async fn send_accepting(
        &self,
        auth: Auth<'_>,
        method: Method,
        path: &str,
        body: Option<Value>,
        accept: &str,
    ) -> Result<Answer, ScmError> {
        let url = format!("{}{path}", self.api_url);
        let mut answer = self.call(auth, method, url, body, accept).await?;
        answer.next = answer
            .headers
            .get("link")
            .and_then(|link| link.to_str().ok())
            .and_then(|link| next_page(link, path));

        Ok(answer)
    }

    /// A GraphQL query with its variables, with the token of `connection`.
    pub(crate) async fn graphql(
        &self,
        connection: &ConnectionRef,
        class: JobClass,
        query: &str,
        variables: Value,
    ) -> Result<Answer, ScmError> {
        let auth = Auth::Installation(connection, class);
        let body = serde_json::json!({ "query": query, "variables": variables });
        self.call(
            auth,
            Method::POST,
            self.graphql_url.clone(),
            Some(body),
            JSON,
        )
        .await
    }

    /// A call to the REST API at `rest` in `repo`, such as `/pulls/1`.
    pub(crate) async fn rest(
        &self,
        repo: &RepoRef,
        class: JobClass,
        method: Method,
        rest: &str,
        body: Option<Value>,
    ) -> Result<Answer, ScmError> {
        let auth = Auth::Installation(&repo.connection, class);
        self.send(auth, method, &repo_path(repo, rest), body).await
    }

    /// A `GET` of `rest` in `repo` that GitHub answers with `200`.
    pub(crate) async fn get_json<T: DeserializeOwned>(
        &self,
        repo: &RepoRef,
        class: JobClass,
        rest: &str,
    ) -> Result<T, ScmError> {
        let answer = self.rest(repo, class, Method::GET, rest, None).await?;
        answer.expect_status(&[StatusCode::OK])?.json()
    }

    /// Every page of `path` that the `Link` header names, for a route that
    /// gives its pages by a cursor.
    pub(crate) async fn linked_pages<P: DeserializeOwned>(
        &self,
        auth: Auth<'_>,
        path: String,
    ) -> Result<Vec<P>, ScmError> {
        let mut path = Some(path);
        let mut pages = Vec::new();
        while let Some(page) = path.take() {
            let answer = self.send(auth, Method::GET, &page, None).await?;
            let answer = answer.expect_status(&[StatusCode::OK])?;
            pages.push(answer.json()?);
            path = answer.next;
        }

        Ok(pages)
    }

    /// The items of the pages of `path` numbered from 1, up to the first page
    /// that is not full or to `max_pages` pages.
    pub(crate) async fn numbered_pages<T: DeserializeOwned>(
        &self,
        auth: Auth<'_>,
        path: &str,
        max_pages: usize,
    ) -> Result<Vec<T>, ScmError> {
        let separator = if path.contains('?') { '&' } else { '?' };
        let mut items = Vec::new();
        for page in 1..=max_pages {
            let page = format!("{path}{separator}per_page={PER_PAGE}&page={page}");
            let answer = self.send(auth, Method::GET, &page, None).await?;
            let listed: Vec<T> = answer.expect_status(&[StatusCode::OK])?.json()?;
            let is_last = listed.len() < PER_PAGE;

            items.extend(listed);
            if is_last {
                break;
            }
        }

        Ok(items)
    }

    /// A JWT of the App at `now`.
    fn jwt(&self, now: OffsetDateTime) -> Result<SecretString, ScmError> {
        self.key
            .jwt(&self.app_id, now)
            .map(SecretString::from)
            .map_err(|err| ScmError::Protocol(err.to_string()))
    }

    /// A call to the API of GitHub at `url`, authenticated by `bearer`.
    fn api_request(
        &self,
        method: Method,
        url: String,
        bearer: &SecretString,
        accept: &str,
    ) -> RequestBuilder {
        self.http
            .request(method, url)
            .header(AUTHORIZATION, format!("Bearer {}", bearer.expose_secret()))
            .header(ACCEPT, accept)
            .header("X-GitHub-Api-Version", API_VERSION)
    }

    /// Sends a call with the token of `auth`, and reads its answer.
    async fn call(
        &self,
        auth: Auth<'_>,
        method: Method,
        url: String,
        body: Option<Value>,
        accept: &str,
    ) -> Result<Answer, ScmError> {
        let bearer = match auth {
            Auth::Installation(connection, class) => {
                self.admit(connection.id, class)?;
                self.token(connection).await?
            }
            Auth::App => self.jwt((self.clock)())?,
            Auth::Person(token) => token.clone(),
        };
        let mut request = self.api_request(method, url, &bearer, accept);
        if let Some(body) = &body {
            request = request
                .header(CONTENT_TYPE, "application/json")
                .body(body.to_string());
        }

        let fetched = fetch(request).await;
        let now = (self.clock)();
        match auth {
            Auth::Installation(connection, _) => self.counted(connection, fetched, now),
            Auth::App | Auth::Person(_) => outside_budgets(auth, fetched?, now),
        }
    }

    /// The answer of a call of `connection`, which its budget observes.
    fn counted(
        &self,
        connection: &ConnectionRef,
        fetched: Result<Answer, ScmError>,
        now: OffsetDateTime,
    ) -> Result<Answer, ScmError> {
        let answer = match fetched {
            Ok(answer) => answer,
            Err(err) => {
                self.observe(connection.id, Observation::Failed, now);
                return Err(err);
            }
        };
        let (observation, error) = classify(&answer, now);
        self.observe(connection.id, observation, now);
        if answer.status == StatusCode::UNAUTHORIZED {
            self.forget_token(&connection.external_id);
        }

        match error {
            Some(error) => Err(error),
            None => Ok(answer),
        }
    }

    /// Pauses `connection` until the reset of its GraphQL limit, and gives the
    /// error.
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

    /// Forgets the token of `installation`, which GitHub refused.
    fn forget_token(&self, installation: &str) {
        let mut tokens = self.tokens.lock().unwrap_or_else(PoisonError::into_inner);
        tokens.remove(installation);
    }

    /// The token of the installation of `connection`, from the cache or from
    /// GitHub.
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

        let url = format!(
            "{}/app/installations/{installation}/access_tokens",
            self.api_url
        );
        let request = self.api_request(Method::POST, url, &self.jwt(now)?, JSON);
        let answer = match fetch(request).await {
            Ok(answer) => answer,
            Err(err) => {
                self.observe(connection.id, Observation::Failed, now);
                return Err(err);
            }
        };
        let (observation, error) = classify(&answer, now);
        // The rate limit headers of this call count the requests of the App,
        // not those of the installation: only the answer itself is kept.
        let observation = match observation {
            Observation::Answered { resource, .. } => Observation::Answered {
                resource,
                rate: None,
            },
            other => other,
        };
        self.observe(connection.id, observation, now);
        if let Some(error) = error {
            return Err(error);
        }

        match answer.status {
            StatusCode::CREATED => {}
            StatusCode::NOT_FOUND | StatusCode::FORBIDDEN | StatusCode::UNAUTHORIZED => {
                return Err(ScmError::AccessLost(answer.message()));
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

/// The answer of a call that no budget of a connection counts. GitHub refuses
/// the token of a person with `401`.
fn outside_budgets(
    auth: Auth<'_>,
    answer: Answer,
    now: OffsetDateTime,
) -> Result<Answer, ScmError> {
    if let (_, Some(error)) = classify(&answer, now) {
        return Err(error);
    }

    if matches!(auth, Auth::Person(_)) && answer.status == StatusCode::UNAUTHORIZED {
        return Err(ScmError::AccessLost(format!(
            "the token of the person is refused: {}",
            answer.message()
        )));
    }

    Ok(answer)
}

/// Sends `request` and reads its whole answer.
pub(crate) async fn fetch(request: RequestBuilder) -> Result<Answer, ScmError> {
    let unavailable = |err: reqwest::Error| ScmError::Unavailable(err.to_string());
    let response = request.send().await.map_err(unavailable)?;
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.bytes().await.map_err(unavailable)?;
    Ok(Answer {
        status,
        headers,
        body: body.to_vec(),
        next: None,
    })
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

/// What `answer` tells the budget of its connection, and its error, if any.
fn classify(answer: &Answer, now: OffsetDateTime) -> (Observation, Option<ScmError>) {
    let header = |name: &str| {
        answer
            .headers
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
        // GitHub answers 429 for a rate limit only; a 403 also refuses an
        // action, and goes to the caller without the markers of a limit.
        let limited = if header("x-ratelimit-remaining") == Some(0) {
            Some((reset_at.unwrap_or(now + SECONDARY_WAIT), false))
        } else if let Some(seconds) = header("retry-after") {
            Some((now + Duration::seconds(seconds), true))
        } else if answer.message().contains("secondary rate limit") {
            Some((now + SECONDARY_WAIT, true))
        } else if status == StatusCode::TOO_MANY_REQUESTS {
            Some((reset_at.unwrap_or(now + SECONDARY_WAIT), true))
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
    let resource = answer
        .headers
        .get("x-ratelimit-resource")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("core")
        .to_owned();
    (Observation::Answered { resource, rate }, None)
}

#[cfg(test)]
impl Answer {
    /// An answer with `status` and the error message `message`.
    pub(crate) fn with_message(status: StatusCode, message: &str) -> Self {
        Self {
            status,
            headers: HeaderMap::new(),
            body: serde_json::json!({ "message": message })
                .to_string()
                .into_bytes(),
            next: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use reqwest::header::HeaderValue;

    use super::*;

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_789_000_000 + seconds).unwrap()
    }

    fn answer(status: u16, message: &str, pairs: &[(&'static str, &str)]) -> Answer {
        let mut answer = Answer::with_message(StatusCode::from_u16(status).unwrap(), message);
        for (name, value) in pairs {
            answer
                .headers
                .insert(*name, HeaderValue::from_str(value).unwrap());
        }
        answer
    }

    #[test]
    fn a_spent_primary_limit_waits_for_its_reset() {
        let reset = at(900).unix_timestamp().to_string();
        let spent = [
            ("x-ratelimit-limit", "5000"),
            ("x-ratelimit-remaining", "0"),
            ("x-ratelimit-reset", reset.as_str()),
        ];
        for status in [403, 429] {
            let (observation, error) = classify(&answer(status, "API rate limit", &spent), at(0));
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
        let retry = [("x-ratelimit-remaining", "12"), ("retry-after", "30")];
        let (_, error) = classify(&answer(403, "slow down", &retry), at(0));

        assert_eq!(error, Some(ScmError::RateLimited { until: at(30) }));
        let message = answer(403, "You have exceeded a secondary rate limit", &[]);
        let (observation, _) = classify(&message, at(0));

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
        let (observation, error) = classify(&answer(403, "protected branch", &[]), at(0));
        assert_eq!(error, None);
        assert!(matches!(
            observation,
            Observation::Answered { rate: None, .. }
        ));
    }

    #[test]
    fn a_too_many_requests_answer_without_a_marker_waits_a_minute() {
        let (observation, error) = classify(&answer(429, "slow down", &[]), at(0));

        assert_eq!(error, Some(ScmError::RateLimited { until: at(60) }));
        assert_eq!(
            observation,
            Observation::RateLimited {
                until: at(60),
                secondary: true
            }
        );
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
        let (observation, error) = classify(&answer(502, "bad gateway", &[]), at(0));
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
        assert_eq!(budgets.take_changed(), []);
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
        let rate = [
            ("x-ratelimit-limit", "5000"),
            ("x-ratelimit-remaining", "4321"),
            ("x-ratelimit-reset", reset.as_str()),
            ("x-ratelimit-resource", "graphql"),
        ];
        let (observation, error) = classify(&answer(200, "", &rate), at(0));

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

    #[test]
    fn a_missing_permission_of_the_app_says_so() {
        let forbidden = |message| Answer::with_message(StatusCode::FORBIDDEN, message);
        let missing = forbidden("Resource not accessible by integration").error();
        let ScmError::Forbidden(message) = missing else {
            panic!("{missing:?}");
        };

        assert!(
            message.starts_with("the GitHub App lacks a permission"),
            "{message}"
        );
        assert!(message.ends_with("HTTP 403 Forbidden: Resource not accessible by integration"));
        let rules = forbidden("Protected branch update failed").error();

        assert_eq!(
            rules,
            ScmError::Forbidden("HTTP 403 Forbidden: Protected branch update failed".into())
        );
    }

    #[test]
    fn a_path_keeps_its_slashes_and_encodes_the_rest() {
        assert_eq!(encode("cihub-temp/main/12"), "cihub-temp/main/12");
        assert_eq!(encode("a b#?&é"), "a%20b%23%3F%26%C3%A9");
    }
}

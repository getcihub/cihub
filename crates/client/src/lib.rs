//! The typed client of the public API v1 of CIHub (`specs/API.md`): the
//! methods that the CLI uses, with the types of `cihub-api-types`. It holds no
//! server code and no queue logic.

use std::fmt::Write;

use cihub_api_types::{
    API_PREFIX, Graph, MAX_LIMIT, Pr, PrList, Problem, QueueList, RequestedQueueState, SubmitPr,
    UpdateQueue,
};
use reqwest::{Method, StatusCode};
use secrecy::{ExposeSecret, SecretString};
use serde::Serialize;
use serde::de::DeserializeOwned;

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("the HTTP client cannot start")]
    Start(#[source] reqwest::Error),
    #[error("the request to CIHub failed")]
    Http(#[source] reqwest::Error),
    #[error("{}: {}", .0.code, .0.detail)]
    Api(Problem),
    #[error("CIHub answered {status}: {body}")]
    Unexpected { status: StatusCode, body: String },
    #[error("the request cannot be written")]
    Encode(#[source] serde_json::Error),
}

pub struct Client {
    http: reqwest::Client,
    base: String,
    token: SecretString,
}

impl Client {
    /// A client of the instance at `url` (`https://cihub.example.com`).
    ///
    /// # Errors
    ///
    /// The HTTP client cannot start.
    pub fn new(url: &str, token: SecretString) -> Result<Self, ClientError> {
        // reqwest uses rustls without a provider: ring is
        // the provider of the process. A second install changes nothing.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let http = reqwest::Client::builder()
            .user_agent(concat!("cihub/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(ClientError::Start)?;
        Ok(Self {
            http,
            base: format!("{}{API_PREFIX}", url.trim_end_matches('/')),
            token,
        })
    }

    /// Sends the request, and gives the body of a successful answer.
    async fn send(
        &self,
        method: Method,
        path: &str,
        body: Option<&impl Serialize>,
    ) -> Result<(StatusCode, String), ClientError> {
        let mut request = self
            .http
            .request(method, format!("{}{path}", self.base))
            .bearer_auth(self.token.expose_secret());

        if let Some(body) = body {
            let text = serde_json::to_string(body).map_err(ClientError::Encode)?;
            request = request
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(text);
        }

        let response = request.send().await.map_err(ClientError::Http)?;
        let status = response.status();
        let text = response.text().await.map_err(ClientError::Http)?;

        if status.is_success() {
            return Ok((status, text));
        }

        Err(match serde_json::from_str::<Problem>(&text) {
            Ok(problem) => ClientError::Api(problem),
            Err(_) => ClientError::Unexpected { status, body: text },
        })
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, ClientError> {
        let (status, text) = self.send(Method::GET, path, None::<&()>).await?;

        serde_json::from_str(&text).map_err(|_| ClientError::Unexpected { status, body: text })
    }

    /// Sends a request that the queue takes later: its answer is `202`,
    /// without a body.
    async fn ask(
        &self,
        method: Method,
        path: &str,
        body: Option<&impl Serialize>,
    ) -> Result<(), ClientError> {
        self.send(method, path, body).await.map(|_| ())
    }

    /// The first page of the queues of the repository, by name.
    ///
    /// # Errors
    ///
    /// The request fails, or CIHub refuses it.
    pub async fn queues(&self, repo: RepoPath<'_>) -> Result<QueueList, ClientError> {
        self.get(&format!("{}/queues?limit={MAX_LIMIT}", repo.path()))
            .await
    }

    /// Asks the queue for `state`.
    ///
    /// # Errors
    ///
    /// The request fails, or CIHub refuses it.
    pub async fn set_queue_state(
        &self,
        queue: QueuePath<'_>,
        state: RequestedQueueState,
    ) -> Result<(), ClientError> {
        let body = UpdateQueue { state };
        self.ask(Method::PATCH, &queue.path(), Some(&body)).await
    }

    /// The first page of the PRs of the queue in `states` (`pending`,
    /// `testing`…), in the order of the queue.
    ///
    /// # Errors
    ///
    /// The request fails, or CIHub refuses it.
    pub async fn prs(&self, queue: QueuePath<'_>, states: &[&str]) -> Result<PrList, ClientError> {
        let path = format!(
            "/orgs/{}/prs?repo={}&queue={}&state={}&sort=position&limit={MAX_LIMIT}",
            segment(queue.org),
            segment(queue.repo),
            segment(queue.queue),
            states.join(",")
        );
        self.get(&path).await
    }

    /// The PR `number` of the repository, as its last submission gives it.
    ///
    /// # Errors
    ///
    /// The request fails, or CIHub refuses it.
    pub async fn pr(&self, repo: RepoPath<'_>, number: u64) -> Result<Pr, ClientError> {
        self.get(&format!("{}/prs/{number}", repo.path())).await
    }

    /// Submits the PR `number` of the repository.
    ///
    /// # Errors
    ///
    /// The request fails, or CIHub refuses it.
    pub async fn submit(
        &self,
        repo: RepoPath<'_>,
        number: u64,
        request: &SubmitPr,
    ) -> Result<(), ClientError> {
        let path = format!("{}/prs/{number}/submit", repo.path());
        self.ask(Method::POST, &path, Some(request)).await
    }

    /// Cancels the current submission of the PR `number` of the repository.
    ///
    /// # Errors
    ///
    /// The request fails, or CIHub refuses it.
    pub async fn cancel(&self, repo: RepoPath<'_>, number: u64) -> Result<(), ClientError> {
        let path = format!("{}/prs/{number}/cancel", repo.path());
        self.ask(Method::POST, &path, None::<&()>).await
    }

    /// Restarts the tests of the current submission of the PR `number` of
    /// the repository.
    ///
    /// # Errors
    ///
    /// The request fails, or CIHub refuses it.
    pub async fn restart(&self, repo: RepoPath<'_>, number: u64) -> Result<(), ClientError> {
        let path = format!("{}/prs/{number}/restart", repo.path());
        self.ask(Method::POST, &path, None::<&()>).await
    }

    /// # Errors
    ///
    /// The request fails, or CIHub refuses it.
    pub async fn graph(&self, queue: QueuePath<'_>) -> Result<Graph, ClientError> {
        self.get(&format!("{}/graph", queue.path())).await
    }
}

/// A repository as the URL of the API names it (`specs/API.md`): the slug of
/// the organization, and the name of the repository in it.
#[derive(Clone, Copy, Debug)]
pub struct RepoPath<'a> {
    pub org: &'a str,
    pub repo: &'a str,
}

impl RepoPath<'_> {
    fn path(self) -> String {
        format!("/orgs/{}/repos/{}", segment(self.org), segment(self.repo))
    }
}

/// A queue as the URL of the API names it (`specs/API.md`): the slug of the
/// organization, the name of the repository in it, and the name of the queue
/// in the repository.
#[derive(Clone, Copy, Debug)]
pub struct QueuePath<'a> {
    pub org: &'a str,
    pub repo: &'a str,
    pub queue: &'a str,
}

impl QueuePath<'_> {
    fn path(self) -> String {
        let repo = RepoPath {
            org: self.org,
            repo: self.repo,
        };
        format!("{}/queues/{}", repo.path(), segment(self.queue))
    }
}

/// `name` as one segment of a path. A queue named after its branch can hold a
/// `/`: every byte but the unreserved characters of RFC 3986 is
/// percent-encoded.
fn segment(name: &str) -> String {
    let mut encoded = String::with_capacity(name.len());

    for byte in name.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            // Writing to a `String` cannot fail.
            let _ = write!(encoded, "%{byte:02X}");
        }
    }

    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_queue_path_keeps_each_name_in_its_segment() {
        let queue = QueuePath {
            org: "acme",
            repo: "app.web",
            queue: "release/1.0 rc",
        };

        let path = queue.path();

        assert_eq!(path, "/orgs/acme/repos/app.web/queues/release%2F1.0%20rc");
    }
}

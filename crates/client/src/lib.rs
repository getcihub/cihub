//! The typed client of the public API v1 of CIHub (`docs/API.md`,
//! `DECISIONS.md` D161): the methods that the CLI uses, with the types of
//! `cihub-api-types`. It holds no server code and no queue logic.

use cihub_api_types::{
    API_PREFIX, Accepted, Graph, ItemPage, PrItem, Problem, Queue, QueueList, RepoList,
    RequestedQueueState, SubmitPr, UpdateQueue,
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
        // reqwest uses rustls without a provider (`DECISIONS.md` D80): ring is
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

    async fn send<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<&impl Serialize>,
    ) -> Result<T, ClientError> {
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
            return serde_json::from_str(&text)
                .map_err(|_| ClientError::Unexpected { status, body: text });
        }
        Err(match serde_json::from_str::<Problem>(&text) {
            Ok(problem) => ClientError::Api(problem),
            Err(_) => ClientError::Unexpected { status, body: text },
        })
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, ClientError> {
        self.send(Method::GET, path, None::<&()>).await
    }

    /// The repositories of the organization of the token.
    ///
    /// # Errors
    ///
    /// The request fails, or CIHub refuses it.
    pub async fn repos(&self) -> Result<RepoList, ClientError> {
        self.get("/repos").await
    }

    /// The queues of the repository `owner/name`.
    ///
    /// # Errors
    ///
    /// The request fails, or CIHub refuses it.
    pub async fn queues(&self, owner: &str, name: &str) -> Result<QueueList, ClientError> {
        self.get(&format!("/repos/{owner}/{name}/queues")).await
    }

    /// Asks the queue for `state`.
    ///
    /// # Errors
    ///
    /// The request fails, or CIHub refuses it.
    pub async fn set_queue_state(
        &self,
        queue_id: &str,
        state: RequestedQueueState,
    ) -> Result<Queue, ClientError> {
        let path = format!("/queues/{queue_id}");
        let body = UpdateQueue { state };
        self.send(Method::PATCH, &path, Some(&body)).await
    }

    /// The first page of the items of the queue, newest first.
    ///
    /// # Errors
    ///
    /// The request fails, or CIHub refuses it.
    pub async fn items(&self, queue_id: &str, take: u32) -> Result<ItemPage, ClientError> {
        self.get(&format!("/queues/{queue_id}/items?take={take}"))
            .await
    }

    /// Submits a PR to the queue.
    ///
    /// # Errors
    ///
    /// The request fails, or CIHub refuses it.
    pub async fn submit(
        &self,
        queue_id: &str,
        request: &SubmitPr,
    ) -> Result<Accepted, ClientError> {
        let path = format!("/queues/{queue_id}/items");
        self.send(Method::POST, &path, Some(request)).await
    }

    /// The last item of the PR `pr_number` in the queue, with its timeline.
    ///
    /// # Errors
    ///
    /// The request fails, or CIHub refuses it.
    pub async fn pr(
        &self,
        queue_id: &str,
        pr_number: u64,
        timeline: bool,
    ) -> Result<PrItem, ClientError> {
        let include = if timeline { "?include=timeline" } else { "" };
        self.get(&format!("/queues/{queue_id}/prs/{pr_number}{include}"))
            .await
    }

    /// # Errors
    ///
    /// The request fails, or CIHub refuses it.
    pub async fn cancel(&self, item_id: &str) -> Result<Accepted, ClientError> {
        let path = format!("/items/{item_id}/cancel");
        self.send(Method::POST, &path, None::<&()>).await
    }

    /// # Errors
    ///
    /// The request fails, or CIHub refuses it.
    pub async fn restart(&self, item_id: &str) -> Result<Accepted, ClientError> {
        let path = format!("/items/{item_id}/restart");
        self.send(Method::POST, &path, None::<&()>).await
    }

    /// # Errors
    ///
    /// The request fails, or CIHub refuses it.
    pub async fn graph(&self, queue_id: &str) -> Result<Graph, ClientError> {
        self.get(&format!("/queues/{queue_id}/graph")).await
    }
}

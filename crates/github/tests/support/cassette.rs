//! The recorded answers of GitHub (`AGENTS.md` D90): the world of the
//! contract suite, and each exchange in the order of the calls. The example
//! `github` writes them; the tests replay them.
#![allow(
    dead_code,
    reason = "shared by the tests and the example: each uses a part"
)]

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The headers of an answer that the adapter reads.
pub const KEPT_HEADERS: [&str; 7] = [
    "content-type",
    "retry-after",
    "x-ratelimit-limit",
    "x-ratelimit-remaining",
    "x-ratelimit-reset",
    "x-ratelimit-resource",
    "x-ratelimit-used",
];

/// The token of an installation, in place of the real one.
pub const RECORDED_TOKEN: &str = "ghs_recorded";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Cassette {
    /// When the recording started, in seconds since the epoch: the clock of
    /// the replay, so that the recorded token of the installation is valid.
    pub recorded_at: i64,
    pub world: World,
    pub exchanges: Vec<Exchange>,
}

/// The repository of the recording and what it held: the `Fixture` of the
/// contract suite.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct World {
    pub installation: String,
    pub namespace: String,
    pub name: String,
    pub base_branch: String,
    pub required_checks: BTreeSet<String>,
    pub up_to_date: bool,
    pub file_path: String,
    pub file_content: String,
    pub mergeable_pr: u64,
    pub conflicting_pr: u64,
    pub draft_pr: u64,
    pub missing_pr: u64,
    pub admin: String,
    pub work_branch: String,
}

/// One call and its answer. The path starts after the address of the API
/// (`/repos/o/r/pulls/1`, `/graphql`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Exchange {
    pub method: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<Value>,
    pub status: u16,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    /// The body of the answer when it is JSON.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<Value>,
    /// The body of the answer when it is not JSON, such as a raw file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// The path of a call after the address of the API: a GitHub Enterprise
/// Server serves the REST API under `/api/v3` and GraphQL at `/api/graphql`.
pub fn api_path(path: &str) -> Option<&str> {
    if path == "/api/graphql" {
        Some("/graphql")
    } else {
        path.strip_prefix("/api/v3")
    }
}

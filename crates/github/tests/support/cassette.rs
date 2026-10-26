//! The recorded answers of GitHub: the world of the contract suite, and each
//! exchange in the order of the calls. The example `github` writes them; the
//! tests replay them.
#![allow(
    dead_code,
    reason = "shared by the tests and the example: each uses a part"
)]

use std::collections::{BTreeMap, BTreeSet};

use cihub_scm::contract::Fixture;
use cihub_scm::{RequiredChecks, ScmUser};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const KEPT_HEADERS: [&str; 7] = [
    "content-type",
    "retry-after",
    "x-ratelimit-limit",
    "x-ratelimit-remaining",
    "x-ratelimit-reset",
    "x-ratelimit-resource",
    "x-ratelimit-used",
];

pub const RECORDED_TOKEN: &str = "ghs_recorded";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Cassette {
    /// When the recording started, in seconds since the epoch: the clock of
    /// the replay, so that the recorded token of the installation is valid.
    pub recorded_at: i64,
    pub world: World,
    pub exchanges: Vec<Exchange>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct World {
    pub installation: String,
    pub owner: String,
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

impl World {
    pub fn fixture(&self) -> Fixture {
        Fixture {
            base_branch: self.base_branch.clone(),
            required: RequiredChecks {
                names: self.required_checks.clone(),
                up_to_date: self.up_to_date,
            },
            file: (self.file_path.clone(), self.file_content.clone()),
            mergeable_pr: self.mergeable_pr,
            conflicting_pr: self.conflicting_pr,
            draft_pr: self.draft_pr,
            missing_pr: self.missing_pr,
            admin: ScmUser {
                external_id: String::new(),
                login: self.admin.clone(),
            },
            work_branch: self.work_branch.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Exchange {
    pub method: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<Value>,
    pub status: u16,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<Value>,
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

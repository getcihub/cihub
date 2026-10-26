use std::collections::{BTreeMap, BTreeSet};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use cihub_domain::{
    CheckConclusion, CheckReport, ConnectionId, MembershipRole, MergeMethod, Mergeability,
    Permission, PrState, ReviewDecision,
};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

/// The connection that a call uses: its quota, its budget and its credentials.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionRef {
    pub id: ConnectionId,
    /// The identifier of the connection on the SCM.
    pub external_id: String,
}

/// A repository, as a call names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoRef {
    pub connection: ConnectionRef,
    pub external_id: String,
    pub owner: String,
    pub name: String,
}

/// A user account on the SCM.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScmUser {
    pub external_id: String,
    pub login: String,
}

/// The state of a PR as the SCM reports it, in one read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrSnapshot {
    pub number: u64,
    /// The page of the PR on the SCM.
    pub url: String,
    pub state: PrState,
    pub head_sha: String,
    pub base_branch: String,
    pub head_branch: String,
    /// The head branch is in another repository.
    pub from_fork: bool,
    pub created_at: OffsetDateTime,
    pub draft: bool,
    /// The title and the description, for the title of the merge commit.
    pub title: String,
    pub body: String,
    pub labels: BTreeSet<String>,
    pub mergeability: Mergeability,
    pub review_decision: ReviewDecision,
    /// The logins whose last review approves the PR, or asks for changes.
    pub approved_by: BTreeSet<String>,
    pub changes_requested_by: BTreeSet<String>,
    /// The users and teams whose review the PR asks for.
    pub review_requested: BTreeSet<String>,
    /// The checks and statuses of the head.
    pub checks: Vec<CheckReport>,
    pub author: ScmUser,
    pub additions: u64,
    pub deletions: u64,
    pub changed_files: u64,
}

/// The check `name` has a passing conclusion on the head of `pr`.
pub fn check_passed(pr: &PrSnapshot, name: &str) -> bool {
    pr.checks.iter().any(|check| {
        check.name == name && check.conclusion.is_some_and(CheckConclusion::is_passing)
    })
}

/// The checks that the rules of a branch require before a merge.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RequiredChecks {
    pub names: BTreeSet<String>,
    /// The rules require the head of a PR to be up to date with its base.
    pub up_to_date: bool,
}

/// The state of a commit status that CIHub publishes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitStatus {
    Pending,
    Success,
    Failure,
    Error,
}

/// A commit status to publish on the commit `sha`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusUpdate {
    pub sha: String,
    pub context: String,
    pub status: CommitStatus,
    pub description: String,
    pub details_url: Option<String>,
}

/// A merge of the PR `number`, guarded by its head: the SCM refuses it when
/// the head is no longer `head_sha`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrMerge {
    pub number: u64,
    pub head_sha: String,
    /// The test commit that proved the PR: `fast_forward` moves the base
    /// there.
    pub tested_sha: String,
    pub method: MergeMethod,
    pub title: String,
    pub message: String,
}

/// The result of a merge of a commit into a ref.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MergeIntoRef {
    /// The ref moved to this new merge commit.
    Merged { sha: String },
    /// The ref already contains the commit: nothing changed.
    AlreadyContained,
    /// The commit conflicts with the ref.
    Conflict,
}

/// An attempt of the SCM to send a webhook delivery to CIHub.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentDelivery {
    /// The identifier that the delivery carries; its new attempts share it.
    pub delivery_id: String,
    /// The identifier of this attempt, to ask for the delivery again.
    pub attempt_id: String,
    /// Whether CIHub answered this attempt with a 2xx status code, which it
    /// does only once it stores the delivery.
    pub is_delivered: bool,
}

/// The account of the SCM that a connection serves: an organization or a
/// user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScmAccount {
    pub external_id: String,
    /// The name of the account in the paths of the SCM.
    pub path: String,
}

/// What an account shows of itself on the SCM.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScmAccountProfile {
    pub name: Option<String>,
    pub description: Option<String>,
    pub avatar_url: Option<String>,
}

/// An installation of the App of CIHub on an account, as the SCM reads it: the
/// permission name gives its access level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScmInstallation {
    pub external_id: String,
    pub account: ScmAccount,
    pub permissions: BTreeMap<String, String>,
    pub is_suspended: bool,
}

/// What the address of a sign-in carries: the SCM sends the person back to
/// `redirect_url` with a code and `state`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignInRequest {
    pub redirect_url: String,
    /// A random value that the callback checks, against a forged callback.
    pub state: String,
    /// The SHA-256 of the code verifier, in base64url without padding
    /// (PKCE, method `S256`).
    pub code_challenge: String,
}

/// The challenge of `verifier` for PKCE, method `S256`: the SHA-256 of the
/// verifier, in base64url without padding (RFC 7636).
pub fn code_challenge(verifier: &SecretString) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.expose_secret().as_bytes()))
}

/// The code that the SCM gives on the callback of a sign-in, and the
/// verifier that proves that CIHub asked for it.
#[derive(Debug)]
pub struct SignInCode {
    pub code: SecretString,
    pub code_verifier: SecretString,
    /// The `redirect_url` of the request, which the SCM checks again.
    pub redirect_url: String,
}

/// A person who signed in, the installations of the App that they reach,
/// and their role in each one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedIn {
    pub user: ScmUser,
    pub profile: ScmProfile,
    /// Every installation that the person reaches, as a sync reads it: the
    /// sign-in connects those that CIHub does not know yet.
    pub installations: Vec<ScmInstallation>,
    pub memberships: Vec<ScmMembership>,
}

/// The token of a person on the SCM, which reads what the person reaches, and
/// how long it and its refresh last.
#[derive(Debug)]
pub struct ScmPersonToken {
    pub access: SecretString,
    /// `None` for a token that does not expire.
    pub access_lifetime: Option<time::Duration>,
    /// Gives a new token once, then no more.
    pub refresh: Option<SecretString>,
    pub refresh_lifetime: Option<time::Duration>,
}

/// What the token of a person reaches: the installations of the App, and the
/// repositories of each one with the permission of the person.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScmPersonReach {
    pub installations: Vec<ScmInstallation>,
    pub repos: Vec<ScmPersonRepo>,
}

/// A repository that a person reaches through an installation of the App.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScmPersonRepo {
    /// The installation, as the `external_id` of its connection.
    pub connection_external_id: String,
    pub repo_external_id: String,
    pub permission: Permission,
}

/// What a person shows of themselves on the SCM.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScmProfile {
    pub name: Option<String>,
    pub avatar_url: Option<String>,
}

/// The role of a person in the account of an installation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScmMembership {
    /// The installation, as the `external_id` of its connection.
    pub connection_external_id: String,
    pub role: MembershipRole,
}

/// A member of the account of a connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScmMember {
    pub user: ScmUser,
    pub role: MembershipRole,
    /// The list of the members does not give the name.
    pub avatar_url: Option<String>,
}

/// A repository that a connection reaches, as the SCM lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScmRepo {
    pub external_id: String,
    pub owner: String,
    pub name: String,
    pub default_branch: String,
}

/// A point of the configuration of the SCM that the queue needs, and that a
/// repository misses (`specs/GITHUB.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SetupFinding {
    /// No rule restricts the updates of the target branch: a merge can skip
    /// the queue.
    UpdateRestrictionMissing,
    /// A rule restricts the updates of the target branch, but does not let
    /// CIHub through: the queue cannot merge.
    AppNotExempt,
    /// A rule covers the test branches too: the queue cannot write them.
    TestBranchesNotExcluded,
    /// The rules ask that a PR be up to date with its base: each merge makes
    /// the next PRs wait.
    UpToDateRequired,
}

impl SetupFinding {
    pub fn code(self) -> &'static str {
        match self {
            Self::UpdateRestrictionMissing => "update_restriction_missing",
            Self::AppNotExempt => "app_not_exempt",
            Self::TestBranchesNotExcluded => "test_branches_not_excluded",
            Self::UpToDateRequired => "up_to_date_required",
        }
    }
}

//! An SCM in memory, for the tests of CIHub. It keeps repositories with
//! commits, branches, PRs, checks, comments and rules; it records each call; it
//! can fail calls and lose, duplicate or reorder its webhook deliveries. It has
//! no clock: a test gives every time.

mod calls;
mod deliveries;
mod faults;
mod git;
mod people;
mod pr;
#[cfg(test)]
mod tests;
mod world;

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::sync::{Mutex, PoisonError};

use cihub_domain::{CheckReport, JobClass, Permission};

use crate::{
    RepoRef, RequiredChecks, ScmAccountProfile, ScmDelivery, ScmError, ScmEvent, ScmFuture,
    ScmInstallation, ScmMember, ScmProfile, ScmUser, SignInRequest,
};

pub use faults::{Fault, RandomFaults, ScmMethod, SimCall, WebhookFaults};
pub use pr::{PrChange, Reviews, UpdateRestriction};
pub use world::contract_world;

use deliveries::Attempt;
use git::Commit;
use pr::{Comment, Pr};

/// The user that acts for CIHub on the simulator.
pub fn bot() -> ScmUser {
    ScmUser {
        external_id: "1".into(),
        login: "cihub-bot".into(),
    }
}

/// The simulator. Every method takes `&self`, so that the tests and the
/// executors can share it behind an `Arc`.
#[derive(Debug, Default)]
pub struct Sim {
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    next_sha: u64,
    next_comment: u64,
    repos: BTreeMap<String, Repo>,
    installations: BTreeMap<String, ScmInstallation>,
    /// The connections that fail every call with `AccessLost`: suspended, or
    /// whose installation was removed.
    suspended_connections: BTreeSet<String>,
    /// The members of the account of each installation, by the external
    /// identifier of the user.
    members: BTreeMap<String, BTreeMap<String, ScmMember>>,
    /// The profile of each user, by their external identifier.
    profiles: BTreeMap<String, ScmProfile>,
    /// The profile of each account, by its external identifier.
    account_profiles: BTreeMap<String, ScmAccountProfile>,
    /// The sign-ins that wait for the exchange of their code, by code.
    sign_in_codes: BTreeMap<String, PendingSignIn>,
    next_sign_in_code: u64,
    /// The page that a browser opens to sign in, for a test in a browser;
    /// `sim:authorize` without it.
    sign_in_page: Option<String>,
    /// The page that installs the App, for a test in a browser; `sim:install`
    /// without it.
    install_page: Option<String>,
    /// The person of each token that a sign-in or a refresh gave, by token.
    person_tokens: BTreeMap<String, String>,
    /// The person of each refresh token not used yet, by refresh token.
    refresh_tokens: BTreeMap<String, String>,
    next_person_token: u64,
    faults: VecDeque<Fault>,
    random: Option<RandomFaults>,
    calls: Vec<SimCall>,
    deliveries: Vec<ScmDelivery>,
    attempts: Vec<Attempt>,
    redelivered: Vec<usize>,
    delivered: u64,
}

/// A sign-in that waits for the exchange of its code: the person and the
/// request of CIHub.
#[derive(Debug)]
struct PendingSignIn {
    user: ScmUser,
    request: SignInRequest,
}

#[derive(Debug)]
struct Repo {
    connection: String,
    owner: String,
    name: String,
    default_branch: String,
    commits: HashMap<String, Commit>,
    branches: BTreeMap<String, String>,
    prs: BTreeMap<u64, Pr>,
    checks: HashMap<String, Vec<CheckReport>>,
    /// The comments of every PR, by identifier, in the order of their
    /// creation.
    comments: BTreeMap<u64, Comment>,
    acknowledged: BTreeSet<u64>,
    rules: BTreeMap<String, RequiredChecks>,
    restrictions: BTreeMap<String, UpdateRestriction>,
    permissions: BTreeMap<String, Permission>,
}

/// A repository and the counters of the simulator, for a change of the
/// repository that sends its deliveries.
struct Ctx<'a> {
    repo_external_id: String,
    repo: &'a mut Repo,
    next_sha: &'a mut u64,
    next_comment: &'a mut u64,
    deliveries: &'a mut Vec<ScmDelivery>,
}

impl Sim {
    /// A simulator with no repository, no installation and no fault.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates the repository `repo` with a first commit of `files` on the
    /// branch `default_branch`, and returns the commit.
    pub fn add_repo(&self, repo: &RepoRef, default_branch: &str, files: &[(&str, &str)]) -> String {
        self.with(|state| {
            let tree = files
                .iter()
                .map(|(path, content)| ((*path).to_owned(), (*content).to_owned()))
                .collect();
            let empty = Repo {
                connection: repo.connection.external_id.clone(),
                owner: repo.owner.clone(),
                name: repo.name.clone(),
                default_branch: default_branch.to_owned(),
                commits: HashMap::new(),
                branches: BTreeMap::new(),
                prs: BTreeMap::new(),
                checks: HashMap::new(),
                comments: BTreeMap::new(),
                acknowledged: BTreeSet::new(),
                rules: BTreeMap::new(),
                restrictions: BTreeMap::new(),
                permissions: BTreeMap::new(),
            };
            let added = state
                .repos
                .entry(repo.external_id.clone())
                .insert_entry(empty)
                .into_mut();
            let sha = added.new_commit(&mut state.next_sha, Vec::new(), tree);
            added
                .branches
                .insert(default_branch.to_owned(), sha.clone());

            sha
        })
    }

    /// Gives the repository `repo_external_id` to the installation
    /// `connection_external_id`, as an owner who changes the repositories of
    /// the App: the installation that had it no longer lists it.
    pub fn move_repo(&self, repo_external_id: &str, connection_external_id: &str) {
        self.with(|state| {
            if let Some(repo) = state.repos.get_mut(repo_external_id) {
                connection_external_id.clone_into(&mut repo.connection);
            }
        });
    }

    /// The head of the branch `branch`, without a call to count.
    pub fn branch(&self, repo: &RepoRef, branch: &str) -> Option<String> {
        self.with(|state| {
            state
                .repos
                .get(&repo.external_id)
                .and_then(|repo| repo.branches.get(branch).cloned())
        })
    }

    fn with<T>(&self, run: impl FnOnce(&mut State) -> T) -> T {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        run(&mut state)
    }

    fn with_repo<T>(
        &self,
        repo: &str,
        run: impl FnOnce(&mut Ctx<'_>) -> Result<T, ScmError>,
    ) -> Result<T, ScmError> {
        self.with(|state| state.ctx(repo).and_then(|mut ctx| run(&mut ctx)))
    }

    /// A call: it is recorded, and can fail.
    fn call<T: Send + 'static>(
        &self,
        method: ScmMethod,
        class: JobClass,
        run: impl FnOnce(&mut State) -> Result<T, ScmError>,
    ) -> ScmFuture<'_, T> {
        let result = self.with(|state| {
            state.calls.push(SimCall { method, class });
            match state.take_fault(method) {
                Some(error) => Err(error),
                None => run(state),
            }
        });
        Box::pin(std::future::ready(result))
    }

    /// A call on `repo`: it is recorded, can fail, and fails with
    /// `AccessLost` when the connection of `repo` is suspended.
    fn call_on_repo<T: Send + 'static>(
        &self,
        method: ScmMethod,
        repo: &RepoRef,
        class: JobClass,
        run: impl FnOnce(&mut Ctx<'_>) -> Result<T, ScmError>,
    ) -> ScmFuture<'_, T> {
        self.call(method, class, |state| {
            state.check_access(&repo.connection.external_id)?;
            state
                .ctx(&repo.external_id)
                .and_then(|mut ctx| run(&mut ctx))
        })
    }
}

impl State {
    fn check_access(&self, connection_external_id: &str) -> Result<(), ScmError> {
        if self.suspended_connections.contains(connection_external_id) {
            return Err(ScmError::AccessLost("connection suspended".into()));
        }

        Ok(())
    }

    fn ctx(&mut self, repo: &str) -> Result<Ctx<'_>, ScmError> {
        let State {
            next_sha,
            next_comment,
            repos,
            deliveries,
            ..
        } = self;
        let repo_state = repos
            .get_mut(repo)
            .ok_or_else(|| ScmError::NotFound(format!("repository {repo}")))?;

        Ok(Ctx {
            repo_external_id: repo.to_owned(),
            repo: repo_state,
            next_sha,
            next_comment,
            deliveries,
        })
    }
}

impl Ctx<'_> {
    fn emit(&mut self, event: ScmEvent) {
        self.deliveries.push(ScmDelivery {
            connection_external_id: Some(self.repo.connection.clone()),
            event,
        });
    }
}

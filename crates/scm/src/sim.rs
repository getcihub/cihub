//! An SCM in memory, for the tests of CIHub. It keeps repositories with
//! commits, branches, PRs, checks, comments and rules; it records each call; it
//! can fail calls and lose, duplicate or reorder its webhook deliveries. It has
//! no clock: a test gives every time.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::fmt::Write;
use std::ops::Range;
use std::sync::{Mutex, PoisonError};

use cihub_domain::{
    CheckConclusion, CheckKind, CheckReport, JobClass, MembershipRole, MergeMethod, Mergeability,
    Permission, PrState, ReviewDecision,
};
use secrecy::ExposeSecret;
use time::OffsetDateTime;

use crate::contract::Fixture;
use crate::{
    CommitStatus, ConnectionRef, MergeIntoRef, PrMerge, PrSnapshot, RecentDelivery, RepoRef,
    RequiredChecks, Scm, ScmAccount, ScmDelivery, ScmError, ScmEvent, ScmFuture, ScmInstallation,
    ScmMember, ScmMembership, ScmRepo, ScmUser, SetupFinding, SignInCode, SignInRequest, SignedIn,
    StatusUpdate, code_challenge,
};

/// The user that acts for CIHub on the simulator.
pub fn bot() -> ScmUser {
    ScmUser {
        external_id: "1".into(),
        login: "cihub-bot".into(),
    }
}

/// Makes the next calls fail with `error`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fault {
    /// The method of [`Scm`] to fail, or `None` for every method.
    pub call: Option<&'static str>,
    pub error: ScmError,
    /// The number of calls to fail.
    pub times: u32,
}

/// A call that the simulator received.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimCall {
    pub name: &'static str,
    pub class: JobClass,
}

/// How [`Sim::take_deliveries_with`] damages the deliveries. The same seed
/// gives the same damage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebhookFaults {
    pub seed: u64,
    /// The share of deliveries lost, in percent.
    pub lose_percent: u8,
    /// The share of deliveries sent twice, in percent.
    pub duplicate_percent: u8,
    pub reorder: bool,
}

fn splitmix(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *seed;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);

    z ^ (z >> 31)
}

impl WebhookFaults {
    fn next(&mut self) -> u64 {
        splitmix(&mut self.seed)
    }

    fn chance(&mut self, percent: u8) -> bool {
        self.next() % 100 < u64::from(percent)
    }
}

/// Makes calls fail at random, after the faults of [`Sim::fail`]. The same seed
/// gives the same failures for the same sequence of calls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RandomFaults {
    pub seed: u64,
    /// The share of calls that fail with a server error, in percent.
    pub unavailable_percent: u8,
    /// The share of calls that fail with a spent rate limit, in percent.
    pub rate_limited_percent: u8,
    /// The end of the rate limit that such a failure reports.
    pub rate_limited_until: OffsetDateTime,
}

impl RandomFaults {
    fn draw(&mut self) -> Option<ScmError> {
        let roll = splitmix(&mut self.seed) % 100;
        let unavailable = u64::from(self.unavailable_percent);
        if roll < unavailable {
            Some(ScmError::Unavailable("random server error".into()))
        } else if roll < unavailable + u64::from(self.rate_limited_percent) {
            Some(ScmError::RateLimited {
                until: self.rate_limited_until,
            })
        } else {
            None
        }
    }
}

type Tree = BTreeMap<String, String>;

#[derive(Debug)]
struct Commit {
    parents: Vec<String>,
    tree: Tree,
}

#[derive(Debug)]
struct Pr {
    head_branch: String,
    last_head: String,
    base_branch: String,
    state: PrState,
    draft: bool,
    title: String,
    body: String,
    labels: BTreeSet<String>,
    review_decision: ReviewDecision,
    reviews: Reviews,
    unresolved_threads: Vec<String>,
    author: ScmUser,
    mergeability: Option<Mergeability>,
    merged_sha: Option<String>,
}

/// Who reviewed a PR, and whose review it asks for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reviews {
    pub approved_by: BTreeSet<String>,
    pub changes_requested_by: BTreeSet<String>,
    pub requested: BTreeSet<String>,
}

#[derive(Debug)]
struct Repo {
    connection: String,
    namespace: String,
    name: String,
    default_branch: String,
    commits: HashMap<String, Commit>,
    branches: BTreeMap<String, String>,
    prs: BTreeMap<u64, Pr>,
    checks: HashMap<String, Vec<CheckReport>>,
    comments: BTreeMap<String, (u64, String)>,
    acknowledged: BTreeSet<String>,
    rules: BTreeMap<String, RequiredChecks>,
    permissions: BTreeMap<String, Permission>,
}

#[derive(Debug, Default)]
struct State {
    next_sha: u64,
    next_comment: u64,
    repos: BTreeMap<String, Repo>,
    installations: BTreeMap<String, ScmInstallation>,
    /// The members of the account of each installation, by the external
    /// identifier of the user.
    members: BTreeMap<String, BTreeMap<String, ScmMember>>,
    /// The sign-ins that wait for the exchange of their code: the person and
    /// the request of CIHub.
    sign_in_codes: BTreeMap<String, (ScmUser, SignInRequest)>,
    next_sign_in_code: u64,
    /// The page that a browser opens to sign in, for a test in a browser;
    /// `sim:authorize` without it.
    sign_in_page: Option<String>,
    lost: BTreeSet<String>,
    faults: VecDeque<Fault>,
    random: Option<RandomFaults>,
    calls: Vec<SimCall>,
    deliveries: Vec<ScmDelivery>,
    attempts: Vec<Attempt>,
    redelivered: Vec<usize>,
    delivered: u64,
}

#[derive(Clone, Debug)]
struct Attempt {
    delivery_id: String,
    delivery: ScmDelivery,
    is_delivered: bool,
}

/// The simulator. Every method takes `&self`, so that the tests and the
/// executors can share it behind an `Arc`.
#[derive(Debug, Default)]
pub struct Sim {
    state: Mutex<State>,
}

impl Sim {
    pub fn new() -> Self {
        Self::default()
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
                namespace: repo.namespace.clone(),
                name: repo.name.clone(),
                default_branch: default_branch.to_owned(),
                commits: HashMap::new(),
                branches: BTreeMap::new(),
                prs: BTreeMap::new(),
                checks: HashMap::new(),
                comments: BTreeMap::new(),
                acknowledged: BTreeSet::new(),
                rules: BTreeMap::new(),
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

    /// Creates a commit on `parent` that writes the files with `Some` and
    /// deletes the files with `None`, and returns it.
    ///
    /// # Errors
    ///
    /// `NotFound` for an unknown repository or parent.
    pub fn commit(
        &self,
        repo: &RepoRef,
        parent: &str,
        files: &[(&str, Option<&str>)],
    ) -> Result<String, ScmError> {
        self.with_repo(&repo.external_id, |ctx| {
            let mut tree = ctx.repo.tree(parent)?.clone();
            for (path, content) in files {
                match content {
                    Some(content) => tree.insert((*path).to_owned(), (*content).to_owned()),
                    None => tree.remove(*path),
                };
            }
            Ok(ctx.new_commit(vec![parent.to_owned()], tree))
        })
    }

    /// Moves the branch `branch` to `sha`, as a push of `sender` from
    /// outside CIHub.
    ///
    /// # Errors
    ///
    /// `NotFound` for an unknown repository or commit.
    pub fn push(
        &self,
        repo: &RepoRef,
        branch: &str,
        sha: &str,
        sender: &ScmUser,
    ) -> Result<(), ScmError> {
        self.with_repo(&repo.external_id, |ctx| {
            ctx.repo.tree(sha)?;
            ctx.move_branch(branch, Some(sha), sender);
            Ok(())
        })
    }

    /// Opens a PR of `author` from `head_branch` into `base_branch`, and
    /// returns its number.
    ///
    /// # Errors
    ///
    /// `NotFound` for an unknown repository or branch.
    pub fn open_pr(
        &self,
        repo: &RepoRef,
        head_branch: &str,
        base_branch: &str,
        author: &ScmUser,
    ) -> Result<u64, ScmError> {
        self.with_repo(&repo.external_id, |ctx| {
            ctx.new_pr(head_branch, base_branch, author, (head_branch, ""), false)
        })
    }

    /// Changes the PR `number` with `change`, and sends its delivery.
    ///
    /// # Errors
    ///
    /// `NotFound` for an unknown repository or PR.
    pub fn change_pr(&self, repo: &RepoRef, number: u64, change: PrChange) -> Result<(), ScmError> {
        self.with_repo(&repo.external_id, |ctx| {
            let pr = ctx.repo.pr_mut(number)?;
            let ready = matches!(change, PrChange::Draft(false)) && pr.draft;

            match change {
                PrChange::Draft(draft) => pr.draft = draft,
                PrChange::Review(decision) => pr.review_decision = decision,
                PrChange::Mergeability(mergeability) => pr.mergeability = mergeability,
                PrChange::Title(title) => title.clone_into(&mut pr.title),
                PrChange::Close => pr.state = PrState::Closed,
            }

            if ready {
                ctx.pr_opened(number);
            } else {
                ctx.pr_changed(number);
            }

            Ok(())
        })
    }

    /// Reports a check of the CI on the commit `sha`; a report with the same
    /// name and kind replaces the previous one.
    ///
    /// # Errors
    ///
    /// `NotFound` for an unknown repository.
    pub fn report_check(
        &self,
        repo: &RepoRef,
        sha: &str,
        check: CheckReport,
    ) -> Result<(), ScmError> {
        self.with_repo(&repo.external_id, |ctx| {
            ctx.report(sha, check);
            Ok(())
        })
    }

    /// Checks, as `user`, the submission box of the comment of CIHub on the
    /// PR `number`, and sends its delivery.
    ///
    /// # Errors
    ///
    /// `NotFound` for an unknown repository, or when no comment of the PR
    /// has an empty box.
    pub fn check_submit_box(
        &self,
        repo: &RepoRef,
        number: u64,
        user: &ScmUser,
    ) -> Result<(), ScmError> {
        let empty = format!("- [ ] {}", crate::SUBMIT_BOX);
        self.with_repo(&repo.external_id, |ctx| {
            let (id, (_, body)) = ctx
                .repo
                .comments
                .iter_mut()
                .find(|(_, (pr, body))| *pr == number && body.contains(&empty))
                .ok_or_else(|| ScmError::NotFound(format!("a box on PR {number}")))?;
            *body = body.replace(&empty, &format!("- [x] {}", crate::SUBMIT_BOX));
            let comment_external_id = id.clone();
            ctx.emit(ScmEvent::SubmitBoxChecked {
                repo_external_id: ctx.repo_external_id.clone(),
                number,
                comment_external_id,
                sender: user.clone(),
            });

            Ok(())
        })
    }

    /// Comments on the PR `number` as `author`.
    ///
    /// # Errors
    ///
    /// `NotFound` for an unknown repository or PR.
    pub fn comment(
        &self,
        repo: &RepoRef,
        number: u64,
        author: &ScmUser,
        body: &str,
    ) -> Result<(), ScmError> {
        self.with_repo(&repo.external_id, |ctx| {
            ctx.add_comment(number, author, body).map(drop)
        })
    }

    /// The bodies of the comments of the PR `number` that CIHub
    /// acknowledged, in their order.
    pub fn acknowledged_comments(&self, repo: &RepoRef, number: u64) -> Vec<String> {
        self.with(|state| {
            state
                .repos
                .get(&repo.external_id)
                .map(|repo| {
                    repo.comments
                        .iter()
                        .filter(|(id, (pr, _))| *pr == number && repo.acknowledged.contains(*id))
                        .map(|(_, (_, body))| body.clone())
                        .collect()
                })
                .unwrap_or_default()
        })
    }

    /// Sets what the rules of the branch `branch` require.
    ///
    /// # Errors
    ///
    /// `NotFound` for an unknown repository.
    pub fn set_rules(
        &self,
        repo: &RepoRef,
        branch: &str,
        rules: RequiredChecks,
    ) -> Result<(), ScmError> {
        self.with_repo(&repo.external_id, |ctx| {
            ctx.repo.rules.insert(branch.to_owned(), rules);
            let repo_external_id = ctx.repo_external_id.clone();
            ctx.emit(ScmEvent::RulesChanged { repo_external_id });
            Ok(())
        })
    }

    /// Sets the reviews of the PR `number`, and sends its delivery. The
    /// review decision does not follow: [`PrChange::Review`] sets it.
    ///
    /// # Errors
    ///
    /// `NotFound` for an unknown repository or PR.
    pub fn set_reviews(
        &self,
        repo: &RepoRef,
        number: u64,
        reviews: Reviews,
    ) -> Result<(), ScmError> {
        self.with_repo(&repo.external_id, |ctx| {
            ctx.repo.pr_mut(number)?.reviews = reviews;
            ctx.pr_changed(number);
            Ok(())
        })
    }

    /// Sets the paths of the unresolved review threads of the PR `number`.
    ///
    /// # Errors
    ///
    /// `NotFound` for an unknown repository or PR.
    pub fn set_unresolved_threads(
        &self,
        repo: &RepoRef,
        number: u64,
        paths: &[&str],
    ) -> Result<(), ScmError> {
        self.with_repo(&repo.external_id, |ctx| {
            ctx.repo.pr_mut(number)?.unresolved_threads =
                paths.iter().map(|path| (*path).to_owned()).collect();
            Ok(())
        })
    }

    /// Gives `user` the permission `permission` on the repository.
    ///
    /// # Errors
    ///
    /// `NotFound` for an unknown repository.
    pub fn set_permission(
        &self,
        repo: &RepoRef,
        user: &ScmUser,
        permission: Permission,
    ) -> Result<(), ScmError> {
        self.with_repo(&repo.external_id, |ctx| {
            ctx.repo
                .permissions
                .insert(user.external_id.clone(), permission);
            Ok(())
        })
    }

    /// Installs the App on `account` as the connection
    /// `connection_external_id`, with `permissions`.
    pub fn add_installation(
        &self,
        connection_external_id: &str,
        account: ScmAccount,
        permissions: &[(&str, &str)],
    ) {
        let installation = ScmInstallation {
            external_id: connection_external_id.to_owned(),
            account,
            permissions: permission_levels(permissions),
            is_suspended: false,
        };
        self.with(|state| {
            state
                .installations
                .insert(connection_external_id.to_owned(), installation);
            state.connection_changed(connection_external_id);
        });
    }

    /// Gives the installation `connection_external_id` the permissions
    /// `permissions`, as an owner who accepts new permissions.
    pub fn set_permissions(&self, connection_external_id: &str, permissions: &[(&str, &str)]) {
        self.with(|state| {
            let Some(installation) = state.installations.get_mut(connection_external_id) else {
                return;
            };

            installation.permissions = permission_levels(permissions);
            state.connection_changed(connection_external_id);
        });
    }

    /// Deletes the installation `connection_external_id`: its connection
    /// fails every call with `AccessLost`.
    pub fn remove_installation(&self, connection_external_id: &str) {
        self.with(|state| {
            if state.installations.remove(connection_external_id).is_none() {
                return;
            }

            state.lost.insert(connection_external_id.to_owned());
            state.connection_changed(connection_external_id);
        });
    }

    /// Gives `user` the role `role` in the account of the installation
    /// `connection_external_id`.
    pub fn set_member(&self, connection_external_id: &str, user: &ScmUser, role: MembershipRole) {
        self.with(|state| {
            let member = ScmMember {
                user: user.clone(),
                role,
            };
            state
                .members
                .entry(connection_external_id.to_owned())
                .or_default()
                .insert(user.external_id.clone(), member);
        });
    }

    /// Takes `user` out of the account of the installation
    /// `connection_external_id`.
    pub fn remove_member(&self, connection_external_id: &str, user: &ScmUser) {
        self.with(|state| {
            if let Some(members) = state.members.get_mut(connection_external_id) {
                members.remove(&user.external_id);
            }
        });
    }

    /// Makes the address of a sign-in a page that a browser opens: a test
    /// server serves it, and signs a person in with [`Sim::authorize`].
    pub fn serve_sign_in_at(&self, page: &str) {
        self.with(|state| state.sign_in_page = Some(page.to_owned()));
    }

    /// Signs `user` in on the page of the SCM for `request`, as after their
    /// consent, and returns the code that the SCM sends to its
    /// `redirect_url`.
    pub fn authorize(&self, user: &ScmUser, request: &SignInRequest) -> String {
        self.with(|state| {
            state.next_sign_in_code += 1;
            let code = format!("code-{}", state.next_sign_in_code);
            state
                .sign_in_codes
                .insert(code.clone(), (user.clone(), request.clone()));
            code
        })
    }

    /// Suspends the connection `connection_external_id`, or makes it active
    /// again: a suspended connection fails every call with `AccessLost`.
    pub fn set_access(&self, connection_external_id: &str, active: bool) {
        self.with(|state| {
            let changed = if active {
                state.lost.remove(connection_external_id)
            } else {
                state.lost.insert(connection_external_id.to_owned())
            };
            if changed {
                state.connection_changed(connection_external_id);
            }
        });
    }

    /// Makes the next calls fail, after the faults already waiting.
    pub fn fail(&self, fault: Fault) {
        self.with(|state| state.faults.push_back(fault));
    }

    /// Makes calls fail at random as `faults` says, or stops it with `None`.
    pub fn fail_at_random(&self, faults: Option<RandomFaults>) {
        self.with(|state| state.random = faults);
    }

    /// The calls received since the start.
    pub fn calls(&self) -> Vec<SimCall> {
        self.with(|state| state.calls.clone())
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

    /// The bodies of the comments of the PR `number`, in their order.
    pub fn comments(&self, repo: &RepoRef, number: u64) -> Vec<String> {
        self.with(|state| {
            state
                .repos
                .get(&repo.external_id)
                .map(|repo| {
                    repo.comments
                        .values()
                        .filter(|(pr, _)| *pr == number)
                        .map(|(_, body)| body.clone())
                        .collect()
                })
                .unwrap_or_default()
        })
    }

    /// The deliveries sent since the last take, in their order, then the
    /// attempts asked again. The `n`-th new delivery taken has the identifier
    /// `sim-n`; an attempt asked again keeps the identifier of its delivery.
    pub fn take_deliveries(&self) -> Vec<ScmDelivery> {
        self.with(|state| {
            let taken = state.take_attempts();
            state.attempts[taken]
                .iter()
                .map(|attempt| attempt.delivery.clone())
                .collect()
        })
    }

    /// The deliveries sent since the last take, lost, sent twice and
    /// reordered as `faults` says. A lost attempt is not delivered.
    pub fn take_deliveries_with(&self, faults: &mut WebhookFaults) -> Vec<ScmDelivery> {
        let mut deliveries = Vec::new();
        self.with(|state| {
            let taken = state.take_attempts();
            for attempt in &mut state.attempts[taken] {
                if faults.chance(faults.lose_percent) {
                    attempt.is_delivered = false;
                    continue;
                }

                if faults.chance(faults.duplicate_percent) {
                    deliveries.push(attempt.delivery.clone());
                }
                deliveries.push(attempt.delivery.clone());
            }
        });
        if faults.reorder {
            for i in (1..deliveries.len()).rev() {
                let j = usize::try_from(faults.next() % (i as u64 + 1)).unwrap_or(i);
                deliveries.swap(i, j);
            }
        }

        deliveries
    }

    fn record_call<T: Send + 'static>(
        &self,
        name: &'static str,
        class: JobClass,
        run: impl FnOnce(&mut State) -> Result<T, ScmError>,
    ) -> ScmFuture<'_, T> {
        let result = self.with(|state| {
            state.calls.push(SimCall { name, class });
            match state.take_fault(name) {
                Some(error) => Err(error),
                None => run(state),
            }
        });
        Box::pin(std::future::ready(result))
    }

    fn call<T: Send + 'static>(
        &self,
        name: &'static str,
        repo: &RepoRef,
        class: JobClass,
        run: impl FnOnce(&mut Ctx<'_>) -> Result<T, ScmError>,
    ) -> ScmFuture<'_, T> {
        self.record_call(name, class, |state| {
            if state.lost.contains(&repo.connection.external_id) {
                return Err(ScmError::AccessLost("connection suspended".into()));
            }
            state
                .ctx(&repo.external_id)
                .and_then(|mut ctx| run(&mut ctx))
        })
    }
}

/// A change of a PR from outside CIHub.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrChange {
    Draft(bool),
    Review(ReviewDecision),
    /// Forces the mergeability, or computes it again with `None`.
    Mergeability(Option<Mergeability>),
    Title(&'static str),
    Close,
}

/// `text` for the query of an address: every byte but the unreserved ones
/// of RFC 3986 is percent-encoded.
fn encode_query(text: &str) -> String {
    text.bytes()
        .fold(String::with_capacity(text.len()), |mut encoded, byte| {
            if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
                encoded.push(char::from(byte));
            } else {
                let _ = write!(encoded, "%{byte:02X}");
            }
            encoded
        })
}

fn permission_levels(permissions: &[(&str, &str)]) -> BTreeMap<String, String> {
    permissions
        .iter()
        .map(|(name, level)| ((*name).to_owned(), (*level).to_owned()))
        .collect()
}

impl State {
    fn take_attempts(&mut self) -> Range<usize> {
        let first = self.attempts.len();
        for delivery in std::mem::take(&mut self.deliveries) {
            self.delivered += 1;
            self.attempts.push(Attempt {
                delivery_id: format!("sim-{}", self.delivered),
                delivery,
                is_delivered: true,
            });
        }

        for attempt in std::mem::take(&mut self.redelivered) {
            let again = Attempt {
                is_delivered: true,
                ..self.attempts[attempt].clone()
            };
            self.attempts.push(again);
        }

        first..self.attempts.len()
    }

    fn connection_changed(&mut self, connection_external_id: &str) {
        self.deliveries.push(ScmDelivery {
            connection_external_id: Some(connection_external_id.to_owned()),
            event: ScmEvent::ConnectionChanged,
        });
    }

    fn installation(&self, external_id: &str) -> Option<ScmInstallation> {
        let mut installation = self.installations.get(external_id)?.clone();
        installation.is_suspended = self.lost.contains(external_id);
        Some(installation)
    }

    fn take_fault(&mut self, name: &str) -> Option<ScmError> {
        let Some(index) = self
            .faults
            .iter()
            .position(|fault| fault.call.is_none_or(|call| call == name))
        else {
            return self.random.as_mut().and_then(RandomFaults::draw);
        };

        let fault = &mut self.faults[index];
        let error = fault.error.clone();

        fault.times = fault.times.saturating_sub(1);
        if fault.times == 0 {
            self.faults.remove(index);
        }

        Some(error)
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

struct Ctx<'a> {
    repo_external_id: String,
    repo: &'a mut Repo,
    next_sha: &'a mut u64,
    next_comment: &'a mut u64,
    deliveries: &'a mut Vec<ScmDelivery>,
}

impl Ctx<'_> {
    fn emit(&mut self, event: ScmEvent) {
        self.deliveries.push(ScmDelivery {
            connection_external_id: Some(self.repo.connection.clone()),
            event,
        });
    }

    fn new_commit(&mut self, parents: Vec<String>, tree: Tree) -> String {
        self.repo.new_commit(self.next_sha, parents, tree)
    }

    fn move_branch(&mut self, branch: &str, sha: Option<&str>, sender: &ScmUser) {
        let before = match sha {
            Some(sha) => self.repo.branches.insert(branch.to_owned(), sha.to_owned()),
            None => self.repo.branches.remove(branch),
        };
        if before.as_deref() == sha {
            return;
        }

        self.emit(ScmEvent::Pushed {
            repo_external_id: self.repo_external_id.clone(),
            branch: branch.to_owned(),
            before,
            after: sha.map(str::to_owned),
            sender: sender.clone(),
        });

        let changed: Vec<u64> = self
            .repo
            .prs
            .iter()
            .filter(|(_, pr)| {
                pr.state == PrState::Open && (pr.head_branch == branch || pr.base_branch == branch)
            })
            .map(|(number, _)| *number)
            .collect();
        for number in changed {
            self.pr_changed(number);
        }
    }

    fn pr_changed(&mut self, number: u64) {
        self.pr_event(number, None, false);
    }

    fn pr_opened(&mut self, number: u64) {
        self.pr_event(number, None, true);
    }

    fn pr_event(&mut self, number: u64, label_added: Option<String>, opened: bool) {
        let Some(pr) = self.repo.prs.get(&number) else {
            return;
        };

        let base_branch = pr.base_branch.clone();
        self.emit(ScmEvent::PrChanged {
            repo_external_id: self.repo_external_id.clone(),
            number,
            base_branch,
            label_added,
            opened,
        });
    }

    fn new_pr(
        &mut self,
        head_branch: &str,
        base_branch: &str,
        author: &ScmUser,
        (title, body): (&str, &str),
        draft: bool,
    ) -> Result<u64, ScmError> {
        let head = self.repo.branch_head(head_branch)?;
        self.repo.branch_head(base_branch)?;
        let number = self.repo.prs.keys().next_back().map_or(1, |last| last + 1);
        self.repo.prs.insert(
            number,
            Pr {
                head_branch: head_branch.to_owned(),
                last_head: head,
                base_branch: base_branch.to_owned(),
                state: PrState::Open,
                draft,
                title: title.to_owned(),
                body: body.to_owned(),
                labels: BTreeSet::new(),
                review_decision: ReviewDecision::None,
                reviews: Reviews::default(),
                unresolved_threads: Vec::new(),
                author: author.clone(),
                mergeability: None,
                merged_sha: None,
            },
        );

        if draft {
            self.pr_changed(number);
        } else {
            self.pr_opened(number);
        }

        Ok(number)
    }

    fn report(&mut self, sha: &str, check: CheckReport) {
        let reports = self.repo.checks.entry(sha.to_owned()).or_default();
        reports.retain(|report| report.name != check.name || report.kind != check.kind);
        reports.push(check.clone());
        let branches = self
            .repo
            .branches
            .iter()
            .filter(|(_, head)| *head == sha)
            .map(|(branch, _)| branch.clone())
            .collect();
        self.emit(ScmEvent::CheckChanged {
            repo_external_id: self.repo_external_id.clone(),
            sha: sha.to_owned(),
            branches,
            check,
        });
    }

    fn add_comment(
        &mut self,
        number: u64,
        author: &ScmUser,
        body: &str,
    ) -> Result<String, ScmError> {
        self.repo.pr_mut(number)?;
        *self.next_comment += 1;
        let id = self.next_comment.to_string();
        self.repo
            .comments
            .insert(id.clone(), (number, body.to_owned()));

        self.emit(ScmEvent::Commented {
            repo_external_id: self.repo_external_id.clone(),
            number,
            comment_external_id: id.clone(),
            author: author.clone(),
            body: body.to_owned(),
        });

        Ok(id)
    }

    fn land(
        &mut self,
        branch: &str,
        theirs: &str,
        method: MergeMethod,
    ) -> Result<String, ScmError> {
        let ours = self.repo.branch_head(branch)?;
        let sha = match method {
            MergeMethod::FastForward => {
                if !self.repo.is_ancestor(&ours, theirs) {
                    return Err(ScmError::NotFastForward);
                }
                theirs.to_owned()
            }
            MergeMethod::MergeCommit | MergeMethod::Squash | MergeMethod::Rebase => {
                let tree = self
                    .repo
                    .merged_tree(&ours, theirs)?
                    .ok_or_else(|| ScmError::MergeRejected("merge conflict".into()))?;
                let parents = if method == MergeMethod::MergeCommit {
                    vec![ours, theirs.to_owned()]
                } else {
                    vec![ours]
                };
                self.new_commit(parents, tree)
            }
        };
        self.move_branch(branch, Some(&sha), &bot());
        Ok(sha)
    }
}

impl Repo {
    fn new_commit(&mut self, next_sha: &mut u64, parents: Vec<String>, tree: Tree) -> String {
        *next_sha += 1;
        let sha = format!("{next_sha:040x}");
        self.commits.insert(sha.clone(), Commit { parents, tree });
        sha
    }

    fn tree(&self, sha: &str) -> Result<&Tree, ScmError> {
        self.commits
            .get(sha)
            .map(|commit| &commit.tree)
            .ok_or_else(|| ScmError::NotFound(format!("commit {sha}")))
    }

    fn branch_head(&self, branch: &str) -> Result<String, ScmError> {
        self.branches
            .get(branch)
            .cloned()
            .ok_or_else(|| ScmError::NotFound(format!("branch {branch}")))
    }

    fn pr(&self, number: u64) -> Result<&Pr, ScmError> {
        self.prs
            .get(&number)
            .ok_or_else(|| ScmError::NotFound(format!("PR {number}")))
    }

    fn pr_mut(&mut self, number: u64) -> Result<&mut Pr, ScmError> {
        self.prs
            .get_mut(&number)
            .ok_or_else(|| ScmError::NotFound(format!("PR {number}")))
    }

    fn ancestors(&self, sha: &str) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut next = vec![sha.to_owned()];
        while let Some(sha) = next.pop() {
            if let Some(commit) = self.commits.get(&sha) {
                next.extend(commit.parents.iter().cloned());
            }
            seen.insert(sha);
        }

        seen
    }

    fn is_ancestor(&self, ancestor: &str, sha: &str) -> bool {
        self.ancestors(sha).contains(ancestor)
    }

    fn merge_base(&self, a: &str, b: &str) -> Option<String> {
        let common: BTreeSet<String> = self
            .ancestors(a)
            .intersection(&self.ancestors(b))
            .cloned()
            .collect();
        common
            .iter()
            .find(|candidate| {
                common
                    .iter()
                    .all(|other| other == *candidate || !self.is_ancestor(candidate, other))
            })
            .cloned()
    }

    fn merged_tree(&self, ours: &str, theirs: &str) -> Result<Option<Tree>, ScmError> {
        let empty = Tree::new();
        let base = match self.merge_base(ours, theirs) {
            Some(base) => self.tree(&base)?,
            None => &empty,
        };
        let (ours, theirs) = (self.tree(ours)?, self.tree(theirs)?);
        let paths: BTreeSet<&String> = base
            .keys()
            .chain(ours.keys())
            .chain(theirs.keys())
            .collect();
        let mut merged = Tree::new();
        for path in paths {
            let (o, a, b) = (base.get(path), ours.get(path), theirs.get(path));
            let content = if a == b || b == o {
                a
            } else if a == o {
                b
            } else {
                return Ok(None);
            };
            if let Some(content) = content {
                merged.insert(path.clone(), content.clone());
            }
        }

        Ok(Some(merged))
    }

    fn head(&self, pr: &Pr) -> String {
        self.branches
            .get(&pr.head_branch)
            .cloned()
            .unwrap_or_else(|| pr.last_head.clone())
    }

    fn mergeability(&self, pr: &Pr, head: &str) -> Result<Mergeability, ScmError> {
        if let Some(forced) = pr.mergeability {
            return Ok(forced);
        }

        let base = self.branch_head(&pr.base_branch)?;
        if self.merged_tree(&base, head)?.is_none() {
            return Ok(Mergeability::Conflict);
        }

        let rules = self.rules.get(&pr.base_branch).cloned().unwrap_or_default();
        if rules.up_to_date && !self.is_ancestor(&base, head) {
            return Ok(Mergeability::Behind);
        }

        let checks = self.checks.get(head).map(Vec::as_slice).unwrap_or_default();
        let checks_pass = rules.names.iter().all(|name| {
            checks.iter().any(|check| {
                check.name == *name && check.conclusion.is_some_and(CheckConclusion::is_passing)
            })
        });
        let reviewed = !matches!(
            pr.review_decision,
            ReviewDecision::ChangesRequested | ReviewDecision::ReviewRequired
        );
        Ok(if pr.draft || !checks_pass || !reviewed {
            Mergeability::Blocked
        } else {
            Mergeability::Mergeable
        })
    }

    fn changes(&self, pr: &Pr, head: &str) -> Result<BTreeMap<String, bool>, ScmError> {
        let base = self.branch_head(&pr.base_branch)?;
        let (base_tree, head_tree) = match self.merge_base(&base, head) {
            Some(fork) => (self.tree(&fork)?, self.tree(head)?),
            None => (self.tree(head)?, self.tree(head)?),
        };
        Ok(base_tree
            .keys()
            .chain(head_tree.keys())
            .filter(|path| base_tree.get(*path) != head_tree.get(*path))
            .map(|path| (path.clone(), !head_tree.contains_key(path)))
            .collect())
    }

    fn changed_files(&self, number: u64) -> Result<Vec<String>, ScmError> {
        let pr = self.pr(number)?;
        Ok(self.changes(pr, &self.head(pr))?.into_keys().collect())
    }

    fn snapshot(&self, number: u64) -> Result<PrSnapshot, ScmError> {
        let pr = self.pr(number)?;
        let head = self.head(pr);
        let changes = self.changes(pr, &head)?;
        let deletions = changes.values().filter(|deleted| **deleted).count();
        let mergeability = if pr.state == PrState::Open {
            self.mergeability(pr, &head)?
        } else {
            Mergeability::Blocked
        };
        Ok(PrSnapshot {
            number,
            state: pr.state,
            head_sha: head.clone(),
            base_branch: pr.base_branch.clone(),
            head_branch: pr.head_branch.clone(),
            from_fork: false,
            created_at: OffsetDateTime::UNIX_EPOCH,
            draft: pr.draft,
            title: pr.title.clone(),
            body: pr.body.clone(),
            labels: pr.labels.clone(),
            mergeability,
            review_decision: pr.review_decision,
            approved_by: pr.reviews.approved_by.clone(),
            changes_requested_by: pr.reviews.changes_requested_by.clone(),
            review_requested: pr.reviews.requested.clone(),
            checks: self.checks.get(&head).cloned().unwrap_or_default(),
            author: pr.author.clone(),
            additions: (changes.len() - deletions) as u64,
            deletions: deletions as u64,
            changed_files: changes.len() as u64,
        })
    }
}

impl Scm for Sim {
    fn pull_requests<'a>(
        &'a self,
        repo: &'a RepoRef,
        numbers: &'a [u64],
        class: JobClass,
    ) -> ScmFuture<'a, Vec<PrSnapshot>> {
        let numbers = numbers.to_vec();
        self.call("pull_requests", repo, class, move |ctx| {
            if numbers.len() > crate::MAX_PRS_PER_READ {
                return Err(ScmError::Validation("too many PRs in one read".into()));
            }
            numbers
                .into_iter()
                .filter(|number| ctx.repo.prs.contains_key(number))
                .map(|number| ctx.repo.snapshot(number))
                .collect()
        })
    }

    fn branch_head<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, Option<String>> {
        let branch = branch.to_owned();
        self.call("branch_head", repo, class, move |ctx| {
            Ok(ctx.repo.branches.get(&branch).cloned())
        })
    }

    fn checks<'a>(
        &'a self,
        repo: &'a RepoRef,
        sha: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<CheckReport>> {
        let sha = sha.to_owned();
        self.call("checks", repo, class, move |ctx| {
            Ok(ctx.repo.checks.get(&sha).cloned().unwrap_or_default())
        })
    }

    fn required_checks<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, RequiredChecks> {
        let branch = branch.to_owned();
        self.call("required_checks", repo, class, move |ctx| {
            ctx.repo.branch_head(&branch)?;
            Ok(ctx.repo.rules.get(&branch).cloned().unwrap_or_default())
        })
    }

    fn file<'a>(
        &'a self,
        repo: &'a RepoRef,
        git_ref: &'a str,
        path: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, Option<String>> {
        let (git_ref, path) = (git_ref.to_owned(), path.to_owned());
        self.call("file", repo, class, move |ctx| {
            let sha = ctx.repo.branches.get(&git_ref).cloned().unwrap_or(git_ref);
            Ok(ctx.repo.tree(&sha)?.get(&path).cloned())
        })
    }

    fn permission<'a>(
        &'a self,
        repo: &'a RepoRef,
        user: &'a ScmUser,
        class: JobClass,
    ) -> ScmFuture<'a, Permission> {
        let user = user.external_id.clone();
        self.call("permission", repo, class, move |ctx| {
            Ok(ctx
                .repo
                .permissions
                .get(&user)
                .copied()
                .unwrap_or(Permission::None))
        })
    }

    fn is_ancestor<'a>(
        &'a self,
        repo: &'a RepoRef,
        ancestor: &'a str,
        descendant: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, bool> {
        self.call("is_ancestor", repo, class, move |ctx| {
            for sha in [ancestor, descendant] {
                ctx.repo.tree(sha)?;
            }
            Ok(ctx.repo.is_ancestor(ancestor, descendant))
        })
    }

    fn pr_files<'a>(
        &'a self,
        repo: &'a RepoRef,
        number: u64,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<String>> {
        self.call("pr_files", repo, class, move |ctx| {
            ctx.repo.changed_files(number)
        })
    }

    fn unresolved_threads<'a>(
        &'a self,
        repo: &'a RepoRef,
        number: u64,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<String>> {
        self.call("unresolved_threads", repo, class, move |ctx| {
            Ok(ctx.repo.pr(number)?.unresolved_threads.clone())
        })
    }

    fn set_branch<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        sha: &'a str,
        force: bool,
        class: JobClass,
    ) -> ScmFuture<'a, ()> {
        let (branch, sha) = (branch.to_owned(), sha.to_owned());
        self.call("set_branch", repo, class, move |ctx| {
            ctx.repo
                .tree(&sha)
                .map_err(|_| ScmError::Validation(format!("unknown commit {sha}")))?;
            if let Some(before) = ctx.repo.branches.get(&branch)
                && !force
                && !ctx.repo.is_ancestor(before, &sha)
            {
                return Err(ScmError::Validation("not a fast forward".into()));
            }

            ctx.move_branch(&branch, Some(&sha), &bot());
            Ok(())
        })
    }

    fn delete_branch<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, ()> {
        let branch = branch.to_owned();
        self.call("delete_branch", repo, class, move |ctx| {
            ctx.move_branch(&branch, None, &bot());
            Ok(())
        })
    }

    fn merge_into_branch<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        sha: &'a str,
        _message: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, MergeIntoRef> {
        let (branch, sha) = (branch.to_owned(), sha.to_owned());
        self.call("merge_into_branch", repo, class, move |ctx| {
            let ours = ctx.repo.branch_head(&branch)?;
            ctx.repo.tree(&sha)?;
            if ctx.repo.is_ancestor(&sha, &ours) {
                return Ok(MergeIntoRef::AlreadyContained);
            }

            let Some(tree) = ctx.repo.merged_tree(&ours, &sha)? else {
                return Ok(MergeIntoRef::Conflict);
            };

            let merged = ctx.new_commit(vec![ours, sha], tree);
            ctx.move_branch(&branch, Some(&merged), &bot());

            Ok(MergeIntoRef::Merged { sha: merged })
        })
    }

    fn merge_pr<'a>(
        &'a self,
        repo: &'a RepoRef,
        merge: &'a PrMerge,
        class: JobClass,
    ) -> ScmFuture<'a, String> {
        let (number, expected, method) = (merge.number, merge.head_sha.clone(), merge.method);
        let tested = merge.tested_sha.clone();
        self.call("merge_pr", repo, class, move |ctx| {
            let pr = ctx.repo.pr_mut(number)?;
            if let Some(merged) = &pr.merged_sha {
                return Ok(merged.clone());
            }

            if pr.state != PrState::Open {
                return Err(ScmError::MergeRejected("PR closed".into()));
            }

            let pr = &ctx.repo.prs[&number];
            let head = ctx.repo.head(pr);
            if head != expected {
                return Err(ScmError::HeadChanged);
            }

            match ctx.repo.mergeability(pr, &head)? {
                Mergeability::Mergeable => {}
                Mergeability::Behind => {
                    return Err(ScmError::Forbidden("the head must be up to date".into()));
                }
                other => return Err(ScmError::MergeRejected(format!("PR {other}"))),
            }

            let base = pr.base_branch.clone();
            let theirs = if method == MergeMethod::FastForward {
                &tested
            } else {
                &head
            };
            let merged = ctx.land(&base, theirs, method)?;
            let pr = ctx.repo.pr_mut(number)?;
            pr.state = PrState::Merged;
            pr.last_head = head;
            pr.merged_sha = Some(merged.clone());
            ctx.pr_changed(number);

            Ok(merged)
        })
    }

    fn set_status<'a>(
        &'a self,
        repo: &'a RepoRef,
        status: &'a StatusUpdate,
        class: JobClass,
    ) -> ScmFuture<'a, ()> {
        let status = status.clone();
        self.call("set_status", repo, class, move |ctx| {
            ctx.repo.tree(&status.sha)?;
            let conclusion = match status.status {
                CommitStatus::Pending => None,
                CommitStatus::Success => Some(CheckConclusion::Success),
                CommitStatus::Failure => Some(CheckConclusion::Failure),
                CommitStatus::Error => Some(CheckConclusion::Error),
            };
            let check = CheckReport {
                name: status.context,
                kind: CheckKind::Status,
                conclusion,
                details_url: status.details_url,
            };
            let unchanged = ctx
                .repo
                .checks
                .get(&status.sha)
                .is_some_and(|reports| reports.contains(&check));
            if !unchanged {
                ctx.report(&status.sha, check);
            }

            Ok(())
        })
    }

    fn create_comment<'a>(
        &'a self,
        repo: &'a RepoRef,
        number: u64,
        body: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, String> {
        let body = body.to_owned();
        self.call("create_comment", repo, class, move |ctx| {
            ctx.add_comment(number, &bot(), &body)
        })
    }

    fn update_comment<'a>(
        &'a self,
        repo: &'a RepoRef,
        comment_id: &'a str,
        body: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, ()> {
        let (comment_id, body) = (comment_id.to_owned(), body.to_owned());
        self.call("update_comment", repo, class, move |ctx| {
            let comment = ctx
                .repo
                .comments
                .get_mut(&comment_id)
                .ok_or_else(|| ScmError::NotFound(format!("comment {comment_id}")))?;
            comment.1 = body;
            Ok(())
        })
    }

    fn acknowledge_comment<'a>(
        &'a self,
        repo: &'a RepoRef,
        comment_id: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, ()> {
        let comment_id = comment_id.to_owned();
        self.call("acknowledge_comment", repo, class, move |ctx| {
            if !ctx.repo.comments.contains_key(&comment_id) {
                return Err(ScmError::NotFound(format!("comment {comment_id}")));
            }

            ctx.repo.acknowledged.insert(comment_id);
            Ok(())
        })
    }

    fn add_label<'a>(
        &'a self,
        repo: &'a RepoRef,
        number: u64,
        label: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, ()> {
        let label = label.to_owned();
        self.call("add_label", repo, class, move |ctx| {
            if ctx.repo.pr_mut(number)?.labels.insert(label.clone()) {
                ctx.pr_event(number, Some(label), false);
            }
            Ok(())
        })
    }

    fn remove_label<'a>(
        &'a self,
        repo: &'a RepoRef,
        number: u64,
        label: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, ()> {
        let label = label.to_owned();
        self.call("remove_label", repo, class, move |ctx| {
            if ctx.repo.pr_mut(number)?.labels.remove(&label) {
                ctx.pr_changed(number);
            }
            Ok(())
        })
    }

    fn open_draft_pr<'a>(
        &'a self,
        repo: &'a RepoRef,
        head: &'a str,
        base: &'a str,
        title: &'a str,
        body: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, u64> {
        let (head, base) = (head.to_owned(), base.to_owned());
        let (title, body) = (title.to_owned(), body.to_owned());
        self.call("open_draft_pr", repo, class, move |ctx| {
            let open = ctx.repo.prs.iter().find(|(_, pr)| {
                pr.state == PrState::Open && pr.head_branch == head && pr.base_branch == base
            });
            match open {
                Some((number, _)) => Ok(*number),
                None => ctx.new_pr(&head, &base, &bot(), (&title, &body), true),
            }
        })
    }

    fn close_pr<'a>(
        &'a self,
        repo: &'a RepoRef,
        number: u64,
        class: JobClass,
    ) -> ScmFuture<'a, ()> {
        self.call("close_pr", repo, class, move |ctx| {
            let pr = ctx.repo.pr_mut(number)?;
            if pr.state == PrState::Open {
                pr.state = PrState::Closed;
                ctx.pr_changed(number);
            }
            Ok(())
        })
    }

    fn recent_deliveries(
        &self,
        _since: OffsetDateTime,
        class: JobClass,
    ) -> ScmFuture<'_, Vec<RecentDelivery>> {
        self.record_call("recent_deliveries", class, |state| {
            Ok(state
                .attempts
                .iter()
                .enumerate()
                .map(|(n, attempt)| RecentDelivery {
                    delivery_id: attempt.delivery_id.clone(),
                    attempt_id: (n + 1).to_string(),
                    is_delivered: attempt.is_delivered,
                })
                .collect())
        })
    }

    fn redeliver<'a>(&'a self, attempt_id: &'a str, class: JobClass) -> ScmFuture<'a, ()> {
        self.record_call("redeliver", class, move |state| {
            let attempt = attempt_id
                .parse::<usize>()
                .ok()
                .and_then(|n| n.checked_sub(1))
                .filter(|n| *n < state.attempts.len())
                .ok_or_else(|| ScmError::NotFound(format!("attempt {attempt_id}")))?;
            state.redelivered.push(attempt);
            Ok(())
        })
    }

    fn installation<'a>(
        &'a self,
        external_id: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, Option<ScmInstallation>> {
        self.record_call("installation", class, move |state| {
            Ok(state.installation(external_id))
        })
    }

    fn installations(&self, class: JobClass) -> ScmFuture<'_, Vec<ScmInstallation>> {
        self.record_call("installations", class, |state| {
            Ok(state
                .installations
                .keys()
                .filter_map(|external_id| state.installation(external_id))
                .collect())
        })
    }

    fn check_setup<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<SetupFinding>> {
        let branch = branch.to_owned();
        self.call("check_setup", repo, class, move |ctx| {
            ctx.repo.branch_head(&branch)?;
            let mut findings = vec![SetupFinding::UpdateRestrictionMissing];
            if ctx
                .repo
                .rules
                .get(&branch)
                .is_some_and(|rules| rules.up_to_date)
            {
                findings.push(SetupFinding::UpToDateRequired);
            }

            Ok(findings)
        })
    }

    fn list_repos<'a>(
        &'a self,
        connection: &'a ConnectionRef,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<ScmRepo>> {
        self.record_call("list_repos", class, move |state| {
            if state.lost.contains(&connection.external_id) {
                return Err(ScmError::AccessLost("connection suspended".into()));
            }
            Ok(state
                .repos
                .iter()
                .filter(|(_, repo)| repo.connection == connection.external_id)
                .map(|(external_id, repo)| ScmRepo {
                    external_id: external_id.clone(),
                    namespace: repo.namespace.clone(),
                    name: repo.name.clone(),
                    default_branch: repo.default_branch.clone(),
                })
                .collect())
        })
    }

    fn app_install_url(&self, class: JobClass) -> ScmFuture<'_, String> {
        self.record_call("app_install_url", class, |_| Ok("sim:install".to_owned()))
    }

    fn sign_in_url(&self, request: &SignInRequest) -> Option<String> {
        let page = self
            .with(|state| state.sign_in_page.clone())
            .unwrap_or_else(|| "sim:authorize".to_owned());
        Some(format!(
            "{page}?redirect_uri={}&state={}&code_challenge={}",
            encode_query(&request.redirect_url),
            encode_query(&request.state),
            encode_query(&request.code_challenge)
        ))
    }

    fn sign_in<'a>(&'a self, code: &'a SignInCode, class: JobClass) -> ScmFuture<'a, SignedIn> {
        self.record_call("sign_in", class, move |state| {
            let refused = || ScmError::Validation("the code of the sign-in is not valid".into());
            let (user, request) = state
                .sign_in_codes
                .remove(code.code.expose_secret())
                .ok_or_else(refused)?;
            let is_same_request = request.redirect_url == code.redirect_url
                && request.code_challenge == code_challenge(&code.code_verifier);
            if !is_same_request {
                return Err(refused());
            }

            let memberships = state
                .members
                .iter()
                .filter(|(installation, _)| state.installations.contains_key(*installation))
                .filter_map(|(installation, members)| {
                    members.get(&user.external_id).map(|member| ScmMembership {
                        connection_external_id: installation.clone(),
                        role: member.role,
                    })
                })
                .collect();
            Ok(SignedIn { user, memberships })
        })
    }

    fn members<'a>(
        &'a self,
        connection: &'a ConnectionRef,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<ScmMember>> {
        self.record_call("members", class, move |state| {
            if state.lost.contains(&connection.external_id) {
                return Err(ScmError::AccessLost("connection suspended".into()));
            }
            Ok(state
                .members
                .get(&connection.external_id)
                .map(|members| members.values().cloned().collect())
                .unwrap_or_default())
        })
    }
}

/// Builds in a new simulator the world of the contract suite, on the
/// repository `repo`.
///
/// # Errors
///
/// None in practice: the world only uses what it created.
pub fn contract_world(repo: &RepoRef) -> Result<(Sim, Fixture), ScmError> {
    let sim = Sim::new();
    let author = ScmUser {
        external_id: "2".into(),
        login: "author".into(),
    };
    let root = sim.add_repo(
        repo,
        "main",
        &[(".cihub.yaml", "version: 1\n"), ("a.txt", "a\n")],
    );

    let conflicting = sim.commit(repo, &root, &[("a.txt", Some("x\n"))])?;
    sim.push(repo, "conflicting", &conflicting, &author)?;
    let conflicting_pr = sim.open_pr(repo, "conflicting", "main", &author)?;
    let main = sim.commit(repo, &root, &[("a.txt", Some("y\n"))])?;
    sim.push(repo, "main", &main, &author)?;

    let feature = sim.commit(repo, &main, &[("b.txt", Some("b\n"))])?;
    sim.push(repo, "feature", &feature, &author)?;
    let mergeable_pr = sim.open_pr(repo, "feature", "main", &author)?;
    sim.change_pr(
        repo,
        mergeable_pr,
        PrChange::Review(ReviewDecision::Approved),
    )?;

    let reviews = Reviews {
        approved_by: BTreeSet::from(["reviewer".to_owned()]),
        ..Reviews::default()
    };
    sim.set_reviews(repo, mergeable_pr, reviews)?;
    let ci = CheckReport {
        name: "ci".into(),
        kind: CheckKind::Check,
        conclusion: Some(CheckConclusion::Success),
        details_url: None,
    };
    sim.report_check(repo, &feature, ci)?;

    let draft = sim.commit(repo, &main, &[("c.txt", Some("c\n"))])?;
    sim.push(repo, "draft", &draft, &author)?;
    let draft_pr = sim.open_pr(repo, "draft", "main", &author)?;
    sim.change_pr(repo, draft_pr, PrChange::Draft(true))?;

    let required = RequiredChecks {
        names: BTreeSet::from(["ci".to_owned()]),
        up_to_date: false,
    };
    sim.set_rules(repo, "main", required.clone())?;
    let admin = ScmUser {
        external_id: "3".into(),
        login: "admin".into(),
    };
    sim.set_permission(repo, &admin, Permission::Admin)?;
    let account = ScmAccount {
        external_id: "10".into(),
        path: repo.namespace.clone(),
    };
    sim.add_installation(
        &repo.connection.external_id,
        account,
        &[("contents", "write"), ("metadata", "read")],
    );
    sim.set_member(&repo.connection.external_id, &admin, MembershipRole::Admin);

    sim.take_deliveries();
    let fixture = Fixture {
        base_branch: "main".into(),
        required,
        file: (".cihub.yaml".into(), "version: 1\n".into()),
        mergeable_pr,
        conflicting_pr,
        draft_pr,
        missing_pr: 999,
        admin,
        work_branch: "cihub-temp/contract".into(),
    };
    Ok((sim, fixture))
}

#[cfg(test)]
mod tests {
    use cihub_domain::{CheckReport, ConnectionId};
    use ulid::Ulid;

    use super::*;
    use crate::ConnectionRef;

    fn repo() -> RepoRef {
        RepoRef {
            connection: ConnectionRef {
                id: ConnectionId::from_ulid(Ulid(1)),
                external_id: "100".into(),
            },
            external_id: "200".into(),
            namespace: "acme".into(),
            name: "widgets".into(),
        }
    }

    fn success(name: &str) -> CheckReport {
        CheckReport {
            name: name.into(),
            kind: CheckKind::Check,
            conclusion: Some(CheckConclusion::Success),
            details_url: None,
        }
    }

    #[tokio::test]
    async fn the_simulator_passes_the_contract() {
        let repo = repo();
        let (sim, fixture) = contract_world(&repo).unwrap();
        crate::contract::run(&sim, &repo, &fixture).await;
        crate::contract::members(&sim, &repo, &fixture).await;
        crate::contract::install_page(&sim).await;
    }

    #[tokio::test]
    async fn a_sign_in_code_gives_the_roles_of_the_person_once() {
        let repo = repo();
        let (sim, _) = contract_world(&repo).unwrap();
        let person = ScmUser {
            external_id: "7".into(),
            login: "ada".into(),
        };
        let request = SignInRequest {
            redirect_url: "https://ci.example.com/auth/callback".into(),
            state: "state".into(),
            code_challenge: code_challenge(&"verifier".into()),
        };
        let exchange = |code: String, verifier: &str, redirect_url: &str| SignInCode {
            code: code.into(),
            code_verifier: verifier.into(),
            redirect_url: redirect_url.into(),
        };
        let member = ScmMembership {
            connection_external_id: "100".into(),
            role: MembershipRole::Member,
        };
        let url = request.redirect_url.as_str();

        sim.set_member("100", &person, MembershipRole::Member);
        sim.set_member("999", &person, MembershipRole::Admin);
        let code = sim.authorize(&person, &request);
        let first = exchange(code.clone(), "verifier", url);
        let replayed = exchange(code, "verifier", url);
        let elsewhere = exchange(
            sim.authorize(&person, &request),
            "verifier",
            "https://evil.example",
        );
        let forged = exchange(sim.authorize(&person, &request), "other", url);
        let signed_in = sim.sign_in(&first, JobClass::Interactive).await.unwrap();
        let mut refused = Vec::new();
        for code in [&replayed, &elsewhere, &forged] {
            refused.push(sim.sign_in(code, JobClass::Interactive).await);
        }

        assert_eq!(signed_in.user, person);
        assert_eq!(signed_in.memberships, [member]);
        for refused in refused {
            assert!(
                matches!(refused, Err(ScmError::Validation(_))),
                "{refused:?}"
            );
        }
    }

    #[test]
    fn a_code_challenge_is_the_one_of_rfc_7636() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".into();

        assert_eq!(
            code_challenge(&verifier),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[tokio::test]
    async fn a_member_who_leaves_the_account_is_no_longer_listed() {
        let repo = repo();
        let (sim, fixture) = contract_world(&repo).unwrap();

        sim.remove_member("100", &fixture.admin);

        let members = sim
            .members(&repo.connection, JobClass::Repair)
            .await
            .unwrap();

        assert!(members.is_empty());

        sim.set_access("100", false);
        let lost = sim.members(&repo.connection, JobClass::Repair).await;

        assert!(matches!(lost, Err(ScmError::AccessLost(_))));
    }

    #[tokio::test]
    async fn a_fault_fails_its_calls_then_stops() {
        let repo = repo();
        let (sim, fixture) = contract_world(&repo).unwrap();
        let until = time::OffsetDateTime::UNIX_EPOCH;
        sim.fail(Fault {
            call: Some("pull_requests"),
            error: ScmError::RateLimited { until },
            times: 2,
        });

        let number = fixture.mergeable_pr;
        let branch = sim.branch_head(&repo, "main", JobClass::Repair).await;

        assert!(branch.unwrap().is_some(), "another method is not failed");
        for _ in 0..2 {
            let failed = sim.pull_request(&repo, number, JobClass::Merge).await;
            assert_eq!(failed, Err(ScmError::RateLimited { until }));
        }

        assert!(
            sim.pull_request(&repo, number, JobClass::Merge)
                .await
                .is_ok()
        );

        let calls = sim.calls();

        assert_eq!(calls.len(), 4);
        assert_eq!(
            calls[0],
            SimCall {
                name: "branch_head",
                class: JobClass::Repair
            }
        );
    }

    #[tokio::test]
    async fn a_suspended_connection_loses_its_access() {
        let repo = repo();
        let (sim, _) = contract_world(&repo).unwrap();
        sim.set_access("100", false);
        let lost = sim.branch_head(&repo, "main", JobClass::Repair).await;

        assert!(matches!(lost, Err(ScmError::AccessLost(_))), "{lost:?}");
        let suspended = sim.installation("100", JobClass::Repair).await.unwrap();

        assert!(suspended.is_some_and(|installation| installation.is_suspended));

        sim.set_access("100", true);

        assert!(
            sim.branch_head(&repo, "main", JobClass::Repair)
                .await
                .is_ok()
        );

        let events: Vec<ScmEvent> = sim.take_deliveries().into_iter().map(|d| d.event).collect();

        assert_eq!(
            events,
            [ScmEvent::ConnectionChanged, ScmEvent::ConnectionChanged]
        );
    }

    #[tokio::test]
    async fn an_installation_changes_its_permissions_then_disappears() {
        let repo = repo();
        let (sim, _) = contract_world(&repo).unwrap();

        sim.set_permissions("100", &[("contents", "read")]);
        let installation = sim.installation("100", JobClass::Repair).await.unwrap();
        let permissions = installation.map(|installation| installation.permissions);
        let expected = BTreeMap::from([("contents".to_owned(), "read".to_owned())]);

        assert_eq!(permissions, Some(expected));

        sim.remove_installation("100");

        assert_eq!(sim.installation("100", JobClass::Repair).await, Ok(None));
        assert_eq!(sim.installations(JobClass::Repair).await, Ok(Vec::new()));
        let lost = sim.branch_head(&repo, "main", JobClass::Repair).await;

        assert!(matches!(lost, Err(ScmError::AccessLost(_))), "{lost:?}");
        let events: Vec<ScmEvent> = sim.take_deliveries().into_iter().map(|d| d.event).collect();

        assert_eq!(
            events,
            [ScmEvent::ConnectionChanged, ScmEvent::ConnectionChanged]
        );
    }

    #[test]
    fn changes_send_their_deliveries() {
        let repo = repo();
        let sim = Sim::new();
        let author = bot();
        let root = sim.add_repo(&repo, "main", &[("a.txt", "a")]);
        let head = sim.commit(&repo, &root, &[("b.txt", Some("b"))]).unwrap();
        sim.push(&repo, "topic", &head, &author).unwrap();
        let number = sim.open_pr(&repo, "topic", "main", &author).unwrap();
        sim.report_check(&repo, &head, success("ci")).unwrap();
        let events: Vec<ScmEvent> = sim.take_deliveries().into_iter().map(|d| d.event).collect();

        assert_eq!(
            events,
            [
                ScmEvent::Pushed {
                    repo_external_id: "200".into(),
                    branch: "topic".into(),
                    before: None,
                    after: Some(head.clone()),
                    sender: author,
                },
                ScmEvent::PrChanged {
                    repo_external_id: "200".into(),
                    number,
                    base_branch: "main".into(),
                    label_added: None,
                    opened: true,
                },
                ScmEvent::CheckChanged {
                    repo_external_id: "200".into(),
                    sha: head,
                    branches: vec!["topic".into()],
                    check: success("ci"),
                },
            ]
        );
    }

    #[tokio::test]
    async fn a_pr_behind_its_base_waits_when_the_rules_say_so() {
        let repo = repo();
        let sim = Sim::new();
        let author = bot();
        let root = sim.add_repo(&repo, "main", &[("a.txt", "a")]);
        let head = sim.commit(&repo, &root, &[("b.txt", Some("b"))]).unwrap();
        sim.push(&repo, "topic", &head, &author).unwrap();
        let number = sim.open_pr(&repo, "topic", "main", &author).unwrap();
        let main = sim.commit(&repo, &root, &[("c.txt", Some("c"))]).unwrap();
        sim.push(&repo, "main", &main, &author).unwrap();
        let read = async || {
            sim.pull_request(&repo, number, JobClass::Repair)
                .await
                .unwrap()
                .mergeability
        };

        assert_eq!(read().await, Mergeability::Mergeable);
        let rules = RequiredChecks {
            names: BTreeSet::new(),
            up_to_date: true,
        };
        sim.set_rules(&repo, "main", rules).unwrap();

        assert_eq!(read().await, Mergeability::Behind);
        sim.change_pr(
            &repo,
            number,
            PrChange::Mergeability(Some(Mergeability::Unknown)),
        )
        .unwrap();

        assert_eq!(read().await, Mergeability::Unknown);
    }

    fn deliveries(count: u64) -> Sim {
        let repo = repo();
        let sim = Sim::new();
        let root = sim.add_repo(&repo, "main", &[]);
        for n in 0..count {
            sim.push(&repo, &format!("b{n}"), &root, &bot()).unwrap();
        }

        sim
    }

    #[tokio::test]
    async fn a_lost_delivery_is_listed_and_sent_again_with_its_identifier() {
        let sim = deliveries(3);
        let lose_all = &mut WebhookFaults {
            seed: 1,
            lose_percent: 100,
            duplicate_percent: 0,
            reorder: false,
        };
        let sent = deliveries(3).take_deliveries();

        assert_eq!(sim.take_deliveries_with(lose_all), []);
        let since = OffsetDateTime::UNIX_EPOCH;
        let recent = sim
            .recent_deliveries(since, JobClass::Repair)
            .await
            .unwrap();
        let ids: Vec<_> = recent.iter().map(|r| r.delivery_id.as_str()).collect();

        assert_eq!(ids, ["sim-1", "sim-2", "sim-3"]);
        assert!(recent.iter().all(|r| !r.is_delivered));
        sim.redeliver("2", JobClass::Repair).await.unwrap();

        assert_eq!(sim.take_deliveries(), [sent[1].clone()]);
        let recent = sim
            .recent_deliveries(since, JobClass::Repair)
            .await
            .unwrap();

        assert_eq!(
            recent[3],
            RecentDelivery {
                delivery_id: "sim-2".into(),
                attempt_id: "4".into(),
                is_delivered: true,
            }
        );

        let missing = sim.redeliver("9", JobClass::Repair).await;

        assert!(matches!(missing, Err(ScmError::NotFound(_))));
    }

    #[test]
    fn webhook_faults_lose_duplicate_and_reorder() {
        let faults = |lose_percent, duplicate_percent, reorder| WebhookFaults {
            seed: 7,
            lose_percent,
            duplicate_percent,
            reorder,
        };
        let clean = deliveries(20).take_deliveries();

        assert_eq!(clean.len(), 20);
        assert_eq!(
            deliveries(20).take_deliveries_with(&mut faults(100, 0, false)),
            []
        );

        let twice = deliveries(20).take_deliveries_with(&mut faults(0, 100, false));

        assert_eq!(twice.len(), 40);
        assert_eq!(twice[0], twice[1]);

        let shuffled = deliveries(20).take_deliveries_with(&mut faults(0, 0, true));

        assert_ne!(shuffled, clean);
        let mut sorted = shuffled.clone();
        sorted.sort_by_key(|d| format!("{d:?}"));
        let mut expected = clean.clone();
        expected.sort_by_key(|d| format!("{d:?}"));

        assert_eq!(sorted, expected, "a reorder keeps every delivery");
        let again = deliveries(20).take_deliveries_with(&mut faults(0, 0, true));

        assert_eq!(again, shuffled, "the same seed gives the same order");

        let some = deliveries(100).take_deliveries_with(&mut faults(30, 0, false));

        assert!((40..100).contains(&some.len()), "{}", some.len());
    }

    async fn outcomes(
        repo: &RepoRef,
        faults: RandomFaults,
    ) -> Vec<Result<Option<String>, ScmError>> {
        let sim = Sim::new();
        sim.add_repo(repo, "main", &[("a.txt", "a")]);
        sim.fail_at_random(Some(faults));
        let mut outcomes = Vec::new();
        for _ in 0..200 {
            outcomes.push(sim.branch_head(repo, "main", JobClass::Repair).await);
        }

        outcomes
    }

    #[tokio::test]
    async fn random_faults_fail_a_share_of_the_calls_after_the_given_ones() {
        let repo = repo();
        let sim = Sim::new();
        sim.add_repo(&repo, "main", &[("a.txt", "a")]);
        let until = OffsetDateTime::UNIX_EPOCH;
        let random = |seed| RandomFaults {
            seed,
            unavailable_percent: 20,
            rate_limited_percent: 10,
            rate_limited_until: until,
        };
        let first = outcomes(&repo, random(3)).await;

        assert_eq!(
            first,
            outcomes(&repo, random(3)).await,
            "the same seed fails the same calls"
        );

        let unavailable = first
            .iter()
            .filter(|o| matches!(o, Err(ScmError::Unavailable(_))))
            .count();
        let limited = first
            .iter()
            .filter(|o| *o == &Err(ScmError::RateLimited { until }))
            .count();

        assert!((20..60).contains(&unavailable), "{unavailable}");
        assert!((5..40).contains(&limited), "{limited}");

        sim.fail_at_random(Some(RandomFaults {
            unavailable_percent: 0,
            rate_limited_percent: 0,
            ..random(1)
        }));

        sim.fail(Fault {
            call: None,
            error: ScmError::HeadChanged,
            times: 1,
        });

        let given = sim.branch_head(&repo, "main", JobClass::Repair).await;

        assert_eq!(given, Err(ScmError::HeadChanged));
        assert!(
            sim.branch_head(&repo, "main", JobClass::Repair)
                .await
                .is_ok()
        );

        sim.fail_at_random(None);
    }
}

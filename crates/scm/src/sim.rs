//! An SCM in memory, for the tests of CIHub (`AGENTS.md` D73). It keeps
//! repositories with commits, branches, PRs, checks, comments and rules; it
//! records each call; it can fail calls and lose, duplicate or reorder its
//! webhook deliveries. It has no clock: a test gives every time.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::sync::{Mutex, PoisonError};

use cihub_domain::{
    CheckConclusion, CheckKind, CheckReport, JobClass, MergeMethod, Mergeability, PrState,
    ReviewDecision,
};

use time::OffsetDateTime;

use crate::contract::Fixture;
use crate::{
    CommitStatus, MergeIntoRef, PrMerge, PrSnapshot, RecentDelivery, RepoPermission, RepoRef,
    RequiredChecks, Scm, ScmDelivery, ScmError, ScmEvent, ScmFuture, ScmUser, StatusUpdate,
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

impl WebhookFaults {
    /// A step of splitmix64: deterministic, without the randomness of the
    /// system (`AGENTS.md` D16).
    fn next(&mut self) -> u64 {
        self.seed = self.seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.seed;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn chance(&mut self, percent: u8) -> bool {
        self.next() % 100 < u64::from(percent)
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
    /// The head when the head branch is gone.
    last_head: String,
    base_branch: String,
    state: PrState,
    draft: bool,
    labels: BTreeSet<String>,
    review_decision: ReviewDecision,
    author: ScmUser,
    /// A value that a test forces, in place of the computed one.
    mergeability: Option<Mergeability>,
    merged_sha: Option<String>,
}

#[derive(Debug)]
struct Repo {
    connection: String,
    commits: HashMap<String, Commit>,
    branches: BTreeMap<String, String>,
    prs: BTreeMap<u64, Pr>,
    checks: HashMap<String, Vec<CheckReport>>,
    comments: BTreeMap<String, (u64, String)>,
    rules: BTreeMap<String, RequiredChecks>,
    permissions: BTreeMap<String, RepoPermission>,
}

#[derive(Debug, Default)]
struct State {
    next_sha: u64,
    next_comment: u64,
    repos: BTreeMap<String, Repo>,
    lost: BTreeSet<String>,
    faults: VecDeque<Fault>,
    calls: Vec<SimCall>,
    deliveries: Vec<ScmDelivery>,
    /// Each attempt taken, with the identifier of its delivery: the attempt
    /// `n` is at `n - 1`.
    attempts: Vec<(String, ScmDelivery)>,
    /// The attempts asked again, sent at the next take.
    redelivered: Vec<usize>,
    /// The number of new deliveries taken.
    delivered: u64,
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
            let sha = state.new_sha();
            let tree = files
                .iter()
                .map(|(path, content)| ((*path).to_owned(), (*content).to_owned()))
                .collect();
            let commit = Commit {
                parents: Vec::new(),
                tree,
            };
            state.repos.insert(
                repo.external_id.clone(),
                Repo {
                    connection: repo.connection.external_id.clone(),
                    commits: HashMap::from([(sha.clone(), commit)]),
                    branches: BTreeMap::from([(default_branch.to_owned(), sha.clone())]),
                    prs: BTreeMap::new(),
                    checks: HashMap::new(),
                    comments: BTreeMap::new(),
                    rules: BTreeMap::new(),
                    permissions: BTreeMap::new(),
                },
            );
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
            ctx.new_pr(head_branch, base_branch, author, false)
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
            match change {
                PrChange::Draft(draft) => pr.draft = draft,
                PrChange::Review(decision) => pr.review_decision = decision,
                PrChange::Mergeability(mergeability) => pr.mergeability = mergeability,
                PrChange::Close => pr.state = PrState::Closed,
            }
            ctx.pr_changed(number);
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

    /// Gives `user` the permission `permission` on the repository.
    ///
    /// # Errors
    ///
    /// `NotFound` for an unknown repository.
    pub fn set_permission(
        &self,
        repo: &RepoRef,
        user: &ScmUser,
        permission: RepoPermission,
    ) -> Result<(), ScmError> {
        self.with_repo(&repo.external_id, |ctx| {
            ctx.repo
                .permissions
                .insert(user.external_id.clone(), permission);
            Ok(())
        })
    }

    /// Suspends the connection `connection_external_id`, or makes it active
    /// again: a suspended connection fails every call with `AccessLost`.
    pub fn set_access(&self, connection_external_id: &str, active: bool) {
        self.with(|state| {
            let (changed, event) = if active {
                (
                    state.lost.remove(connection_external_id),
                    ScmEvent::AccessRestored,
                )
            } else {
                (
                    state.lost.insert(connection_external_id.to_owned()),
                    ScmEvent::AccessLost,
                )
            };
            if changed {
                state.deliveries.push(ScmDelivery {
                    connection_external_id: Some(connection_external_id.to_owned()),
                    event,
                });
            }
        });
    }

    /// Makes the next calls fail, after the faults already waiting.
    pub fn fail(&self, fault: Fault) {
        self.with(|state| state.faults.push_back(fault));
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
            let mut taken = std::mem::take(&mut state.deliveries);
            for delivery in &taken {
                state.delivered += 1;
                let id = format!("sim-{}", state.delivered);
                state.attempts.push((id, delivery.clone()));
            }
            for attempt in std::mem::take(&mut state.redelivered) {
                let again = state.attempts[attempt].clone();
                taken.push(again.1.clone());
                state.attempts.push(again);
            }
            taken
        })
    }

    /// The deliveries sent since the last take, lost, sent twice and
    /// reordered as `faults` says.
    pub fn take_deliveries_with(&self, faults: &mut WebhookFaults) -> Vec<ScmDelivery> {
        let mut deliveries = Vec::new();
        for delivery in self.take_deliveries() {
            if faults.chance(faults.lose_percent) {
                continue;
            }
            if faults.chance(faults.duplicate_percent) {
                deliveries.push(delivery.clone());
            }
            deliveries.push(delivery);
        }
        if faults.reorder {
            // Fisher-Yates, with the generator of the faults.
            for i in (1..deliveries.len()).rev() {
                let j = usize::try_from(faults.next() % (i as u64 + 1)).unwrap_or(i);
                deliveries.swap(i, j);
            }
        }
        deliveries
    }

    /// Runs a call of the trait on the whole instance: records it and applies
    /// the faults.
    fn instance_call<T: Send + 'static>(
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

    /// Runs a call of the trait: records it, applies the faults and the
    /// access of the connection, then runs `run` on the repository.
    fn call<T: Send + 'static>(
        &self,
        name: &'static str,
        repo: &RepoRef,
        class: JobClass,
        run: impl FnOnce(&mut Ctx<'_>) -> Result<T, ScmError>,
    ) -> ScmFuture<'_, T> {
        let result = self.with(|state| {
            state.calls.push(SimCall { name, class });
            if let Some(error) = state.take_fault(name) {
                return Err(error);
            }
            if state.lost.contains(&repo.connection.external_id) {
                return Err(ScmError::AccessLost("connection suspended".into()));
            }
            state
                .ctx(&repo.external_id)
                .and_then(|mut ctx| run(&mut ctx))
        });
        Box::pin(std::future::ready(result))
    }
}

/// A change of a PR from outside CIHub.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrChange {
    Draft(bool),
    Review(ReviewDecision),
    /// Forces the mergeability, or computes it again with `None`.
    Mergeability(Option<Mergeability>),
    Close,
}

impl State {
    fn new_sha(&mut self) -> String {
        self.next_sha += 1;
        format!("{:040x}", self.next_sha)
    }

    fn take_fault(&mut self, name: &str) -> Option<ScmError> {
        let index = self
            .faults
            .iter()
            .position(|fault| fault.call.is_none_or(|call| call == name))?;
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

/// A repository and the counters of the simulator, during a call.
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
        *self.next_sha += 1;
        let sha = format!("{:040x}", *self.next_sha);
        self.repo
            .commits
            .insert(sha.clone(), Commit { parents, tree });
        sha
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
        self.pr_labeled(number, None);
    }

    fn pr_labeled(&mut self, number: u64, label_added: Option<String>) {
        let Some(pr) = self.repo.prs.get(&number) else {
            return;
        };
        let base_branch = pr.base_branch.clone();
        self.emit(ScmEvent::PrChanged {
            repo_external_id: self.repo_external_id.clone(),
            number,
            base_branch,
            label_added,
        });
    }

    fn new_pr(
        &mut self,
        head_branch: &str,
        base_branch: &str,
        author: &ScmUser,
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
                labels: BTreeSet::new(),
                review_decision: ReviewDecision::None,
                author: author.clone(),
                mergeability: None,
                merged_sha: None,
            },
        );
        self.pr_changed(number);
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
            author: author.clone(),
            body: body.to_owned(),
        });
        Ok(id)
    }

    /// Merges the commit `theirs` into the branch `branch` for a PR merge:
    /// the new commit has the parents that `method` gives.
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
                    return Err(ScmError::MergeRejected("not a fast forward".into()));
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

    /// A common ancestor of `a` and `b` that no other common ancestor
    /// descends from.
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

    /// The three-way merge of the trees of `ours` and `theirs`, or `None`
    /// when a file changed differently on both sides.
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

    fn snapshot(&self, number: u64) -> Result<PrSnapshot, ScmError> {
        let pr = self
            .prs
            .get(&number)
            .ok_or_else(|| ScmError::NotFound(format!("PR {number}")))?;
        let head = self.head(pr);
        let base = self.branch_head(&pr.base_branch)?;
        let (base_tree, head_tree) = match self.merge_base(&base, &head) {
            Some(fork) => (self.tree(&fork)?, self.tree(&head)?),
            None => (self.tree(&head)?, self.tree(&head)?),
        };
        let changed: BTreeSet<&String> = base_tree
            .keys()
            .chain(head_tree.keys())
            .filter(|path| base_tree.get(*path) != head_tree.get(*path))
            .collect();
        let deletions = changed
            .iter()
            .filter(|path| !head_tree.contains_key(**path))
            .count();
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
            draft: pr.draft,
            labels: pr.labels.clone(),
            mergeability,
            review_decision: pr.review_decision,
            checks: self.checks.get(&head).cloned().unwrap_or_default(),
            author: pr.author.clone(),
            additions: (changed.len() - deletions) as u64,
            deletions: deletions as u64,
            changed_files: changed.len() as u64,
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

    fn pull_request<'a>(
        &'a self,
        repo: &'a RepoRef,
        number: u64,
        class: JobClass,
    ) -> ScmFuture<'a, PrSnapshot> {
        self.call("pull_request", repo, class, move |ctx| {
            ctx.repo.snapshot(number)
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
    ) -> ScmFuture<'a, RepoPermission> {
        let user = user.external_id.clone();
        self.call("permission", repo, class, move |ctx| {
            Ok(ctx
                .repo
                .permissions
                .get(&user)
                .copied()
                .unwrap_or(RepoPermission::None))
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
            let merged = ctx.land(&base, &head, method)?;
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
                ctx.pr_labeled(number, Some(label));
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
        _title: &'a str,
        _body: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, u64> {
        let (head, base) = (head.to_owned(), base.to_owned());
        self.call("open_draft_pr", repo, class, move |ctx| {
            let open = ctx.repo.prs.iter().find(|(_, pr)| {
                pr.state == PrState::Open && pr.head_branch == head && pr.base_branch == base
            });
            match open {
                Some((number, _)) => Ok(*number),
                None => ctx.new_pr(&head, &base, &bot(), true),
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

    /// Every attempt taken: the simulator has no clock.
    fn recent_deliveries(
        &self,
        _since: OffsetDateTime,
        class: JobClass,
    ) -> ScmFuture<'_, Vec<RecentDelivery>> {
        self.instance_call("recent_deliveries", class, |state| {
            Ok(state
                .attempts
                .iter()
                .enumerate()
                .map(|(n, (id, _))| RecentDelivery {
                    delivery_id: id.clone(),
                    attempt_id: (n + 1).to_string(),
                })
                .collect())
        })
    }

    fn redeliver<'a>(&'a self, attempt_id: &'a str, class: JobClass) -> ScmFuture<'a, ()> {
        self.instance_call("redeliver", class, move |state| {
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

    let feature = sim.commit(repo, &root, &[("b.txt", Some("b\n"))])?;
    sim.push(repo, "feature", &feature, &author)?;
    let mergeable_pr = sim.open_pr(repo, "feature", "main", &author)?;
    sim.change_pr(
        repo,
        mergeable_pr,
        PrChange::Review(ReviewDecision::Approved),
    )?;
    let ci = CheckReport {
        name: "ci".into(),
        kind: CheckKind::Check,
        conclusion: Some(CheckConclusion::Success),
        details_url: None,
    };
    sim.report_check(repo, &feature, ci)?;

    let conflicting = sim.commit(repo, &root, &[("a.txt", Some("x\n"))])?;
    sim.push(repo, "conflicting", &conflicting, &author)?;
    let conflicting_pr = sim.open_pr(repo, "conflicting", "main", &author)?;
    let main = sim.commit(repo, &root, &[("a.txt", Some("y\n"))])?;
    sim.push(repo, "main", &main, &author)?;

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
    sim.set_permission(repo, &admin, RepoPermission::Admin)?;
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
    }

    #[tokio::test]
    async fn a_fault_fails_its_calls_then_stops() {
        let repo = repo();
        let (sim, fixture) = contract_world(&repo).unwrap();
        let until = time::OffsetDateTime::UNIX_EPOCH;
        sim.fail(Fault {
            call: Some("pull_request"),
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
        sim.set_access("100", true);
        assert!(
            sim.branch_head(&repo, "main", JobClass::Repair)
                .await
                .is_ok()
        );
        let events: Vec<ScmEvent> = sim.take_deliveries().into_iter().map(|d| d.event).collect();
        assert_eq!(events, [ScmEvent::AccessLost, ScmEvent::AccessRestored]);
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
        let sent = sim.take_deliveries();
        assert_eq!(sim.take_deliveries_with(lose_all), []);
        let since = OffsetDateTime::UNIX_EPOCH;
        let recent = sim
            .recent_deliveries(since, JobClass::Repair)
            .await
            .unwrap();
        let ids: Vec<_> = recent.iter().map(|r| r.delivery_id.as_str()).collect();
        assert_eq!(ids, ["sim-1", "sim-2", "sim-3"]);
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
                attempt_id: "4".into()
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
        assert!(
            deliveries(20)
                .take_deliveries_with(&mut faults(100, 0, false))
                .is_empty()
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
}

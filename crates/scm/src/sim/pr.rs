use std::collections::{BTreeMap, BTreeSet};

use cihub_domain::{CheckReport, MergeMethod, Mergeability, Permission, PrState, ReviewDecision};
use time::OffsetDateTime;

use super::{Ctx, Repo, Sim, bot};
use crate::repo::has_passed;
use crate::{PrSnapshot, RepoRef, RequiredChecks, SUBMIT_BOX, ScmError, ScmEvent, ScmUser};

#[derive(Debug)]
pub(super) struct Pr {
    pub(super) head_branch: String,
    /// The head when the PR closed, for a PR whose branch is gone.
    pub(super) last_head: String,
    pub(super) base_branch: String,
    pub(super) state: PrState,
    pub(super) is_draft: bool,
    pub(super) title: String,
    pub(super) body: String,
    pub(super) labels: BTreeSet<String>,
    pub(super) review_decision: ReviewDecision,
    pub(super) reviews: Reviews,
    pub(super) unresolved_threads: Vec<String>,
    pub(super) author: ScmUser,
    /// The mergeability that a test forces, or `None` to compute it.
    pub(super) mergeability: Option<Mergeability>,
    pub(super) merged_sha: Option<String>,
}

/// A comment on the PR `pr`.
#[derive(Debug)]
pub(super) struct Comment {
    pub(super) pr: u64,
    pub(super) body: String,
}

/// Who reviewed a PR, and whose review it asks for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reviews {
    /// The logins whose last review approves the PR.
    pub approved_by: BTreeSet<String>,
    /// The logins whose last review asks for changes.
    pub changes_requested_by: BTreeSet<String>,
    /// The users and teams whose review the PR asks for.
    pub requested: BTreeSet<String>,
}

/// A change of a PR from outside CIHub.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrChange {
    /// Makes the PR a draft with `true`, or ready for review with `false`.
    Draft(bool),
    /// Sets the review decision; the reviews themselves do not follow.
    Review(ReviewDecision),
    /// Forces the mergeability, or computes it again with `None`.
    Mergeability(Option<Mergeability>),
    /// Renames the PR.
    Title(&'static str),
    /// Closes the PR without a merge.
    Close,
}

/// A rule that restricts the updates of a branch, as
/// [`Sim::set_update_restriction`] sets it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpdateRestriction {
    /// The App passes the rule, so the queue can merge.
    pub is_app_exempt: bool,
    /// The rule covers the test branches of the queue too.
    pub covers_test_branches: bool,
}

impl Sim {
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
            let opened = NewPr {
                head_branch,
                base_branch,
                author,
                title: head_branch,
                body: "",
                is_draft: false,
            };
            ctx.new_pr(&opened)
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
                PrChange::Draft(is_draft) => pr.is_draft = is_draft,
                PrChange::Review(decision) => pr.review_decision = decision,
                PrChange::Mergeability(mergeability) => pr.mergeability = mergeability,
                PrChange::Title(title) => title.clone_into(&mut pr.title),
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
        let empty = format!("- [ ] {SUBMIT_BOX}");
        self.with_repo(&repo.external_id, |ctx| {
            let (id, comment) = ctx
                .repo
                .comments
                .iter_mut()
                .find(|(_, comment)| comment.pr == number && comment.body.contains(&empty))
                .ok_or_else(|| ScmError::NotFound(format!("a box on PR {number}")))?;
            comment.body = comment.body.replace(&empty, &format!("- [x] {SUBMIT_BOX}"));
            let comment_external_id = id.to_string();
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

    /// The bodies of the comments of the PR `number`, in their order.
    pub fn comments(&self, repo: &RepoRef, number: u64) -> Vec<String> {
        self.comments_where(repo, number, |_| true)
    }

    /// The bodies of the comments of the PR `number` that CIHub
    /// acknowledged, in their order.
    pub fn acknowledged_comments(&self, repo: &RepoRef, number: u64) -> Vec<String> {
        self.comments_where(repo, number, |is_acknowledged| is_acknowledged)
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
            ctx.rules_changed();

            Ok(())
        })
    }

    /// Restricts the updates of the branch `branch` with `restriction`, or
    /// lifts the restriction with `None`. Only the setup of the repository
    /// reads it: a merge does not check it.
    ///
    /// # Errors
    ///
    /// `NotFound` for an unknown repository.
    pub fn set_update_restriction(
        &self,
        repo: &RepoRef,
        branch: &str,
        restriction: Option<UpdateRestriction>,
    ) -> Result<(), ScmError> {
        self.with_repo(&repo.external_id, |ctx| {
            match restriction {
                Some(restriction) => ctx.repo.restrictions.insert(branch.to_owned(), restriction),
                None => ctx.repo.restrictions.remove(branch),
            };
            ctx.rules_changed();

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

    fn comments_where(
        &self,
        repo: &RepoRef,
        number: u64,
        keep: impl Fn(bool) -> bool,
    ) -> Vec<String> {
        self.with(|state| {
            let Some(repo) = state.repos.get(&repo.external_id) else {
                return Vec::new();
            };

            repo.comments
                .iter()
                .filter(|(id, comment)| {
                    comment.pr == number && keep(repo.acknowledged.contains(id))
                })
                .map(|(_, comment)| comment.body.clone())
                .collect()
        })
    }
}

/// A PR to open.
pub(super) struct NewPr<'a> {
    pub(super) head_branch: &'a str,
    pub(super) base_branch: &'a str,
    pub(super) author: &'a ScmUser,
    pub(super) title: &'a str,
    pub(super) body: &'a str,
    pub(super) is_draft: bool,
}

/// The page of PR `number`, for the read of the PR and for
/// [`crate::RepoCalls::pr_url`].
pub(super) fn pr_page(owner: &str, name: &str, number: u64) -> String {
    format!("https://scm.example/{owner}/{name}/prs/{number}")
}

impl Ctx<'_> {
    /// Moves the branch `branch` to `sha`, or deletes it with `None`, and
    /// sends the deliveries of the push and of the PRs that it changes.
    pub(super) fn move_branch(&mut self, branch: &str, sha: Option<&str>, sender: &ScmUser) {
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

    pub(super) fn pr_changed(&mut self, number: u64) {
        self.pr_event(number, None);
    }

    pub(super) fn pr_event(&mut self, number: u64, label_added: Option<String>) {
        let Some(pr) = self.repo.prs.get(&number) else {
            return;
        };

        let base_branch = pr.base_branch.clone();
        let is_ready_for_review = pr.state == PrState::Open && !pr.is_draft;
        self.emit(ScmEvent::PrChanged {
            repo_external_id: self.repo_external_id.clone(),
            number,
            base_branch,
            label_added,
            is_ready_for_review,
        });
    }

    pub(super) fn new_pr(&mut self, opened: &NewPr<'_>) -> Result<u64, ScmError> {
        let head = self.repo.branch_head(opened.head_branch)?;
        self.repo.branch_head(opened.base_branch)?;
        let number = self.repo.prs.keys().next_back().map_or(1, |last| last + 1);
        let pr = Pr {
            head_branch: opened.head_branch.to_owned(),
            last_head: head,
            base_branch: opened.base_branch.to_owned(),
            state: PrState::Open,
            is_draft: opened.is_draft,
            title: opened.title.to_owned(),
            body: opened.body.to_owned(),
            labels: BTreeSet::new(),
            review_decision: ReviewDecision::None,
            reviews: Reviews::default(),
            unresolved_threads: Vec::new(),
            author: opened.author.clone(),
            mergeability: None,
            merged_sha: None,
        };

        self.repo.prs.insert(number, pr);
        self.pr_changed(number);

        Ok(number)
    }

    pub(super) fn report(&mut self, sha: &str, check: CheckReport) {
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

    pub(super) fn add_comment(
        &mut self,
        number: u64,
        author: &ScmUser,
        body: &str,
    ) -> Result<String, ScmError> {
        self.repo.pr(number)?;
        *self.next_comment += 1;
        let id = *self.next_comment;
        let comment = Comment {
            pr: number,
            body: body.to_owned(),
        };
        self.repo.comments.insert(id, comment);

        self.emit(ScmEvent::Commented {
            repo_external_id: self.repo_external_id.clone(),
            number,
            comment_external_id: id.to_string(),
            author: author.clone(),
            body: body.to_owned(),
        });

        Ok(id.to_string())
    }

    /// Lands `theirs` on the branch `branch` with `method`, and returns the
    /// new head of the branch.
    pub(super) fn land(
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

    fn rules_changed(&mut self) {
        let repo_external_id = self.repo_external_id.clone();
        self.emit(ScmEvent::RulesChanged { repo_external_id });
    }
}

impl Repo {
    pub(super) fn pr(&self, number: u64) -> Result<&Pr, ScmError> {
        self.prs
            .get(&number)
            .ok_or_else(|| ScmError::NotFound(format!("PR {number}")))
    }

    pub(super) fn pr_mut(&mut self, number: u64) -> Result<&mut Pr, ScmError> {
        self.prs
            .get_mut(&number)
            .ok_or_else(|| ScmError::NotFound(format!("PR {number}")))
    }

    /// The head of `pr`: the head of its branch, or its last one once the
    /// branch is gone.
    pub(super) fn head(&self, pr: &Pr) -> String {
        self.branches
            .get(&pr.head_branch)
            .cloned()
            .unwrap_or_else(|| pr.last_head.clone())
    }

    pub(super) fn mergeability(&self, pr: &Pr, head: &str) -> Result<Mergeability, ScmError> {
        if let Some(forced) = pr.mergeability {
            return Ok(forced);
        }

        let base = self.branch_head(&pr.base_branch)?;
        if self.merged_tree(&base, head)?.is_none() {
            return Ok(Mergeability::Conflict);
        }

        let rules = self.rules.get(&pr.base_branch).cloned().unwrap_or_default();
        if rules.requires_up_to_date && !self.is_ancestor(&base, head) {
            return Ok(Mergeability::Behind);
        }

        let checks = self.checks.get(head).map(Vec::as_slice).unwrap_or_default();
        let do_checks_pass = rules.names.iter().all(|name| has_passed(checks, name));
        let is_reviewed = !matches!(
            pr.review_decision,
            ReviewDecision::ChangesRequested | ReviewDecision::ReviewRequired
        );
        if pr.is_draft || !do_checks_pass || !is_reviewed {
            return Ok(Mergeability::Blocked);
        }

        Ok(Mergeability::Mergeable)
    }

    pub(super) fn changed_files(&self, number: u64) -> Result<Vec<String>, ScmError> {
        let pr = self.pr(number)?;

        Ok(self.changes(pr, &self.head(pr))?.into_keys().collect())
    }

    pub(super) fn snapshot(&self, number: u64) -> Result<PrSnapshot, ScmError> {
        let pr = self.pr(number)?;
        let head = self.head(pr);
        let changes = self.changes(pr, &head)?;
        let deletions = changes.values().filter(|is_deleted| **is_deleted).count();
        let mergeability = if pr.state == PrState::Open {
            self.mergeability(pr, &head)?
        } else {
            Mergeability::Blocked
        };

        Ok(PrSnapshot {
            number,
            url: pr_page(&self.owner, &self.name, number),
            state: pr.state,
            head_sha: head.clone(),
            base_branch: pr.base_branch.clone(),
            head_branch: pr.head_branch.clone(),
            is_from_fork: false,
            created_at: OffsetDateTime::UNIX_EPOCH,
            is_draft: pr.is_draft,
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

    /// The files that `pr` changes since it left its base, each with whether
    /// it deletes the file.
    fn changes(&self, pr: &Pr, head: &str) -> Result<BTreeMap<String, bool>, ScmError> {
        let head_tree = self.tree(head)?;
        let base = self.branch_head(&pr.base_branch)?;
        let Some(fork) = self.merge_base(&base, head) else {
            // Without a common history, nothing tells what the PR changes.
            return Ok(BTreeMap::new());
        };

        let fork_tree = self.tree(&fork)?;

        Ok(fork_tree
            .keys()
            .chain(head_tree.keys())
            .filter(|path| fork_tree.get(*path) != head_tree.get(*path))
            .map(|path| (path.clone(), !head_tree.contains_key(path)))
            .collect())
    }
}

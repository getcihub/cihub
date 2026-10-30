use cihub_domain::{
    CheckConclusion, CheckKind, CheckReport, CheckStatus, JobClass, MembershipRole, MergeMethod,
    Mergeability, Permission, PrState,
};
use secrecy::{ExposeSecret, SecretString};
use time::OffsetDateTime;

use super::people::encode_query;
use super::pr::{NewPr, pr_page};
use super::{Ctx, ScmMethod, Sim, bot};
use crate::{
    AppCalls, BranchMove, CommitStatus, ConnectionRef, DraftPr, MAX_OPEN_PRS, MAX_PRS_PER_READ,
    MergeIntoRef, PersonCalls, PrMerge, PrSnapshot, RecentDelivery, RepoCalls, RepoRef,
    RequiredChecks, ScmAccount, ScmAccountProfile, ScmError, ScmFuture, ScmInstallation, ScmMember,
    ScmMembership, ScmPersonReach, ScmPersonRepo, ScmPersonToken, ScmRepo, ScmUser, SetupFinding,
    SignInCode, SignInRequest, SignedIn, StatusUpdate, code_challenge,
};

impl RepoCalls for Sim {
    fn pull_requests<'a>(
        &'a self,
        repo: &'a RepoRef,
        numbers: &'a [u64],
        class: JobClass,
    ) -> ScmFuture<'a, Vec<PrSnapshot>> {
        self.call_on_repo(ScmMethod::PullRequests, repo, class, |ctx| {
            if numbers.len() > MAX_PRS_PER_READ {
                return Err(ScmError::Validation("too many PRs in one read".into()));
            }

            numbers
                .iter()
                .filter(|number| ctx.repo.prs.contains_key(number))
                .map(|number| ctx.repo.snapshot(*number))
                .collect()
        })
    }

    fn open_pull_requests<'a>(
        &'a self,
        repo: &'a RepoRef,
        base_branch: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<u64>> {
        self.call_on_repo(ScmMethod::OpenPullRequests, repo, class, |ctx| {
            Ok(ctx
                .repo
                .prs
                .iter()
                .filter(|(_, pr)| pr.state == PrState::Open && pr.base_branch == base_branch)
                .map(|(number, _)| *number)
                .take(MAX_OPEN_PRS)
                .collect())
        })
    }

    fn branch_head<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, Option<String>> {
        self.call_on_repo(ScmMethod::BranchHead, repo, class, |ctx| {
            Ok(ctx.repo.branches.get(branch).cloned())
        })
    }

    fn checks<'a>(
        &'a self,
        repo: &'a RepoRef,
        sha: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<CheckReport>> {
        self.call_on_repo(ScmMethod::Checks, repo, class, |ctx| {
            Ok(ctx.repo.checks.get(sha).cloned().unwrap_or_default())
        })
    }

    fn required_checks<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, RequiredChecks> {
        self.call_on_repo(ScmMethod::RequiredChecks, repo, class, |ctx| {
            ctx.repo.branch_head(branch)?;

            Ok(ctx.repo.rules.get(branch).cloned().unwrap_or_default())
        })
    }

    fn file<'a>(
        &'a self,
        repo: &'a RepoRef,
        git_ref: &'a str,
        path: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, Option<String>> {
        self.call_on_repo(ScmMethod::File, repo, class, |ctx| {
            let sha = ctx
                .repo
                .branches
                .get(git_ref)
                .map_or(git_ref, String::as_str);

            Ok(ctx.repo.tree(sha)?.get(path).cloned())
        })
    }

    fn permission<'a>(
        &'a self,
        repo: &'a RepoRef,
        user: &'a ScmUser,
        class: JobClass,
    ) -> ScmFuture<'a, Permission> {
        self.call_on_repo(ScmMethod::Permission, repo, class, |ctx| {
            Ok(ctx
                .repo
                .permissions
                .get(&user.external_id)
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
        self.call_on_repo(ScmMethod::IsAncestor, repo, class, |ctx| {
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
        self.call_on_repo(ScmMethod::PrFiles, repo, class, |ctx| {
            ctx.repo.changed_files(number)
        })
    }

    fn unresolved_threads<'a>(
        &'a self,
        repo: &'a RepoRef,
        number: u64,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<String>> {
        self.call_on_repo(ScmMethod::UnresolvedThreads, repo, class, |ctx| {
            Ok(ctx.repo.pr(number)?.unresolved_threads.clone())
        })
    }

    fn set_branch<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        sha: &'a str,
        how: BranchMove,
        class: JobClass,
    ) -> ScmFuture<'a, ()> {
        self.call_on_repo(ScmMethod::SetBranch, repo, class, |ctx| {
            ctx.repo
                .tree(sha)
                .map_err(|_| ScmError::Validation(format!("unknown commit {sha}")))?;
            if let Some(before) = ctx.repo.branches.get(branch)
                && how == BranchMove::FastForwardOnly
                && !ctx.repo.is_ancestor(before, sha)
            {
                return Err(ScmError::NotFastForward);
            }

            ctx.move_branch(branch, Some(sha), &bot());

            Ok(())
        })
    }

    fn delete_branch<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, ()> {
        self.call_on_repo(ScmMethod::DeleteBranch, repo, class, |ctx| {
            ctx.move_branch(branch, None, &bot());

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
        self.call_on_repo(ScmMethod::MergeIntoBranch, repo, class, |ctx| {
            let ours = ctx.repo.branch_head(branch)?;
            ctx.repo.tree(sha)?;
            if ctx.repo.is_ancestor(sha, &ours) {
                return Ok(MergeIntoRef::AlreadyContained);
            }

            let Some(tree) = ctx.repo.merged_tree(&ours, sha)? else {
                return Ok(MergeIntoRef::Conflict);
            };

            let merged = ctx.new_commit(vec![ours, sha.to_owned()], tree);
            ctx.move_branch(branch, Some(&merged), &bot());

            Ok(MergeIntoRef::Merged { sha: merged })
        })
    }

    fn merge_pr<'a>(
        &'a self,
        repo: &'a RepoRef,
        merge: &'a PrMerge,
        class: JobClass,
    ) -> ScmFuture<'a, String> {
        self.call_on_repo(ScmMethod::MergePr, repo, class, |ctx| {
            let pr = ctx.repo.pr(merge.number)?;
            if let Some(merged) = &pr.merged_sha {
                return Ok(merged.clone());
            }

            if pr.state != PrState::Open {
                return Err(ScmError::MergeRejected("PR closed".into()));
            }

            let head = ctx.repo.head(pr);
            if head != merge.head_sha {
                return Err(ScmError::HeadChanged);
            }

            match ctx.repo.mergeability(pr, &head)? {
                Mergeability::Mergeable => {}
                Mergeability::Behind => {
                    return Err(ScmError::MergeRejected(
                        "the head must be up to date".into(),
                    ));
                }
                other => return Err(ScmError::MergeRejected(format!("PR {other}"))),
            }

            let base = pr.base_branch.clone();
            let theirs = if merge.method == MergeMethod::FastForward {
                &merge.tested_sha
            } else {
                &head
            };
            let merged = ctx.land(&base, theirs, merge.method)?;
            let pr = ctx.repo.pr_mut(merge.number)?;
            pr.state = PrState::Merged;
            pr.last_head = head;
            pr.merged_sha = Some(merged.clone());
            ctx.pr_changed(merge.number);

            Ok(merged)
        })
    }

    fn set_status<'a>(
        &'a self,
        repo: &'a RepoRef,
        status: &'a StatusUpdate,
        class: JobClass,
    ) -> ScmFuture<'a, ()> {
        self.call_on_repo(ScmMethod::SetStatus, repo, class, |ctx| {
            ctx.repo.tree(&status.sha)?;
            let check = status_report(status);
            let is_unchanged = ctx
                .repo
                .checks
                .get(&status.sha)
                .is_some_and(|reports| reports.contains(&check));
            if !is_unchanged {
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
        self.call_on_repo(ScmMethod::CreateComment, repo, class, |ctx| {
            ctx.add_comment(number, &bot(), body)
        })
    }

    fn update_comment<'a>(
        &'a self,
        repo: &'a RepoRef,
        comment_id: &'a str,
        body: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, ()> {
        self.call_on_repo(ScmMethod::UpdateComment, repo, class, |ctx| {
            let id = comment_key(ctx, comment_id)?;
            if let Some(comment) = ctx.repo.comments.get_mut(&id) {
                body.clone_into(&mut comment.body);
            }

            Ok(())
        })
    }

    fn acknowledge_comment<'a>(
        &'a self,
        repo: &'a RepoRef,
        comment_id: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, ()> {
        self.call_on_repo(ScmMethod::AcknowledgeComment, repo, class, |ctx| {
            let id = comment_key(ctx, comment_id)?;
            ctx.repo.acknowledged.insert(id);

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
        self.call_on_repo(ScmMethod::AddLabel, repo, class, |ctx| {
            if ctx.repo.pr_mut(number)?.labels.insert(label.to_owned()) {
                ctx.pr_event(number, Some(label.to_owned()));
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
        self.call_on_repo(ScmMethod::RemoveLabel, repo, class, |ctx| {
            if ctx.repo.pr_mut(number)?.labels.remove(label) {
                ctx.pr_changed(number);
            }

            Ok(())
        })
    }

    fn open_draft_pr<'a>(
        &'a self,
        repo: &'a RepoRef,
        draft: &'a DraftPr,
        class: JobClass,
    ) -> ScmFuture<'a, u64> {
        self.call_on_repo(ScmMethod::OpenDraftPr, repo, class, |ctx| {
            let open = ctx.repo.prs.iter().find(|(_, pr)| {
                pr.state == PrState::Open
                    && pr.head_branch == draft.head
                    && pr.base_branch == draft.base
            });
            if let Some((number, _)) = open {
                return Ok(*number);
            }

            let opened = NewPr {
                head_branch: &draft.head,
                base_branch: &draft.base,
                author: &bot(),
                title: &draft.title,
                body: &draft.body,
                is_draft: true,
            };
            ctx.new_pr(&opened)
        })
    }

    fn close_pr<'a>(
        &'a self,
        repo: &'a RepoRef,
        number: u64,
        class: JobClass,
    ) -> ScmFuture<'a, ()> {
        self.call_on_repo(ScmMethod::ClosePr, repo, class, |ctx| {
            let pr = ctx.repo.pr_mut(number)?;
            if pr.state == PrState::Open {
                pr.state = PrState::Closed;
                ctx.pr_changed(number);
            }

            Ok(())
        })
    }

    fn check_setup<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<SetupFinding>> {
        self.call_on_repo(ScmMethod::CheckSetup, repo, class, |ctx| {
            ctx.repo.branch_head(branch)?;
            let mut findings = Vec::new();
            match ctx.repo.restrictions.get(branch) {
                None => findings.push(SetupFinding::UpdateRestrictionMissing),
                Some(restriction) => {
                    if !restriction.is_app_exempt {
                        findings.push(SetupFinding::AppNotExempt);
                    }
                    if restriction.covers_test_branches {
                        findings.push(SetupFinding::TestBranchesNotExcluded);
                    }
                }
            }
            let requires_up_to_date = ctx
                .repo
                .rules
                .get(branch)
                .is_some_and(|rules| rules.requires_up_to_date);
            if requires_up_to_date {
                findings.push(SetupFinding::UpToDateRequired);
            }

            Ok(findings)
        })
    }

    fn branch_rules_url(&self, _repo: &RepoRef) -> Option<String> {
        None
    }

    fn pr_url(&self, repo: &RepoRef, number: u64) -> String {
        pr_page(&repo.owner, &repo.name, number)
    }
}

/// The check that a commit status gives.
fn status_report(status: &StatusUpdate) -> CheckReport {
    let conclusion = match status.status {
        CommitStatus::Pending => None,
        CommitStatus::Success => Some(CheckConclusion::Success),
        CommitStatus::Failure => Some(CheckConclusion::Failure),
        CommitStatus::Error => Some(CheckConclusion::Error),
    };
    let check_status = if conclusion.is_some() {
        CheckStatus::Completed
    } else {
        CheckStatus::InProgress
    };

    CheckReport {
        name: status.context.clone(),
        kind: CheckKind::Status,
        status: check_status,
        conclusion,
        details_url: status.details_url.clone(),
        started_at: None,
        completed_at: None,
    }
}

/// The key of the comment `comment_id` of the repository of `ctx`;
/// `NotFound` when it has none.
fn comment_key(ctx: &Ctx<'_>, comment_id: &str) -> Result<u64, ScmError> {
    comment_id
        .parse()
        .ok()
        .filter(|id| ctx.repo.comments.contains_key(id))
        .ok_or_else(|| ScmError::NotFound(format!("comment {comment_id}")))
}

impl AppCalls for Sim {
    fn recent_deliveries(
        &self,
        _since: OffsetDateTime,
        class: JobClass,
    ) -> ScmFuture<'_, Vec<RecentDelivery>> {
        self.call(ScmMethod::RecentDeliveries, class, |state| {
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
        self.call(ScmMethod::Redeliver, class, |state| {
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
        self.call(ScmMethod::Installation, class, |state| {
            Ok(state.installation(external_id))
        })
    }

    fn installations(&self, class: JobClass) -> ScmFuture<'_, Vec<ScmInstallation>> {
        self.call(ScmMethod::Installations, class, |state| {
            Ok(state
                .installations
                .keys()
                .filter_map(|external_id| state.installation(external_id))
                .collect())
        })
    }

    fn list_repos<'a>(
        &'a self,
        connection: &'a ConnectionRef,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<ScmRepo>> {
        self.call(ScmMethod::ListRepos, class, |state| {
            state.check_access(&connection.external_id)?;

            Ok(state
                .repos
                .iter()
                .filter(|(_, repo)| repo.connection == connection.external_id)
                .map(|(external_id, repo)| ScmRepo {
                    external_id: external_id.clone(),
                    owner: repo.owner.clone(),
                    name: repo.name.clone(),
                    default_branch: repo.default_branch.clone(),
                })
                .collect())
        })
    }

    fn members<'a>(
        &'a self,
        connection: &'a ConnectionRef,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<ScmMember>> {
        self.call(ScmMethod::Members, class, |state| {
            state.check_access(&connection.external_id)?;

            let Some(members) = state.members.get(&connection.external_id) else {
                return Ok(Vec::new());
            };

            let avatar_of = |user: &ScmUser| {
                state
                    .profiles
                    .get(&user.external_id)
                    .and_then(|profile| profile.avatar_url.clone())
            };

            Ok(members
                .values()
                .map(|member| ScmMember {
                    avatar_url: avatar_of(&member.user),
                    ..member.clone()
                })
                .collect())
        })
    }

    fn account_profile<'a>(
        &'a self,
        connection: &'a ConnectionRef,
        class: JobClass,
    ) -> ScmFuture<'a, ScmAccountProfile> {
        self.call(ScmMethod::AccountProfile, class, |state| {
            state.check_access(&connection.external_id)?;

            Ok(state
                .installations
                .get(&connection.external_id)
                .and_then(|installation| {
                    state
                        .account_profiles
                        .get(&installation.account.external_id)
                })
                .cloned()
                .unwrap_or_default())
        })
    }

    fn app_install_url(&self, class: JobClass) -> ScmFuture<'_, String> {
        self.call(ScmMethod::AppInstallUrl, class, |state| {
            Ok(state
                .install_page
                .clone()
                .unwrap_or_else(|| "sim:install".to_owned()))
        })
    }
}

impl PersonCalls for Sim {
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

    fn sign_in<'a>(
        &'a self,
        code: &'a SignInCode,
        class: JobClass,
    ) -> ScmFuture<'a, (SignedIn, ScmPersonToken)> {
        self.call(ScmMethod::SignIn, class, |state| {
            let refused = || ScmError::Validation("the code of the sign-in is not valid".into());
            let pending = state
                .sign_in_codes
                .remove(code.code.expose_secret())
                .ok_or_else(refused)?;
            let is_same_request = pending.request.redirect_url == code.redirect_url
                && pending.request.code_challenge == code_challenge(&code.code_verifier);
            if !is_same_request {
                return Err(refused());
            }

            let user = pending.user;
            let memberships: Vec<ScmMembership> = state
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
            let installations = state
                .reach_of_person(&user.external_id)
                .iter()
                .filter_map(|installation| state.installation(installation))
                .collect();
            let profile = state
                .profiles
                .get(&user.external_id)
                .cloned()
                .unwrap_or_default();
            let token = state.person_token(&user.external_id);
            let signed_in = SignedIn {
                user,
                profile,
                installations,
                memberships,
            };

            Ok((signed_in, token))
        })
    }

    fn refresh_person_token<'a>(
        &'a self,
        refresh: &'a SecretString,
        class: JobClass,
    ) -> ScmFuture<'a, ScmPersonToken> {
        self.call(ScmMethod::RefreshPersonToken, class, |state| {
            let user = state
                .refresh_tokens
                .remove(refresh.expose_secret())
                .ok_or_else(|| ScmError::AccessLost("the refresh token is refused".into()))?;

            Ok(state.person_token(&user))
        })
    }

    fn person_reach<'a>(
        &'a self,
        token: &'a SecretString,
        class: JobClass,
    ) -> ScmFuture<'a, ScmPersonReach> {
        self.call(ScmMethod::PersonReach, class, |state| {
            let user = state.person_of(token)?;
            let reached = state.reach_of_person(&user);
            let installations = reached
                .iter()
                .filter_map(|installation| state.installation(installation))
                .collect();
            let repos = state
                .repos
                .iter()
                .filter(|(_, repo)| reached.contains(&repo.connection))
                .map(|(external_id, repo)| ScmPersonRepo {
                    connection_external_id: repo.connection.clone(),
                    repo_external_id: external_id.clone(),
                    permission: state.permission_of_person(external_id, &user),
                })
                .filter(|repo| repo.permission != Permission::None)
                .collect();

            Ok(ScmPersonReach {
                installations,
                repos,
            })
        })
    }

    fn person_permission<'a>(
        &'a self,
        token: &'a SecretString,
        repo: &'a RepoRef,
        class: JobClass,
    ) -> ScmFuture<'a, Permission> {
        self.call(ScmMethod::PersonPermission, class, |state| {
            let user = state.person_of(token)?;

            Ok(state.permission_of_person(&repo.external_id, &user))
        })
    }

    fn person_role<'a>(
        &'a self,
        token: &'a SecretString,
        person: &'a ScmUser,
        account: &'a ScmAccount,
        class: JobClass,
    ) -> ScmFuture<'a, Option<MembershipRole>> {
        self.call(ScmMethod::PersonRole, class, |state| {
            state.person_of(token)?;

            Ok(state
                .installations
                .iter()
                .filter(|(_, installation)| {
                    installation.account.external_id == account.external_id
                        && installation.account.path == account.path
                })
                .find_map(|(external_id, _)| {
                    state
                        .members
                        .get(external_id)
                        .and_then(|members| members.get(&person.external_id))
                        .map(|member| member.role)
                }))
        })
    }
}

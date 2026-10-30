//! The `Scm` traits on GitHub. Each method calls the function of the same
//! name in the module of its resource.

use cihub_domain::{CheckReport, JobClass, MembershipRole, Permission};
use cihub_scm::{
    AppCalls, BranchMove, ConnectionRef, DraftPr, MergeIntoRef, PersonCalls, PrMerge, PrSnapshot,
    RecentDelivery, RepoCalls, RepoRef, RequiredChecks, ScmAccount, ScmAccountProfile, ScmFuture,
    ScmInstallation, ScmMember, ScmPersonReach, ScmPersonToken, ScmRepo, ScmUser, SetupFinding,
    SignInCode, SignInRequest, SignedIn, StatusUpdate,
};
use secrecy::SecretString;
use time::OffsetDateTime;

use crate::GithubApp;
use crate::{apps, checks, git, issues, people, pulls, rules};

impl RepoCalls for GithubApp {
    fn pull_requests<'a>(
        &'a self,
        repo: &'a RepoRef,
        numbers: &'a [u64],
        class: JobClass,
    ) -> ScmFuture<'a, Vec<PrSnapshot>> {
        Box::pin(pulls::pull_requests(self, repo, numbers, class))
    }

    fn open_pull_requests<'a>(
        &'a self,
        repo: &'a RepoRef,
        base_branch: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<u64>> {
        Box::pin(pulls::open_pull_requests(self, repo, base_branch, class))
    }

    fn branch_head<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, Option<String>> {
        Box::pin(git::branch_head(self, repo, branch, class))
    }

    fn checks<'a>(
        &'a self,
        repo: &'a RepoRef,
        sha: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<CheckReport>> {
        Box::pin(checks::checks(self, repo, sha, class))
    }

    fn required_checks<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, RequiredChecks> {
        Box::pin(rules::required_checks(self, repo, branch, class))
    }

    fn file<'a>(
        &'a self,
        repo: &'a RepoRef,
        git_ref: &'a str,
        path: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, Option<String>> {
        Box::pin(git::file(self, repo, git_ref, path, class))
    }

    fn permission<'a>(
        &'a self,
        repo: &'a RepoRef,
        user: &'a ScmUser,
        class: JobClass,
    ) -> ScmFuture<'a, Permission> {
        Box::pin(people::permission(self, repo, user, class))
    }

    fn is_ancestor<'a>(
        &'a self,
        repo: &'a RepoRef,
        ancestor: &'a str,
        descendant: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, bool> {
        Box::pin(git::is_ancestor(self, repo, ancestor, descendant, class))
    }

    fn pr_files<'a>(
        &'a self,
        repo: &'a RepoRef,
        number: u64,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<String>> {
        Box::pin(pulls::pr_files(self, repo, number, class))
    }

    fn unresolved_threads<'a>(
        &'a self,
        repo: &'a RepoRef,
        number: u64,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<String>> {
        Box::pin(pulls::unresolved_threads(self, repo, number, class))
    }

    fn set_branch<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        sha: &'a str,
        how: BranchMove,
        class: JobClass,
    ) -> ScmFuture<'a, ()> {
        Box::pin(git::set_branch(self, repo, branch, sha, how, class))
    }

    fn delete_branch<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, ()> {
        Box::pin(git::delete_branch(self, repo, branch, class))
    }

    fn merge_into_branch<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        sha: &'a str,
        message: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, MergeIntoRef> {
        Box::pin(git::merge_into_branch(
            self, repo, branch, sha, message, class,
        ))
    }

    fn merge_pr<'a>(
        &'a self,
        repo: &'a RepoRef,
        merge: &'a PrMerge,
        class: JobClass,
    ) -> ScmFuture<'a, String> {
        Box::pin(pulls::merge_pr(self, repo, merge, class))
    }

    fn set_status<'a>(
        &'a self,
        repo: &'a RepoRef,
        status: &'a StatusUpdate,
        class: JobClass,
    ) -> ScmFuture<'a, ()> {
        Box::pin(checks::set_status(self, repo, status, class))
    }

    fn create_comment<'a>(
        &'a self,
        repo: &'a RepoRef,
        number: u64,
        body: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, String> {
        Box::pin(issues::create_comment(self, repo, number, body, class))
    }

    fn update_comment<'a>(
        &'a self,
        repo: &'a RepoRef,
        comment_id: &'a str,
        body: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, ()> {
        Box::pin(issues::update_comment(self, repo, comment_id, body, class))
    }

    fn acknowledge_comment<'a>(
        &'a self,
        repo: &'a RepoRef,
        comment_id: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, ()> {
        Box::pin(issues::acknowledge_comment(self, repo, comment_id, class))
    }

    fn add_label<'a>(
        &'a self,
        repo: &'a RepoRef,
        number: u64,
        label: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, ()> {
        Box::pin(issues::add_label(self, repo, number, label, class))
    }

    fn remove_label<'a>(
        &'a self,
        repo: &'a RepoRef,
        number: u64,
        label: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, ()> {
        Box::pin(issues::remove_label(self, repo, number, label, class))
    }

    fn open_draft_pr<'a>(
        &'a self,
        repo: &'a RepoRef,
        draft: &'a DraftPr,
        class: JobClass,
    ) -> ScmFuture<'a, u64> {
        Box::pin(pulls::open_draft_pr(self, repo, draft, class))
    }

    fn close_pr<'a>(
        &'a self,
        repo: &'a RepoRef,
        number: u64,
        class: JobClass,
    ) -> ScmFuture<'a, ()> {
        Box::pin(pulls::close_pr(self, repo, number, class))
    }

    fn check_setup<'a>(
        &'a self,
        repo: &'a RepoRef,
        branch: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<SetupFinding>> {
        Box::pin(rules::check_setup(self, repo, branch, class))
    }

    fn branch_rules_url(&self, repo: &RepoRef) -> Option<String> {
        Some(rules::branch_rules_url(self, repo))
    }

    fn pr_url(&self, repo: &RepoRef, number: u64) -> String {
        pulls::pr_url(self, repo, number)
    }
}

impl AppCalls for GithubApp {
    fn recent_deliveries(
        &self,
        since: OffsetDateTime,
        _class: JobClass,
    ) -> ScmFuture<'_, Vec<RecentDelivery>> {
        Box::pin(apps::recent_deliveries(self, since))
    }

    fn redeliver<'a>(&'a self, attempt_id: &'a str, _class: JobClass) -> ScmFuture<'a, ()> {
        Box::pin(apps::redeliver(self, attempt_id))
    }

    fn installation<'a>(
        &'a self,
        external_id: &'a str,
        _class: JobClass,
    ) -> ScmFuture<'a, Option<ScmInstallation>> {
        Box::pin(apps::installation(self, external_id))
    }

    fn installations(&self, _class: JobClass) -> ScmFuture<'_, Vec<ScmInstallation>> {
        Box::pin(apps::installations(self))
    }

    fn list_repos<'a>(
        &'a self,
        connection: &'a ConnectionRef,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<ScmRepo>> {
        Box::pin(apps::list_repos(self, connection, class))
    }

    fn members<'a>(
        &'a self,
        connection: &'a ConnectionRef,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<ScmMember>> {
        Box::pin(people::members(self, connection, class))
    }

    fn account_profile<'a>(
        &'a self,
        connection: &'a ConnectionRef,
        class: JobClass,
    ) -> ScmFuture<'a, ScmAccountProfile> {
        Box::pin(people::account_profile(self, connection, class))
    }

    fn app_install_url(&self, _class: JobClass) -> ScmFuture<'_, String> {
        Box::pin(apps::app_install_url(self))
    }
}

impl PersonCalls for GithubApp {
    fn sign_in_url(&self, request: &SignInRequest) -> Option<String> {
        people::sign_in_url(self, request)
    }

    fn sign_in<'a>(
        &'a self,
        code: &'a SignInCode,
        _class: JobClass,
    ) -> ScmFuture<'a, (SignedIn, ScmPersonToken)> {
        Box::pin(people::sign_in(self, code))
    }

    fn refresh_person_token<'a>(
        &'a self,
        refresh: &'a SecretString,
        _class: JobClass,
    ) -> ScmFuture<'a, ScmPersonToken> {
        Box::pin(people::refresh_person_token(self, refresh))
    }

    fn person_reach<'a>(
        &'a self,
        token: &'a SecretString,
        _class: JobClass,
    ) -> ScmFuture<'a, ScmPersonReach> {
        Box::pin(people::person_reach(self, token))
    }

    fn person_permission<'a>(
        &'a self,
        token: &'a SecretString,
        repo: &'a RepoRef,
        _class: JobClass,
    ) -> ScmFuture<'a, Permission> {
        Box::pin(people::person_permission(self, token, repo))
    }

    fn person_role<'a>(
        &'a self,
        token: &'a SecretString,
        person: &'a ScmUser,
        account: &'a ScmAccount,
        _class: JobClass,
    ) -> ScmFuture<'a, Option<MembershipRole>> {
        Box::pin(people::person_role(self, token, person, account))
    }
}

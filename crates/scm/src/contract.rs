//! The contract suite of the `Scm` trait (lot 4): one sequence of calls,
//! with the answers that every adapter must give. It runs against the
//! simulator, and against the recorded answers of each SCM. The world it
//! reads is described by a [`Fixture`]; [`crate::sim::contract_world`] builds
//! it in the simulator.
#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code: an answer outside the contract fails the test"
)]

use cihub_domain::{
    CheckKind, JobClass, MembershipRole, MergeMethod, Mergeability, Permission, PrState,
    ReviewDecision,
};

use crate::{
    CommitStatus, MergeIntoRef, PrMerge, RepoRef, RequiredChecks, Scm, ScmError, ScmMember,
    ScmUser, SetupFinding, StatusUpdate,
};

/// What the repository of the contract holds before the suite runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fixture {
    pub base_branch: String,
    /// What the rules of the base branch require.
    pub required: RequiredChecks,
    /// A file of the base branch, and its content.
    pub file: (String, String),
    /// An open PR, approved, with its required checks passed.
    pub mergeable_pr: u64,
    /// An open PR that conflicts with the base branch.
    pub conflicting_pr: u64,
    /// An open draft PR.
    pub draft_pr: u64,
    /// A PR number that does not exist.
    pub missing_pr: u64,
    /// A user with the permission `admin` on the repository, and an owner of
    /// the account of the connection.
    pub admin: ScmUser,
    /// A branch that does not exist yet, that the suite creates and deletes.
    pub work_branch: String,
}

const CLASS: JobClass = JobClass::Interactive;

/// Runs the whole suite on `repo` through `scm`. The suite merges the
/// mergeable PR at its end.
///
/// # Panics
///
/// When an answer is not the one of the contract.
pub async fn run(scm: &dyn Scm, repo: &RepoRef, fixture: &Fixture) {
    connection(scm, repo, fixture).await;
    installations(scm, repo).await;
    reads(scm, repo, fixture).await;
    branches(scm, repo, fixture).await;
    statuses_comments_and_labels(scm, repo, fixture).await;
    draft_prs(scm, repo, fixture).await;
    merge(scm, repo, fixture).await;
}

/// The account of the connection of `repo`, and its repositories.
async fn connection(scm: &dyn Scm, repo: &RepoRef, fixture: &Fixture) {
    let installation = scm
        .installation(&repo.connection.external_id, CLASS)
        .await
        .unwrap()
        .expect("the SCM knows the installation of the connection");

    assert_eq!(
        installation.account.path, repo.namespace,
        "the connection serves the account of the repository"
    );
    assert_ne!(installation.account.external_id, "");
    assert!(!installation.permissions.is_empty());
    assert!(!installation.is_suspended);
    let findings = scm
        .check_setup(repo, &fixture.base_branch, CLASS)
        .await
        .unwrap();

    assert_eq!(
        findings,
        [SetupFinding::UpdateRestrictionMissing],
        "the rules of the base branch do not restrict its updates"
    );

    let repos = scm.list_repos(&repo.connection, CLASS).await.unwrap();
    let listed = repos
        .iter()
        .find(|listed| listed.namespace == repo.namespace && listed.name == repo.name)
        .expect("the connection lists the repository");

    assert_ne!(listed.external_id, "");
    assert_eq!(listed.default_branch, fixture.base_branch);
}

/// The installations of the App list the installation of the connection of
/// `repo` as its own read gives it.
async fn installations(scm: &dyn Scm, repo: &RepoRef) {
    let installation = scm
        .installation(&repo.connection.external_id, CLASS)
        .await
        .unwrap()
        .expect("the SCM knows the installation of the connection");
    let listed = scm.installations(CLASS).await.unwrap();
    assert!(
        listed.contains(&installation),
        "the installations of the App list {installation:?}: {listed:?}"
    );
}

/// The members of the account of the connection of `repo` list the admin of
/// the fixture as `admin`. Outside [`run`] until the recorded answers of each
/// SCM hold the call.
///
/// # Panics
///
/// When an answer is not the one of the contract.
pub async fn members(scm: &dyn Scm, repo: &RepoRef, fixture: &Fixture) {
    let members = scm.members(&repo.connection, CLASS).await.unwrap();
    let admin = ScmMember {
        user: fixture.admin.clone(),
        role: MembershipRole::Admin,
    };

    assert!(
        members.contains(&admin),
        "the members list {admin:?}: {members:?}"
    );
}

/// The SCM gives a page that installs the App. Outside [`run`] until the
/// recorded answers of each SCM hold the call.
///
/// # Panics
///
/// When an answer is not the one of the contract.
pub async fn install_page(scm: &dyn Scm) {
    let url = scm.app_install_url(CLASS).await.unwrap();

    assert!(
        !url.is_empty(),
        "the SCM gives the page that installs the App"
    );
}

async fn reads(scm: &dyn Scm, repo: &RepoRef, fixture: &Fixture) {
    let numbers = [
        fixture.mergeable_pr,
        fixture.conflicting_pr,
        fixture.draft_pr,
        fixture.missing_pr,
    ];
    let prs = scm.pull_requests(repo, &numbers, CLASS).await.unwrap();
    let found: Vec<u64> = prs.iter().map(|pr| pr.number).collect();

    assert_eq!(
        found,
        [
            fixture.mergeable_pr,
            fixture.conflicting_pr,
            fixture.draft_pr
        ],
        "a missing PR is absent from a read of PRs"
    );

    let (mergeable, conflicting, draft) = (&prs[0], &prs[1], &prs[2]);

    assert_eq!(mergeable.state, PrState::Open);
    assert_eq!(mergeable.base_branch, fixture.base_branch);
    assert_eq!(mergeable.mergeability, Mergeability::Mergeable);
    assert_eq!(mergeable.review_decision, ReviewDecision::Approved);
    assert!(!mergeable.draft);
    assert!(mergeable.changed_files > 0);
    assert_ne!(mergeable.head_branch, "");
    assert!(!mergeable.from_fork);
    assert!(
        !mergeable.approved_by.is_empty(),
        "an approved PR names who approved it"
    );
    assert!(mergeable.changes_requested_by.is_empty());
    assert_eq!(conflicting.mergeability, Mergeability::Conflict);
    assert!(draft.draft);
    assert_eq!(draft.mergeability, Mergeability::Blocked);

    let one = scm
        .pull_request(repo, fixture.mergeable_pr, CLASS)
        .await
        .unwrap();

    assert_eq!(
        &one, mergeable,
        "one read gives the same PR as a read of PRs"
    );

    let files = scm
        .pr_files(repo, fixture.mergeable_pr, CLASS)
        .await
        .unwrap();

    assert_eq!(
        u64::try_from(files.len()).unwrap(),
        mergeable.changed_files,
        "the files of a PR are its changed files"
    );

    let base = scm
        .branch_head(repo, &fixture.base_branch, CLASS)
        .await
        .unwrap()
        .expect("the base branch exists");

    assert!(
        scm.is_ancestor(repo, &base, &mergeable.head_sha, CLASS)
            .await
            .unwrap(),
        "the mergeable PR contains the head of its base"
    );
    assert!(
        !scm.is_ancestor(repo, &mergeable.head_sha, &base, CLASS)
            .await
            .unwrap(),
        "the base does not contain the PR"
    );

    let threads = scm
        .unresolved_threads(repo, fixture.mergeable_pr, CLASS)
        .await
        .unwrap();

    assert!(threads.is_empty(), "{threads:?}");
    let missing = scm.pull_request(repo, fixture.missing_pr, CLASS).await;

    assert!(matches!(missing, Err(ScmError::NotFound(_))), "{missing:?}");

    let required = scm
        .required_checks(repo, &fixture.base_branch, CLASS)
        .await
        .unwrap();

    assert_eq!(required, fixture.required);

    let (path, content) = &fixture.file;
    let file = scm.file(repo, &fixture.base_branch, path, CLASS).await;

    assert_eq!(file.unwrap().as_ref(), Some(content));
    let none = scm
        .file(repo, &fixture.base_branch, "cihub-contract-missing", CLASS)
        .await;

    assert_eq!(none.unwrap(), None);

    let admin = scm.permission(repo, &fixture.admin, CLASS).await.unwrap();

    assert_eq!(admin, Permission::Admin);
}

async fn branches(scm: &dyn Scm, repo: &RepoRef, fixture: &Fixture) {
    let work = fixture.work_branch.as_str();
    let base = scm
        .branch_head(repo, &fixture.base_branch, CLASS)
        .await
        .unwrap()
        .expect("the base branch exists");

    assert_eq!(scm.branch_head(repo, work, CLASS).await.unwrap(), None);
    for _ in 0..2 {
        scm.set_branch(repo, work, &base, false, CLASS)
            .await
            .unwrap();
    }

    assert_eq!(
        scm.branch_head(repo, work, CLASS).await.unwrap().as_ref(),
        Some(&base)
    );

    let mergeable = pr_head(scm, repo, fixture.mergeable_pr).await;
    let merged = scm
        .merge_into_branch(repo, work, &mergeable, "contract merge", CLASS)
        .await
        .unwrap();
    let MergeIntoRef::Merged { sha } = merged else {
        panic!("a new commit merges into a new commit: {merged:?}");
    };

    assert_eq!(scm.branch_head(repo, work, CLASS).await.unwrap(), Some(sha));
    let again = scm
        .merge_into_branch(repo, work, &mergeable, "contract merge", CLASS)
        .await
        .unwrap();

    assert_eq!(again, MergeIntoRef::AlreadyContained);
    let conflicting = pr_head(scm, repo, fixture.conflicting_pr).await;
    let conflict = scm
        .merge_into_branch(repo, work, &conflicting, "contract conflict", CLASS)
        .await
        .unwrap();

    assert_eq!(conflict, MergeIntoRef::Conflict);

    let behind = scm.set_branch(repo, work, &base, false, CLASS).await;

    assert!(
        matches!(behind, Err(ScmError::Validation(_))),
        "a move back without force is refused: {behind:?}"
    );

    scm.set_branch(repo, work, &base, true, CLASS)
        .await
        .unwrap();

    for _ in 0..2 {
        scm.delete_branch(repo, work, CLASS).await.unwrap();
    }

    assert_eq!(scm.branch_head(repo, work, CLASS).await.unwrap(), None);
}

async fn statuses_comments_and_labels(scm: &dyn Scm, repo: &RepoRef, fixture: &Fixture) {
    let head = pr_head(scm, repo, fixture.mergeable_pr).await;
    let status = StatusUpdate {
        sha: head.clone(),
        context: "cihub-contract".into(),
        status: CommitStatus::Pending,
        description: "contract".into(),
        details_url: None,
    };
    for _ in 0..2 {
        scm.set_status(repo, &status, CLASS).await.unwrap();
    }

    let checks = scm.checks(repo, &head, CLASS).await.unwrap();
    let published: Vec<_> = checks
        .iter()
        .filter(|check| check.name == status.context && check.kind == CheckKind::Status)
        .collect();

    assert_eq!(published.len(), 1, "{checks:?}");
    assert_eq!(published[0].conclusion, None);
    let success = StatusUpdate {
        status: CommitStatus::Success,
        ..status
    };
    scm.set_status(repo, &success, CLASS).await.unwrap();

    let number = fixture.mergeable_pr;
    let comment = scm
        .create_comment(repo, number, "contract comment", CLASS)
        .await
        .unwrap();
    scm.update_comment(repo, &comment, "contract comment, updated", CLASS)
        .await
        .unwrap();

    for _ in 0..2 {
        scm.add_label(repo, number, "cihub-contract", CLASS)
            .await
            .unwrap();
    }

    let labelled = scm.pull_request(repo, number, CLASS).await.unwrap();

    assert!(labelled.labels.contains("cihub-contract"));
    for _ in 0..2 {
        scm.remove_label(repo, number, "cihub-contract", CLASS)
            .await
            .unwrap();
    }

    let unlabelled = scm.pull_request(repo, number, CLASS).await.unwrap();

    assert!(!unlabelled.labels.contains("cihub-contract"));
}

async fn draft_prs(scm: &dyn Scm, repo: &RepoRef, fixture: &Fixture) {
    let work = fixture.work_branch.as_str();
    let head = pr_head(scm, repo, fixture.mergeable_pr).await;
    scm.set_branch(repo, work, &head, true, CLASS)
        .await
        .unwrap();

    let base = fixture.base_branch.as_str();
    let number = scm
        .open_draft_pr(repo, work, base, "contract draft", "contract", CLASS)
        .await
        .unwrap();
    let again = scm
        .open_draft_pr(repo, work, base, "contract draft", "contract", CLASS)
        .await
        .unwrap();

    assert_eq!(again, number, "an open draft PR is not opened twice");
    let draft = scm.pull_request(repo, number, CLASS).await.unwrap();

    assert!(draft.draft);
    assert_eq!(
        (draft.title.as_str(), draft.body.as_str()),
        ("contract draft", "contract")
    );

    for _ in 0..2 {
        scm.close_pr(repo, number, CLASS).await.unwrap();
    }

    let closed = scm.pull_request(repo, number, CLASS).await.unwrap();

    assert_eq!(closed.state, PrState::Closed);
    scm.delete_branch(repo, work, CLASS).await.unwrap();
}

async fn merge(scm: &dyn Scm, repo: &RepoRef, fixture: &Fixture) {
    let head = pr_head(scm, repo, fixture.mergeable_pr).await;
    let mut merge = PrMerge {
        number: fixture.mergeable_pr,
        head_sha: pr_head(scm, repo, fixture.conflicting_pr).await,
        tested_sha: String::new(),
        method: MergeMethod::Squash,
        title: "contract merge".into(),
        message: String::new(),
    };
    let changed = scm.merge_pr(repo, &merge, CLASS).await;

    assert_eq!(changed, Err(ScmError::HeadChanged));
    merge.head_sha = head;
    let merged = scm.merge_pr(repo, &merge, CLASS).await.unwrap();
    let again = scm.merge_pr(repo, &merge, CLASS).await.unwrap();

    assert_eq!(again, merged, "a merged PR returns its commit");
    let base = scm
        .branch_head(repo, &fixture.base_branch, CLASS)
        .await
        .unwrap();

    assert_eq!(base, Some(merged));
    let pr = scm
        .pull_request(repo, fixture.mergeable_pr, CLASS)
        .await
        .unwrap();

    assert_eq!(pr.state, PrState::Merged);
}

async fn pr_head(scm: &dyn Scm, repo: &RepoRef, number: u64) -> String {
    scm.pull_request(repo, number, CLASS)
        .await
        .unwrap()
        .head_sha
}

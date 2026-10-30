use std::collections::BTreeSet;

use cihub_domain::{
    CheckConclusion, CheckKind, CheckReport, CheckStatus, MembershipRole, Permission,
    ReviewDecision,
};
use time::OffsetDateTime;

use super::{PrChange, Reviews, Sim};
use crate::contract::Fixture;
use crate::{RepoRef, RequiredChecks, ScmAccount, ScmError, ScmUser};

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
        status: CheckStatus::Completed,
        conclusion: Some(CheckConclusion::Success),
        details_url: None,
        started_at: Some(OffsetDateTime::UNIX_EPOCH),
        completed_at: Some(OffsetDateTime::UNIX_EPOCH + time::Duration::minutes(5)),
    };
    sim.report_check(repo, &feature, ci)?;

    let draft = sim.commit(repo, &main, &[("c.txt", Some("c\n"))])?;
    sim.push(repo, "draft", &draft, &author)?;
    let draft_pr = sim.open_pr(repo, "draft", "main", &author)?;
    sim.change_pr(repo, draft_pr, PrChange::Draft(true))?;

    let required = RequiredChecks {
        names: BTreeSet::from(["ci".to_owned()]),
        requires_up_to_date: false,
    };
    sim.set_rules(repo, "main", required.clone())?;
    let admin = ScmUser {
        external_id: "3".into(),
        login: "admin".into(),
    };
    sim.set_permission(repo, &admin, Permission::Admin)?;
    let account = ScmAccount {
        external_id: "10".into(),
        path: repo.owner.clone(),
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

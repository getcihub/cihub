use std::collections::{BTreeMap, BTreeSet};

use cihub_domain::{
    CheckConclusion, CheckKind, CheckReport, CheckStatus, ConnectionId, JobClass, MembershipRole,
    MergeMethod, Mergeability, Permission,
};
use time::OffsetDateTime;
use ulid::Ulid;

use super::{
    Fault, PrChange, RandomFaults, ScmMethod, Sim, SimCall, UpdateRestriction, WebhookFaults, bot,
    contract_world,
};
use crate::{
    AppCalls, BranchMove, ConnectionRef, MAX_CHECKS_PER_READ, PersonCalls, PrMerge, RecentDelivery,
    RepoCalls, RepoRef, RequiredChecks, ScmError, ScmEvent, ScmMembership, ScmPersonRepo,
    ScmProfile, ScmUser, SetupFinding, SignInCode, SignInRequest, code_challenge,
};

fn repo() -> RepoRef {
    RepoRef {
        connection: ConnectionRef {
            id: ConnectionId::from_ulid(Ulid(1)),
            external_id: "100".into(),
        },
        external_id: "200".into(),
        owner: "acme".into(),
        name: "widgets".into(),
    }
}

fn success(name: &str) -> CheckReport {
    CheckReport {
        name: name.into(),
        kind: CheckKind::Check,
        status: CheckStatus::Completed,
        conclusion: Some(CheckConclusion::Success),
        details_url: None,
        started_at: None,
        completed_at: None,
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

#[test]
fn a_pr_page_is_built_without_a_call() {
    let repo = repo();
    let (sim, _) = contract_world(&repo).unwrap();

    assert_eq!(
        sim.pr_url(&repo, 7),
        "https://scm.example/acme/widgets/prs/7"
    );
    assert_eq!(sim.calls().len(), 0);
}

#[test]
fn the_comments_of_a_pr_keep_their_order_past_nine() {
    let repo = repo();
    let (sim, fixture) = contract_world(&repo).unwrap();
    let bodies: Vec<String> = (1..=12).map(|n| format!("comment {n}")).collect();

    for body in &bodies {
        sim.comment(&repo, fixture.mergeable_pr, &bot(), body)
            .unwrap();
    }

    assert_eq!(sim.comments(&repo, fixture.mergeable_pr), bodies);
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
    let profile = ScmProfile {
        name: Some("Ada Lovelace".into()),
        avatar_url: Some("https://avatars.example.com/u/7".into()),
    };

    sim.set_profile(&person, profile.clone());
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
    let (signed_in, _) = sim.sign_in(&first, JobClass::Interactive).await.unwrap();
    let mut refused = Vec::new();
    for code in [&replayed, &elsewhere, &forged] {
        refused.push(sim.sign_in(code, JobClass::Interactive).await);
    }

    assert_eq!(signed_in.user, person);
    assert_eq!(signed_in.profile, profile);
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

    assert_eq!(members, []);

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
        call: Some(ScmMethod::PullRequests),
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
            method: ScmMethod::BranchHead,
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
                is_ready_for_review: true,
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
async fn the_update_restriction_of_a_branch_gives_its_findings() {
    let repo = repo();
    let sim = Sim::new();
    sim.add_repo(&repo, "main", &[]);
    let findings = async |restriction| {
        sim.set_update_restriction(&repo, "main", restriction)
            .unwrap();
        sim.check_setup(&repo, "main", JobClass::Repair)
            .await
            .unwrap()
    };
    let blocked = UpdateRestriction {
        is_app_exempt: false,
        covers_test_branches: true,
    };
    let exempt = UpdateRestriction {
        is_app_exempt: true,
        covers_test_branches: false,
    };

    assert_eq!(
        findings(None).await,
        [SetupFinding::UpdateRestrictionMissing]
    );
    assert_eq!(
        findings(Some(blocked)).await,
        [
            SetupFinding::AppNotExempt,
            SetupFinding::TestBranchesNotExcluded
        ]
    );
    assert_eq!(findings(Some(exempt)).await, []);
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
        requires_up_to_date: true,
    };
    sim.set_rules(&repo, "main", rules).unwrap();

    assert_eq!(read().await, Mergeability::Behind);
    let merge = PrMerge {
        number,
        head_sha: head,
        tested_sha: String::new(),
        method: MergeMethod::Squash,
        title: "behind".into(),
        message: String::new(),
    };
    let refused = sim.merge_pr(&repo, &merge, JobClass::Merge).await;

    assert!(
        matches!(refused, Err(ScmError::MergeRejected(_))),
        "GitHub refuses the merge of a PR behind its base: {refused:?}"
    );
    sim.change_pr(
        &repo,
        number,
        PrChange::Mergeability(Some(Mergeability::Unknown)),
    )
    .unwrap();

    assert_eq!(read().await, Mergeability::Unknown);
}

#[tokio::test]
async fn an_outside_collaborator_reaches_the_installation_of_the_repository() {
    let repo = repo();
    let (sim, _) = contract_world(&repo).unwrap();
    let person = ScmUser {
        external_id: "8".into(),
        login: "grace".into(),
    };
    let request = SignInRequest {
        redirect_url: "https://ci.example.com/auth/callback".into(),
        state: "state".into(),
        code_challenge: code_challenge(&"verifier".into()),
    };
    sim.set_permission(&repo, &person, Permission::Read)
        .unwrap();
    let code = SignInCode {
        code: sim.authorize(&person, &request).into(),
        code_verifier: "verifier".into(),
        redirect_url: request.redirect_url.clone(),
    };

    let (signed_in, token) = sim.sign_in(&code, JobClass::Interactive).await.unwrap();
    let reach = sim
        .person_reach(&token.access, JobClass::Interactive)
        .await
        .unwrap();

    assert_eq!(signed_in.memberships, []);
    assert_eq!(signed_in.installations.len(), 1);
    assert_eq!(signed_in.installations, reach.installations);
    assert_eq!(
        reach.repos,
        [ScmPersonRepo {
            connection_external_id: repo.connection.external_id.clone(),
            repo_external_id: repo.external_id.clone(),
            permission: Permission::Read,
        }]
    );
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

async fn outcomes(repo: &RepoRef, faults: RandomFaults) -> Vec<Result<Option<String>, ScmError>> {
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
        lost_answer_percent: 0,
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

fn fast_forward(fixture: &crate::contract::Fixture, sha: &str) -> PrMerge {
    PrMerge {
        number: fixture.mergeable_pr,
        head_sha: sha.into(),
        tested_sha: sha.into(),
        method: MergeMethod::FastForward,
        title: String::new(),
        message: String::new(),
    }
}

#[tokio::test]
async fn a_lost_answer_leaves_the_effect_of_the_call() {
    let repo = repo();
    let (sim, fixture) = contract_world(&repo).unwrap();
    let head = sim.branch(&repo, "feature").unwrap();
    sim.lose_answer(Fault {
        call: Some(ScmMethod::MergePr),
        error: ScmError::Unavailable("502".into()),
        times: 1,
    });

    let lost = sim
        .merge_pr(&repo, &fast_forward(&fixture, &head), JobClass::Merge)
        .await;
    let again = sim
        .merge_pr(&repo, &fast_forward(&fixture, &head), JobClass::Merge)
        .await;

    assert_eq!(lost, Err(ScmError::Unavailable("502".into())));
    assert_eq!(sim.branch(&repo, "main"), Some(head.clone()));
    assert_eq!(again, Ok(head));
}

#[tokio::test]
async fn a_lost_answer_of_a_failing_call_reports_the_failure_of_the_call() {
    let repo = repo();
    let (sim, _) = contract_world(&repo).unwrap();
    sim.lose_answer(Fault {
        call: Some(ScmMethod::SetBranch),
        error: ScmError::Unavailable("502".into()),
        times: 1,
    });

    let lost = sim
        .set_branch(&repo, "main", "unknown", BranchMove::Force, JobClass::Merge)
        .await;

    assert_eq!(
        lost,
        Err(ScmError::Validation("unknown commit unknown".into()))
    );
}

#[tokio::test]
async fn random_lost_answers_only_hit_the_calls_that_change_the_scm() {
    let repo = repo();
    let (sim, fixture) = contract_world(&repo).unwrap();
    let head = sim.branch(&repo, "feature").unwrap();
    sim.fail_at_random(Some(RandomFaults {
        seed: 1,
        unavailable_percent: 0,
        rate_limited_percent: 0,
        lost_answer_percent: 100,
        rate_limited_until: OffsetDateTime::UNIX_EPOCH,
    }));

    let read = sim
        .pull_request(&repo, fixture.mergeable_pr, JobClass::Merge)
        .await;
    let lost = sim
        .merge_pr(&repo, &fast_forward(&fixture, &head), JobClass::Merge)
        .await;

    assert!(read.is_ok(), "{read:?}");
    assert!(lost.is_err(), "{lost:?}");
    assert_eq!(sim.branch(&repo, "main"), Some(head));
}

#[tokio::test]
async fn a_fast_forward_of_a_closed_pr_is_refused() {
    let repo = repo();
    let (sim, fixture) = contract_world(&repo).unwrap();
    let head = sim.branch(&repo, "feature").unwrap();
    sim.change_pr(&repo, fixture.mergeable_pr, PrChange::Close)
        .unwrap();

    let merged = sim
        .merge_pr(&repo, &fast_forward(&fixture, &head), JobClass::Merge)
        .await;

    assert_eq!(merged, Err(ScmError::MergeRejected("PR closed".into())));
}

#[tokio::test]
async fn a_fast_forward_does_not_read_the_mergeability_of_the_pr() {
    let repo = repo();
    let (sim, fixture) = contract_world(&repo).unwrap();
    let head = sim.branch(&repo, "feature").unwrap();
    sim.change_pr(
        &repo,
        fixture.mergeable_pr,
        PrChange::Mergeability(Some(Mergeability::Blocked)),
    )
    .unwrap();

    let merged = sim
        .merge_pr(&repo, &fast_forward(&fixture, &head), JobClass::Merge)
        .await;

    assert_eq!(merged, Ok(head));
}

#[tokio::test]
async fn a_stale_name_finds_no_repository() {
    let repo = repo();
    let (sim, fixture) = contract_world(&repo).unwrap();
    sim.rename_repo(&repo.external_id, "new-name");

    let stale = sim
        .pull_request(&repo, fixture.mergeable_pr, JobClass::Merge)
        .await;
    let renamed = RepoRef {
        name: "new-name".into(),
        ..repo
    };
    let current = sim
        .pull_request(&renamed, fixture.mergeable_pr, JobClass::Merge)
        .await;

    assert!(matches!(stale, Err(ScmError::NotFound(_))), "{stale:?}");
    assert!(current.is_ok(), "{current:?}");
}

#[tokio::test]
async fn a_read_of_the_checks_gives_at_most_the_number_of_the_contract() {
    let repo = repo();
    let (sim, _) = contract_world(&repo).unwrap();
    let head = sim.branch(&repo, "feature").unwrap();
    for n in 0..MAX_CHECKS_PER_READ + 50 {
        sim.report_check(&repo, &head, success(&format!("check-{n}")))
            .unwrap();
    }

    let checks = sim.checks(&repo, &head, JobClass::Merge).await.unwrap();

    assert_eq!(checks.len(), MAX_CHECKS_PER_READ);
}

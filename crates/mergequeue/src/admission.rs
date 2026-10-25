//! The `queue_conditions` of a queue: the jobs that read a PR evaluate them,
//! and give the conditions that the PR does not meet in its `pr.observed`. The
//! core sees the result, not the conditions: they need the logins, which the
//! log does not keep.

use std::time::Duration;

use cihub_conditions::{Attribute, Check, Conditions, Context};
use cihub_config::{ProfileConfig, RepoConfig};
use cihub_domain::{BranchProtectionInjection, JobClass, Mergeability, Permission, Priority};
use cihub_scm::{PrSnapshot, RequiredChecks, Scm, check_passed};
use time::OffsetDateTime;

use crate::jobs::{Failure, is_own_context};
use crate::target::QueueTarget;

/// Who submitted the PR, for `sender` and `sender_permission`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Sender {
    pub login: Option<String>,
    pub permission: Option<Permission>,
}

/// The `queue_conditions` of `defaults` and of the entry of the queue in
/// `config`, its last valid file; none without a file.
fn queue_conditions(target: &QueueTarget, config: Option<&RepoConfig>) -> Conditions {
    config
        .map(|config| config.queue_conditions(&target.target_branch))
        .unwrap_or_default()
}

/// The context of the conditions for `pr`, before its admission: the PR is
/// not in the queue yet. The statuses of CIHub are left out: a condition on
/// failed checks would otherwise fail on the status that it writes.
fn context(pr: &PrSnapshot, sender: &Sender, now: OffsetDateTime) -> Context {
    let list = |set: &std::collections::BTreeSet<String>| set.iter().cloned().collect();
    Context {
        draft: pr.draft,
        conflict: pr.mergeability == Mergeability::Conflict,
        from_fork: pr.from_fork,
        behind_base: pr.mergeability == Mergeability::Behind,
        number: pr.number,
        title: pr.title.clone(),
        body: pr.body.clone(),
        review_decision: pr.review_decision,
        approved_reviews_by: list(&pr.approved_by),
        changes_requested_reviews_by: list(&pr.changes_requested_by),
        review_requested: list(&pr.review_requested),
        review_threads_unresolved: None,
        checks: pr
            .checks
            .iter()
            .filter(|check| !is_own_context(&check.name))
            .map(|check| Check {
                name: check.name.clone(),
                conclusion: check.conclusion,
            })
            .collect(),
        labels: pr.labels.iter().cloned().collect(),
        author: pr.author.login.clone(),
        sender: sender.login.clone(),
        sender_permission: sender.permission,
        files: None,
        additions: pr.additions,
        deletions: pr.deletions,
        base: pr.base_branch.clone(),
        head: pr.head_branch.clone(),
        queue_position: None,
        queue_age: None,
        pr_age: Duration::try_from(now - pr.created_at).unwrap_or_default(),
    }
}

/// What a read of a PR gives to its conditions besides the PR.
pub(crate) struct Reading<'a> {
    pub sender: &'a Sender,
    /// The checks that the rules of the target branch require, and the
    /// statuses of CIHub, which they may require.
    pub rules: &'a RequiredChecks,
    /// The `branch_protection_injection` of the settings of the queue.
    pub injection: BranchProtectionInjection,
    pub class: JobClass,
    pub now: OffsetDateTime,
}

/// The `queue_conditions` of the queue of `target` that `pr` does not meet, as `config`, the last valid file
/// of the queue, writes them, then, with `branch_protection_injection: both`, each check that the rules
/// require and that has not passed on the head of the PR, as `checks.success: <name>`. The files and the
/// review threads are read only when a condition uses them.
pub(crate) async fn unmet_conditions(
    scm: &dyn Scm,
    target: &QueueTarget,
    config: Option<&RepoConfig>,
    pr: &PrSnapshot,
    reading: &Reading<'_>,
) -> Result<Vec<String>, Failure> {
    let mut unmet = file_conditions(scm, target, config, pr, reading).await?;
    unmet.extend(injected(pr, reading));
    Ok(unmet)
}

/// With `branch_protection_injection: both`, each check that the rules
/// require and that has not passed on the head of the PR, as
/// `checks.success: <name>`.
fn injected(pr: &PrSnapshot, reading: &Reading<'_>) -> Vec<String> {
    if reading.injection != BranchProtectionInjection::Both {
        return Vec::new();
    }

    reading
        .rules
        .names
        .iter()
        .filter(|name| !is_own_context(name) && !check_passed(pr, name))
        .map(|name| format!("checks.success: {name}"))
        .collect()
}

async fn file_conditions(
    scm: &dyn Scm,
    target: &QueueTarget,
    config: Option<&RepoConfig>,
    pr: &PrSnapshot,
    reading: &Reading<'_>,
) -> Result<Vec<String>, Failure> {
    let conditions = queue_conditions(target, config);
    if conditions.0.is_empty() {
        return Ok(Vec::new());
    }

    let ctx = loaded_context(scm, target, pr, reading, |a| conditions.uses(a)).await?;
    conditions
        .unmet(&ctx)
        .map_err(|err| Failure::Never(err.into()))
}

/// The context of `pr`, with the attributes loaded on demand that `uses`
/// asks for.
async fn loaded_context(
    scm: &dyn Scm,
    target: &QueueTarget,
    pr: &PrSnapshot,
    reading: &Reading<'_>,
    uses: impl Fn(Attribute) -> bool,
) -> Result<Context, Failure> {
    let class = reading.class;
    let mut ctx = context(pr, reading.sender, reading.now);

    if uses(Attribute::Files) {
        ctx.files = Some(scm.pr_files(&target.repo, pr.number, class).await?);
    }

    if uses(Attribute::ReviewThreadsUnresolved) {
        ctx.review_threads_unresolved = Some(
            scm.unresolved_threads(&target.repo, pr.number, class)
                .await?,
        );
    }

    Ok(ctx)
}

/// What the file gives to a submission of `pr`, from `config`, the last valid
/// file of the queue that it enters.
pub(crate) struct Choices {
    /// The `queue_conditions` that the PR does not meet, as
    /// [`unmet_conditions`] gives them.
    pub unmet: Vec<String>,
    /// The priority of the first rule of `priority_rules` that the PR meets.
    pub priority: Option<Priority>,
    /// The first profile of the queue whose `conditions` the PR meets; a
    /// profile without conditions is chosen only by its name.
    pub profile: Option<String>,
}

/// The conditions, the priority and the profile of a submission of `pr`. The
/// files and the review threads are read once, when one of them uses them.
pub(crate) async fn submission_choices(
    scm: &dyn Scm,
    target: &QueueTarget,
    config: Option<&RepoConfig>,
    pr: &PrSnapshot,
    reading: &Reading<'_>,
) -> Result<Choices, Failure> {
    let conditions = queue_conditions(target, config);
    let rules = config.map_or(&[][..], |config| &config.priority_rules[..]);
    let profiles: Vec<&ProfileConfig> = config
        .and_then(|config| config.queue(&target.target_branch))
        .into_iter()
        .flat_map(|queue| &queue.profiles)
        .filter(|profile| !profile.conditions.0.is_empty())
        .collect();
    let uses = |a| {
        conditions.uses(a)
            || rules.iter().any(|rule| rule.conditions.uses(a))
            || profiles.iter().any(|profile| profile.conditions.uses(a))
    };
    let ctx = loaded_context(scm, target, pr, reading, uses).await?;

    let mut unmet = conditions
        .unmet(&ctx)
        .map_err(|err| Failure::Never(err.into()))?;
    unmet.extend(injected(pr, reading));
    let priority = first_met(rules, &ctx, |rule| &rule.conditions)?.map(|rule| rule.priority);
    let profile = first_met(profiles, &ctx, |profile| &profile.conditions)?
        .map(|profile| profile.name.clone());

    Ok(Choices {
        unmet,
        priority,
        profile,
    })
}

/// The first of `items` whose conditions hold on `ctx`.
fn first_met<'a, T: 'a>(
    items: impl IntoIterator<Item = &'a T>,
    ctx: &Context,
    conditions: impl Fn(&T) -> &Conditions,
) -> Result<Option<&'a T>, Failure> {
    for item in items {
        if conditions(item)
            .eval(ctx)
            .map_err(|err| Failure::Never(err.into()))?
        {
            return Ok(Some(item));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use cihub_domain::ReviewDecision;
    use cihub_scm::sim::Sim;

    use super::*;
    use crate::target::{test_pr, test_target};

    async fn unmet(injection: &str, checks: &[(&str, bool)]) -> Vec<String> {
        let mut target = test_target();
        target.config_text = Some(format!(
            "version: 1\nqueues:\n  - branch: main\n    \
             branch_protection_injection: {injection}\n    queue_conditions:\n      labels: ready\n      author: alice\n"
        ));

        let rules = RequiredChecks {
            names: BTreeSet::from([
                "ci".to_owned(),
                "lint".to_owned(),
                crate::jobs::queue_status_context("main"),
            ]),
            up_to_date: false,
        };
        let config = target.config().unwrap();
        let settings = target.settings_in(config.as_ref()).unwrap();
        let reading = Reading {
            sender: &Sender::default(),
            rules: &rules,
            injection: settings.branch_protection_injection.value,
            class: JobClass::Repair,
            now: OffsetDateTime::UNIX_EPOCH,
        };
        let mut pr = test_pr(Mergeability::Blocked, ReviewDecision::Approved, checks);
        pr.labels.insert("ready".to_owned());

        let result = unmet_conditions(&Sim::new(), &target, config.as_ref(), &pr, &reading).await;

        result.unwrap_or_else(|_| panic!("the conditions evaluate"))
    }

    #[tokio::test]
    async fn a_submission_reads_the_files_once_for_its_three_choices() {
        let mut target = test_target();
        target.config_text = Some(
            "version: 1
queues:
  - branch: main
    queue_conditions:
      files:
        any_match: src/**
    profiles:
      - name: docs
        conditions:
          files:
            any_match: docs/**
      - name: core
        conditions:
          files:
            any_match: src/**
priority_rules:
  - name: core
    conditions:
      files:
        any_match: src/**
    priority: high
"
            .to_owned(),
        );
        let sim = Sim::new();
        let author = cihub_scm::ScmUser {
            external_id: "1".into(),
            login: "octocat".into(),
        };
        let main = sim.add_repo(&target.repo, "main", &[("a.txt", "a")]);
        let head = sim
            .commit(&target.repo, &main, &[("src/lib.rs", Some("x"))])
            .unwrap();
        sim.push(&target.repo, "feat", &head, &author).unwrap();
        let mut pr = test_pr(Mergeability::Mergeable, ReviewDecision::Approved, &[]);
        pr.number = sim.open_pr(&target.repo, "feat", "main", &author).unwrap();
        let rules = RequiredChecks {
            names: BTreeSet::new(),
            up_to_date: false,
        };
        let reading = Reading {
            sender: &Sender::default(),
            rules: &rules,
            injection: BranchProtectionInjection::Merge,
            class: JobClass::Repair,
            now: OffsetDateTime::UNIX_EPOCH,
        };
        let config = target.config().unwrap();

        let choices = submission_choices(&sim, &target, config.as_ref(), &pr, &reading)
            .await
            .unwrap_or_else(|_| panic!("the conditions evaluate"));

        assert_eq!(choices.unmet, [] as [String; 0]);
        assert_eq!(choices.priority, Some(Priority::HIGH));
        assert_eq!(choices.profile.as_deref(), Some("core"));
        let reads = sim
            .calls()
            .iter()
            .filter(|call| call.name == "pr_files")
            .count();
        assert_eq!(reads, 1);
    }

    #[tokio::test]
    async fn the_injection_names_the_required_checks_that_did_not_pass() {
        assert_eq!(
            unmet("both", &[("ci", true), ("lint", false)]).await,
            ["author: alice", "checks.success: lint"]
        );
        assert_eq!(
            unmet("merge", &[("ci", true), ("lint", false)]).await,
            ["author: alice"]
        );
        assert_eq!(unmet("none", &[]).await, ["author: alice"]);
    }

    #[tokio::test]
    async fn the_conditions_do_not_see_the_statuses_of_cihub() {
        let mut target = test_target();
        target.config_text = Some(
            "version: 1
queues:
  - branch: main
    queue_conditions:
      checks:
        failure:
          count:
            max: 0
"
            .to_owned(),
        );
        let config = target.config().unwrap();
        let reading = Reading {
            sender: &Sender::default(),
            rules: &RequiredChecks::default(),
            injection: BranchProtectionInjection::Merge,
            class: JobClass::Repair,
            now: OffsetDateTime::UNIX_EPOCH,
        };
        let own = crate::jobs::conditions_status_context("main");
        let own_failed = test_pr(
            Mergeability::Mergeable,
            ReviewDecision::Approved,
            &[(&own, false)],
        );
        let ci_failed = test_pr(
            Mergeability::Mergeable,
            ReviewDecision::Approved,
            &[("ci", false)],
        );

        let unmet_of = async |pr| {
            unmet_conditions(&Sim::new(), &target, config.as_ref(), pr, &reading)
                .await
                .unwrap_or_else(|_| panic!("the conditions evaluate"))
        };

        assert_eq!(unmet_of(&own_failed).await, [] as [String; 0]);
        assert_eq!(unmet_of(&ci_failed).await.len(), 1);
    }
}

//! The `queue_conditions` of a queue: the jobs that read a PR evaluate them,
//! and give the conditions that the PR does not meet in its `pr.observed`. The
//! core sees the result, not the conditions: they need the logins, which the
//! log does not keep.

use std::time::Duration;

use cihub_conditions::{Attribute, Check, Conditions, Context};
use cihub_config::RepoConfig;
use cihub_domain::{BranchProtectionInjection, JobClass, Mergeability, Permission, Priority};
use cihub_scm::{PrSnapshot, RequiredChecks, Scm, check_passed};
use time::OffsetDateTime;

use crate::jobs::Failure;
use crate::target::QueueTarget;

/// Who submitted the PR, for `sender` and `sender-permission`.
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
/// not in the queue yet.
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
    /// status of the queue, which they may require.
    pub rules: &'a RequiredChecks,
    pub own_status: &'a str,
    pub class: JobClass,
    pub now: OffsetDateTime,
}

/// The `queue_conditions` of the queue of `target` that `pr` does not meet, as `config`, the last valid file
/// of the queue, writes them, then, with `branch_protection_injection: both`, each check that the rules
/// require and that has not passed on the head of the PR, as `check-success = <name>`. The files and the
/// review threads are read only when a condition uses them.
pub(crate) async fn unmet_conditions(
    scm: &dyn Scm,
    target: &QueueTarget,
    config: Option<&RepoConfig>,
    pr: &PrSnapshot,
    reading: &Reading<'_>,
) -> Result<Vec<String>, Failure> {
    let mut unmet = file_conditions(scm, target, config, pr, reading).await?;
    let injection = target
        .settings_in(config)?
        .branch_protection_injection
        .value;

    if injection == BranchProtectionInjection::Both {
        unmet.extend(
            reading
                .rules
                .names
                .iter()
                .filter(|name| *name != reading.own_status && !check_passed(pr, name))
                .map(|name| format!("check-success = {name}")),
        );
    }

    Ok(unmet)
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
        .map_err(|err| Failure::Never(err.to_string()))
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

/// The priority of the first rule of `priority_rules` that `pr` meets, in
/// `config`, the last valid file of the queue that it enters.
pub(crate) async fn rule_priority(
    scm: &dyn Scm,
    target: &QueueTarget,
    config: Option<&RepoConfig>,
    pr: &PrSnapshot,
    reading: &Reading<'_>,
) -> Result<Option<Priority>, Failure> {
    let Some(config) = config else {
        return Ok(None);
    };

    let rules = &config.priority_rules;
    if rules.is_empty() {
        return Ok(None);
    }

    let uses = |a| rules.iter().any(|rule| rule.conditions.uses(a));
    let ctx = loaded_context(scm, target, pr, reading, uses).await?;

    for rule in rules {
        if rule
            .conditions
            .eval(&ctx)
            .map_err(|err| Failure::Never(err.to_string()))?
        {
            return Ok(Some(rule.priority));
        }
    }

    Ok(None)
}

/// The first profile of the queue in `config` whose `conditions` `pr` meets.
/// A profile without conditions is chosen only by its name.
pub(crate) async fn rule_profile(
    scm: &dyn Scm,
    target: &QueueTarget,
    config: Option<&RepoConfig>,
    pr: &PrSnapshot,
    reading: &Reading<'_>,
) -> Result<Option<String>, Failure> {
    let profiles: Vec<_> = config
        .and_then(|config| config.queue(&target.target_branch))
        .into_iter()
        .flat_map(|queue| &queue.profiles)
        .filter(|profile| !profile.conditions.0.is_empty())
        .collect();
    if profiles.is_empty() {
        return Ok(None);
    }

    let uses = |a| profiles.iter().any(|profile| profile.conditions.uses(a));
    let ctx = loaded_context(scm, target, pr, reading, uses).await?;

    for profile in profiles {
        if profile
            .conditions
            .eval(&ctx)
            .map_err(|err| Failure::Never(err.to_string()))?
        {
            return Ok(Some(profile.name.clone()));
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
             branch_protection_injection: {injection}\n    queue_conditions:\n      - label = ready\n      - author = alice\n"
        ));

        let rules = RequiredChecks {
            names: BTreeSet::from(["ci".to_owned(), "lint".to_owned(), "own".to_owned()]),
            up_to_date: false,
        };
        let reading = Reading {
            sender: &Sender::default(),
            rules: &rules,
            own_status: "own",
            class: JobClass::Repair,
            now: OffsetDateTime::UNIX_EPOCH,
        };
        let mut pr = test_pr(Mergeability::Blocked, ReviewDecision::Approved, checks);
        pr.labels.insert("ready".to_owned());

        let config = target.config().unwrap();
        let result = unmet_conditions(&Sim::new(), &target, config.as_ref(), &pr, &reading).await;

        result.unwrap_or_else(|_| panic!("the conditions evaluate"))
    }

    #[tokio::test]
    async fn the_injection_names_the_required_checks_that_did_not_pass() {
        assert_eq!(
            unmet("both", &[("ci", true), ("lint", false)]).await,
            ["author = alice", "check-success = lint"]
        );
        assert_eq!(
            unmet("merge", &[("ci", true), ("lint", false)]).await,
            ["author = alice"]
        );
        assert_eq!(unmet("none", &[]).await, ["author = alice"]);
    }
}

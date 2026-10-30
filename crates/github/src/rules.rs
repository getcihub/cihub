//! The rules of a branch: the checks that they require, and the setup
//! that CIHub needs from them.

use std::collections::BTreeSet;

use cihub_domain::JobClass;
use cihub_scm::{RepoRef, RequiredChecks, ScmError, SetupFinding};
use reqwest::{Method, StatusCode};
use serde::Deserialize;
use serde_json::Value;

use crate::GithubApp;
use crate::client::{Answer, PER_PAGE, encode};

/// GitHub Free offers no branch protection and no ruleset on a private
/// repository: it answers 403 with an offer to upgrade. Such a repository has
/// no rules, and the configuration checker reports it.
fn needs_paid_plan(answer: &Answer) -> bool {
    answer.status == StatusCode::FORBIDDEN && answer.message().contains("Upgrade to GitHub")
}

/// A rule that applies to a branch.
#[derive(Deserialize)]
struct BranchRule {
    #[serde(rename = "type")]
    kind: String,
    ruleset_id: Option<u64>,
    #[serde(default)]
    parameters: Value,
}

/// The classic branch protection of a branch, next to its rules.
#[derive(Deserialize)]
struct Protection {
    #[serde(default)]
    strict: bool,
    #[serde(default)]
    contexts: Vec<String>,
}

/// The rules that apply to `branch`; none on a plan that has no rules.
async fn branch_rules(
    app: &GithubApp,
    repo: &RepoRef,
    branch: &str,
    class: JobClass,
) -> Result<Vec<BranchRule>, ScmError> {
    let rest = format!("/rules/branches/{}?per_page={PER_PAGE}", encode(branch));
    let answer = app.rest(repo, class, Method::GET, &rest, None).await?;
    if needs_paid_plan(&answer) {
        return Ok(Vec::new());
    }

    answer.expect_status(&[StatusCode::OK])?.json()
}

/// What `rules` and the classic protection of `branch` require.
async fn required_checks_of(
    app: &GithubApp,
    repo: &RepoRef,
    branch: &str,
    class: JobClass,
    rules: &[BranchRule],
) -> Result<RequiredChecks, ScmError> {
    let mut required = RequiredChecks::default();
    for rule in rules {
        if rule.kind != "required_status_checks" {
            continue;
        }

        let checks = rule.parameters["required_status_checks"].as_array();
        let contexts = checks
            .into_iter()
            .flatten()
            .filter_map(|check| check["context"].as_str());
        required.names.extend(contexts.map(str::to_owned));

        required.requires_up_to_date |= rule.parameters["strict_required_status_checks_policy"]
            .as_bool()
            .unwrap_or(false);
    }

    let rest = format!(
        "/branches/{}/protection/required_status_checks",
        encode(branch)
    );
    let classic = app.rest(repo, class, Method::GET, &rest, None).await?;
    if classic.status != StatusCode::NOT_FOUND && !needs_paid_plan(&classic) {
        let protection: Protection = classic.expect_status(&[StatusCode::OK])?.json()?;
        required.names.extend(protection.contexts);
        required.requires_up_to_date |= protection.strict;
    }

    Ok(required)
}

/// The checks that the rules of `branch` require.
pub(crate) async fn required_checks(
    app: &GithubApp,
    repo: &RepoRef,
    branch: &str,
    class: JobClass,
) -> Result<RequiredChecks, ScmError> {
    let rules = branch_rules(app, repo, branch, class).await?;
    required_checks_of(app, repo, branch, class, &rules).await
}

/// What the rules of `branch` and of the test branches miss for CIHub.
pub(crate) async fn check_setup(
    app: &GithubApp,
    repo: &RepoRef,
    branch: &str,
    class: JobClass,
) -> Result<Vec<SetupFinding>, ScmError> {
    let rules = branch_rules(app, repo, branch, class).await?;
    let update = update_finding(app, repo, class, &rules).await?;
    let test_branches = test_branches_finding(app, repo, class).await?;
    let required = required_checks_of(app, repo, branch, class, &rules).await?;
    let up_to_date = required
        .requires_up_to_date
        .then_some(SetupFinding::UpToDateRequired);

    Ok([update, test_branches, up_to_date]
        .into_iter()
        .flatten()
        .collect())
}

/// A ruleset, with the actors that may bypass it.
#[derive(Deserialize)]
struct Ruleset {
    #[serde(default)]
    bypass_actors: Vec<Exemption>,
}

/// An actor that may bypass a ruleset.
#[derive(Deserialize)]
struct Exemption {
    actor_id: Option<u64>,
    actor_type: String,
    bypass_mode: String,
}

impl Ruleset {
    /// Whether the ruleset always lets the App `app_id` through.
    fn exempts(&self, app_id: &str) -> bool {
        self.bypass_actors.iter().any(|actor| {
            actor.actor_type == "Integration"
                && actor.actor_id.map(|id| id.to_string()).as_deref() == Some(app_id)
                && actor.bypass_mode == "exempt"
        })
    }
}

/// `UpdateRestrictionMissing` when no ruleset restricts the updates of the
/// branch, `AppNotExempt` when one of them does not exempt the App.
async fn update_finding(
    app: &GithubApp,
    repo: &RepoRef,
    class: JobClass,
    rules: &[BranchRule],
) -> Result<Option<SetupFinding>, ScmError> {
    let updates: BTreeSet<u64> = rules
        .iter()
        .filter(|rule| rule.kind == "update")
        .filter_map(|rule| rule.ruleset_id)
        .collect();
    if updates.is_empty() {
        return Ok(Some(SetupFinding::UpdateRestrictionMissing));
    }

    for id in updates {
        let ruleset: Ruleset = app
            .get_json(repo, class, &format!("/rulesets/{id}"))
            .await?;
        if !ruleset.exempts(app.app_id()) {
            return Ok(Some(SetupFinding::AppNotExempt));
        }
    }

    Ok(None)
}

/// `TestBranchesNotExcluded` when a rule covers the test branches of CIHub.
async fn test_branches_finding(
    app: &GithubApp,
    repo: &RepoRef,
    class: JobClass,
) -> Result<Option<SetupFinding>, ScmError> {
    for test_branch in ["cihub-temp/check/1", "cihub-merge/check/1"] {
        if !branch_rules(app, repo, test_branch, class)
            .await?
            .is_empty()
        {
            return Ok(Some(SetupFinding::TestBranchesNotExcluded));
        }
    }

    Ok(None)
}

/// The page of the rules of `repo`.
pub(crate) fn branch_rules_url(app: &GithubApp, repo: &RepoRef) -> String {
    format!(
        "{}/{}/{}/settings/rules",
        app.web_url, repo.owner, repo.name
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_private_repository_on_github_free_has_no_rules() {
        let answer = Answer::with_message;
        let upgrade =
            "Upgrade to GitHub Pro or make this repository public to enable this feature.";

        assert!(needs_paid_plan(&answer(StatusCode::FORBIDDEN, upgrade)));
        assert!(!needs_paid_plan(&answer(StatusCode::OK, upgrade)));
        let other = answer(
            StatusCode::FORBIDDEN,
            "Resource not accessible by integration",
        );

        assert!(!needs_paid_plan(&other));
    }
}

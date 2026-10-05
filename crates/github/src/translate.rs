//! A GitHub webhook delivery in the neutral terms of `cihub-scm`
//! (`DECISIONS.md` D75, D84): what changed, not the new state.

use cihub_domain::{CheckConclusion, CheckKind, CheckReport};
use cihub_scm::{SUBMIT_BOX, ScmDelivery, ScmEvent, ScmUser};
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum TranslateError {
    #[error("the delivery `{event}` has no field {pointer}")]
    Missing {
        event: String,
        pointer: &'static str,
    },
}

/// Translates the delivery of the event `event` (`X-GitHub-Event`) with its
/// payload. An event that CIHub does not use is [`ScmEvent::Ignored`].
///
/// # Errors
///
/// A field that the event always has is missing.
pub fn translate(event: &str, payload: &Value) -> Result<ScmDelivery, TranslateError> {
    let read = Reader { event, payload };
    let action = payload["action"].as_str().unwrap_or_default();
    let scm_event = match event {
        "pull_request" | "pull_request_review" => pr_changed(&read, action)?,
        "issue_comment"
            if action == "edited"
                && !payload["issue"]["pull_request"].is_null()
                && submit_box_checked(payload) =>
        {
            submit_box(&read)?
        }
        "issue_comment" if action == "created" && !payload["issue"]["pull_request"].is_null() => {
            ScmEvent::Commented {
                repo_external_id: read.id("/repository/id")?,
                number: read.number("/issue/number")?,
                comment_external_id: read.id("/comment/id")?,
                author: read.user("/comment/user")?,
                body: read.text("/comment/body")?,
            }
        }
        "push" => match read.text("/ref")?.strip_prefix("refs/heads/") {
            Some(branch) => ScmEvent::Pushed {
                repo_external_id: read.id("/repository/id")?,
                branch: branch.to_owned(),
                before: (!read.flag("/created"))
                    .then(|| read.text("/before"))
                    .transpose()?,
                after: (!read.flag("/deleted"))
                    .then(|| read.text("/after"))
                    .transpose()?,
                sender: read.user("/sender")?,
            },
            None => ScmEvent::Ignored,
        },
        "check_run" => ScmEvent::CheckChanged {
            repo_external_id: read.id("/repository/id")?,
            sha: read.text("/check_run/head_sha")?,
            branches: payload["check_run"]["check_suite"]["head_branch"]
                .as_str()
                .map(str::to_owned)
                .into_iter()
                .collect(),
            check: CheckReport {
                name: read.text("/check_run/name")?,
                kind: CheckKind::Check,
                conclusion: payload["check_run"]["conclusion"]
                    .as_str()
                    .and_then(|conclusion| conclusion.parse().ok()),
                details_url: payload["check_run"]["details_url"]
                    .as_str()
                    .map(str::to_owned),
            },
        },
        "status" => ScmEvent::CheckChanged {
            repo_external_id: read.id("/repository/id")?,
            sha: read.text("/sha")?,
            branches: payload["branches"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|branch| branch["name"].as_str().map(str::to_owned))
                .collect(),
            check: CheckReport {
                name: read.text("/context")?,
                kind: CheckKind::Status,
                conclusion: status_conclusion(&read.text("/state")?),
                details_url: payload["target_url"].as_str().map(str::to_owned),
            },
        },
        "branch_protection_rule" => ScmEvent::RulesChanged {
            repo_external_id: read.id("/repository/id")?,
        },
        "repository" => ScmEvent::RepoChanged {
            repo_external_id: read.id("/repository/id")?,
        },
        "installation" => match action {
            "created" => ScmEvent::ConnectionCreated {
                account_external_id: read.id("/installation/account/id")?,
                account_path: read.text("/installation/account/login")?,
                claim: None,
            },
            "deleted" | "suspend" => ScmEvent::AccessLost,
            "unsuspend" => ScmEvent::AccessRestored,
            _ => ScmEvent::Ignored,
        },
        "installation_repositories" => ScmEvent::ReposChanged {
            added: read.ids("/repositories_added")?,
            removed: read.ids("/repositories_removed")?,
        },
        _ => ScmEvent::Ignored,
    };
    Ok(ScmDelivery {
        connection_external_id: id(payload.pointer("/installation/id")),
        event: scm_event,
    })
}

/// The conclusion of a commit status; `pending` and `expected` have none. A
/// webhook writes the state in lower case, GraphQL in upper case.
pub(crate) fn status_conclusion(state: &str) -> Option<CheckConclusion> {
    match state.to_ascii_lowercase().as_str() {
        "success" => Some(CheckConclusion::Success),
        "failure" => Some(CheckConclusion::Failure),
        "error" => Some(CheckConclusion::Error),
        _ => None,
    }
}

/// A change of a PR or of its reviews: the label that it added, and whether
/// it made the PR ready for review (`DECISIONS.md` D163).
fn pr_changed(read: &Reader<'_>, action: &str) -> Result<ScmEvent, TranslateError> {
    let on_pr = read.event == "pull_request";
    Ok(ScmEvent::PrChanged {
        repo_external_id: read.id("/repository/id")?,
        number: read.number("/pull_request/number")?,
        base_branch: read.text("/pull_request/base/ref")?,
        label_added: (on_pr && action == "labeled")
            .then(|| read.payload["label"]["name"].as_str().map(str::to_owned))
            .flatten(),
        opened: on_pr
            && match action {
                "opened" | "reopened" => !read.flag("/pull_request/draft"),
                "ready_for_review" => true,
                _ => false,
            },
    })
}

fn submit_box(read: &Reader<'_>) -> Result<ScmEvent, TranslateError> {
    Ok(ScmEvent::SubmitBoxChecked {
        repo_external_id: read.id("/repository/id")?,
        number: read.number("/issue/number")?,
        comment_external_id: read.id("/comment/id")?,
        sender: read.user("/sender")?,
    })
}

/// An edit of a comment that checked its submission box (`DECISIONS.md`
/// D163): the box was empty before, it is checked now, and the App did not
/// make the edit itself.
fn submit_box_checked(payload: &Value) -> bool {
    let has = |pointer: &str, check: &str| {
        payload
            .pointer(pointer)
            .and_then(Value::as_str)
            .is_some_and(|body| body.contains(&format!("- [{check}] {SUBMIT_BOX}")))
    };
    let by_bot = payload["sender"]["type"].as_str() == Some("Bot");
    !by_bot && has("/comment/body", "x") && has("/changes/body/from", " ")
}

/// A numeric or text identifier of GitHub, as text (`DECISIONS.md` D46).
fn id(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

struct Reader<'a> {
    event: &'a str,
    payload: &'a Value,
}

impl Reader<'_> {
    fn missing(&self, pointer: &'static str) -> TranslateError {
        TranslateError::Missing {
            event: self.event.to_owned(),
            pointer,
        }
    }

    fn id(&self, pointer: &'static str) -> Result<String, TranslateError> {
        id(self.payload.pointer(pointer)).ok_or_else(|| self.missing(pointer))
    }

    fn text(&self, pointer: &'static str) -> Result<String, TranslateError> {
        self.payload
            .pointer(pointer)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| self.missing(pointer))
    }

    fn number(&self, pointer: &'static str) -> Result<u64, TranslateError> {
        self.payload
            .pointer(pointer)
            .and_then(Value::as_u64)
            .ok_or_else(|| self.missing(pointer))
    }

    fn flag(&self, pointer: &'static str) -> bool {
        self.payload
            .pointer(pointer)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    fn user(&self, pointer: &'static str) -> Result<ScmUser, TranslateError> {
        let user = self
            .payload
            .pointer(pointer)
            .ok_or_else(|| self.missing(pointer))?;
        Ok(ScmUser {
            external_id: id(user.get("id")).ok_or_else(|| self.missing(pointer))?,
            login: user["login"]
                .as_str()
                .ok_or_else(|| self.missing(pointer))?
                .to_owned(),
        })
    }

    /// The identifiers of the repositories of a list; an absent list is
    /// empty.
    fn ids(&self, pointer: &'static str) -> Result<Vec<String>, TranslateError> {
        let Some(list) = self.payload.pointer(pointer) else {
            return Ok(Vec::new());
        };
        list.as_array()
            .ok_or_else(|| self.missing(pointer))?
            .iter()
            .map(|repo| id(repo.get("id")).ok_or_else(|| self.missing(pointer)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn event(name: &str, payload: &Value) -> ScmEvent {
        translate(name, payload).unwrap().event
    }

    fn user(id: u64, login: &str) -> Value {
        json!({ "id": id, "login": login })
    }

    #[test]
    fn a_delivery_names_its_installation() {
        let payload = json!({ "zen": "Design for failure.", "installation": { "id": 42 } });
        let delivery = translate("ping", &payload).unwrap();
        assert_eq!(delivery.connection_external_id.as_deref(), Some("42"));
        assert_eq!(delivery.event, ScmEvent::Ignored);
        let none = translate("ping", &json!({})).unwrap();
        assert_eq!(none.connection_external_id, None);
    }

    #[test]
    fn a_pr_or_its_review_is_a_pr_change() {
        let payload = json!({
            "action": "labeled",
            "label": { "name": "ready" },
            "repository": { "id": 7 },
            "pull_request": { "number": 12, "base": { "ref": "main" } },
        });
        let changed = |label_added: Option<&str>, opened| ScmEvent::PrChanged {
            repo_external_id: "7".into(),
            number: 12,
            base_branch: "main".into(),
            label_added: label_added.map(str::to_owned),
            opened,
        };
        assert_eq!(
            event("pull_request", &payload),
            changed(Some("ready"), false)
        );
        // A review adds no label, whatever its action.
        assert_eq!(event("pull_request_review", &payload), changed(None, false));
        // A PR ready for review opens (`DECISIONS.md` D163); a draft does not.
        let action = |action: &str, draft: bool| {
            json!({
                "action": action,
                "repository": { "id": 7 },
                "pull_request": { "number": 12, "base": { "ref": "main" }, "draft": draft },
            })
        };
        for (name, draft, opened) in [
            ("opened", false, true),
            ("opened", true, false),
            ("reopened", false, true),
            ("ready_for_review", false, true),
            ("synchronize", false, false),
        ] {
            assert_eq!(
                event("pull_request", &action(name, draft)),
                changed(None, opened),
                "{name} {draft}"
            );
        }
    }

    #[test]
    fn checking_the_submission_box_of_a_comment_is_a_submission() {
        let box_of = |check: &str| format!("Merging...\n\n- [{check}] {SUBMIT_BOX} To merge...");
        let edit = |before: &str, after: &str, sender: &str| {
            json!({
                "action": "edited",
                "repository": { "id": 7 },
                "issue": { "number": 12, "pull_request": { "url": "u" } },
                "comment": { "id": 99, "user": user(1, "cihub[bot]"), "body": after },
                "changes": { "body": { "from": before } },
                "sender": { "id": 3, "login": "alice", "type": sender },
            })
        };
        assert_eq!(
            event("issue_comment", &edit(&box_of(" "), &box_of("x"), "User")),
            ScmEvent::SubmitBoxChecked {
                repo_external_id: "7".into(),
                number: 12,
                comment_external_id: "99".into(),
                sender: ScmUser {
                    external_id: "3".into(),
                    login: "alice".into()
                },
            }
        );
        // The App edits its own comment; an unchecked box; no box before.
        for (before, after, sender) in [
            (box_of(" "), box_of("x"), "Bot"),
            (box_of("x"), box_of(" "), "User"),
            ("no box".to_owned(), box_of("x"), "User"),
        ] {
            assert_eq!(
                event("issue_comment", &edit(&before, &after, sender)),
                ScmEvent::Ignored
            );
        }
    }

    #[test]
    fn only_a_new_comment_on_a_pr_is_a_comment() {
        let comment = |action: &str, pull_request: Value| {
            json!({
                "action": action,
                "repository": { "id": 7 },
                "issue": { "number": 12, "pull_request": pull_request },
                "comment": { "id": 40, "user": user(3, "alice"), "body": "/cihub merge" },
            })
        };
        let on_pr = json!({ "url": "https://api.github.com/repos/o/r/pulls/12" });
        assert_eq!(
            event("issue_comment", &comment("created", on_pr.clone())),
            ScmEvent::Commented {
                repo_external_id: "7".into(),
                number: 12,
                comment_external_id: "40".into(),
                author: ScmUser {
                    external_id: "3".into(),
                    login: "alice".into()
                },
                body: "/cihub merge".into(),
            }
        );
        assert_eq!(
            event("issue_comment", &comment("edited", on_pr)),
            ScmEvent::Ignored
        );
        assert_eq!(
            event("issue_comment", &comment("created", Value::Null)),
            ScmEvent::Ignored
        );
    }

    #[test]
    fn a_push_on_a_branch_gives_its_heads_and_a_tag_is_ignored() {
        let push = |git_ref: &str, created: bool, deleted: bool| {
            json!({
                "ref": git_ref,
                "before": "b".repeat(40),
                "after": "a".repeat(40),
                "created": created,
                "deleted": deleted,
                "repository": { "id": 7 },
                "sender": user(3, "alice"),
            })
        };
        let pushed = |before: Option<String>, after: Option<String>| ScmEvent::Pushed {
            repo_external_id: "7".into(),
            branch: "main".into(),
            before,
            after,
            sender: ScmUser {
                external_id: "3".into(),
                login: "alice".into(),
            },
        };
        let (before, after) = (Some("b".repeat(40)), Some("a".repeat(40)));
        assert_eq!(
            event("push", &push("refs/heads/main", false, false)),
            pushed(before.clone(), after.clone())
        );
        assert_eq!(
            event("push", &push("refs/heads/main", true, false)),
            pushed(None, after)
        );
        assert_eq!(
            event("push", &push("refs/heads/main", false, true)),
            pushed(before, None)
        );
        assert_eq!(
            event("push", &push("refs/tags/v1", false, false)),
            ScmEvent::Ignored
        );
    }

    #[test]
    fn a_check_run_and_a_status_are_check_changes() {
        let check_run = json!({
            "repository": { "id": 7 },
            "check_run": {
                "name": "ci", "head_sha": "abc", "conclusion": "timed_out",
                "details_url": "https://ci.example.com/1",
                "check_suite": { "head_branch": "cihub-merge/pr-12/r1" },
            },
        });
        assert_eq!(
            event("check_run", &check_run),
            ScmEvent::CheckChanged {
                repo_external_id: "7".into(),
                sha: "abc".into(),
                branches: vec!["cihub-merge/pr-12/r1".into()],
                check: CheckReport {
                    name: "ci".into(),
                    kind: CheckKind::Check,
                    conclusion: Some(CheckConclusion::TimedOut),
                    details_url: Some("https://ci.example.com/1".into()),
                },
            }
        );
        let status = json!({
            "repository": { "id": 7 },
            "sha": "abc", "context": "deploy", "state": "pending", "target_url": null,
            "branches": [{ "name": "main" }, { "name": "next" }],
        });
        assert_eq!(
            event("status", &status),
            ScmEvent::CheckChanged {
                repo_external_id: "7".into(),
                sha: "abc".into(),
                branches: vec!["main".into(), "next".into()],
                check: CheckReport {
                    name: "deploy".into(),
                    kind: CheckKind::Status,
                    conclusion: None,
                    details_url: None,
                },
            }
        );
    }

    #[test]
    fn rules_repositories_and_installations() {
        let repo = json!({ "action": "edited", "repository": { "id": 7 } });
        let id = || "7".to_owned();
        assert_eq!(
            event("branch_protection_rule", &repo),
            ScmEvent::RulesChanged {
                repo_external_id: id()
            }
        );
        assert_eq!(
            event("repository", &repo),
            ScmEvent::RepoChanged {
                repo_external_id: id()
            }
        );
        let installation = |action: &str| {
            json!({
                "action": action,
                "installation": { "id": 42, "account": { "id": 9, "login": "acme" } },
            })
        };
        assert_eq!(
            event("installation", &installation("created")),
            ScmEvent::ConnectionCreated {
                account_external_id: "9".into(),
                account_path: "acme".into(),
                claim: None,
            }
        );
        for action in ["deleted", "suspend"] {
            assert_eq!(
                event("installation", &installation(action)),
                ScmEvent::AccessLost
            );
        }
        assert_eq!(
            event("installation", &installation("unsuspend")),
            ScmEvent::AccessRestored
        );
        assert_eq!(
            event("installation", &installation("new_permissions_accepted")),
            ScmEvent::Ignored
        );
        let repos = json!({
            "action": "added",
            "installation": { "id": 42 },
            "repositories_added": [{ "id": 7 }, { "id": 8 }],
            "repositories_removed": [],
        });
        assert_eq!(
            event("installation_repositories", &repos),
            ScmEvent::ReposChanged {
                added: vec!["7".into(), "8".into()],
                removed: vec![],
            }
        );
    }

    #[test]
    fn a_missing_field_is_an_error() {
        let err = translate("pull_request", &json!({ "repository": { "id": 7 } })).unwrap_err();
        assert_eq!(
            err.to_string(),
            "the delivery `pull_request` has no field /pull_request/number"
        );
    }
}

closed_enum! {
    /// The role of a user in an organization.
    pub enum MembershipRole {
        /// Changes the settings, pauses the queues, creates API tokens.
        Admin,
        /// Submits and cancels PRs.
        Member,
        /// Reads only.
        Viewer,
    }
}

closed_enum! {
    /// The product of an SCM instance.
    pub enum ScmKind {
        /// github.com or GitHub Enterprise Server.
        Github,
    }
}

closed_enum! {
    /// How CIHub reaches the repositories of an SCM account.
    pub enum ConnectionKind {
        /// An installation of the App of the queue.
        GithubApp,
        /// An installation of the opt-in `cihub-sudo` App, used only to merge
        /// forced items.
        GithubSudoApp,
    }
}

closed_enum! {
    /// What an API token may do (`docs/API.md`).
    pub enum ApiScope {
        /// Read the queues, their items and their timeline.
        #[serde(rename = "queues:read")]
        #[strum(serialize = "queues:read")]
        QueuesRead,
        /// Create, change, pause and delete queues.
        #[serde(rename = "queues:write")]
        #[strum(serialize = "queues:write")]
        QueuesWrite,
        /// Submit, cancel and restart PRs.
        #[serde(rename = "prs:write")]
        #[strum(serialize = "prs:write")]
        PrsWrite,
        /// Upload the impacted targets of a PR; the only scope of a CI token.
        #[serde(rename = "targets:write")]
        #[strum(serialize = "targets:write")]
        TargetsWrite,
        /// Read the metrics of the queues.
        #[serde(rename = "metrics:read")]
        #[strum(serialize = "metrics:read")]
        MetricsRead,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_keep_their_colon() {
        assert_eq!(ApiScope::QueuesRead.as_str(), "queues:read");
        assert_eq!("targets:write".parse(), Ok(ApiScope::TargetsWrite));
        assert_eq!(
            serde_json::to_string(&ApiScope::MetricsRead).unwrap(),
            r#""metrics:read""#
        );
        assert_eq!(MembershipRole::Viewer.as_str(), "viewer");
        assert_eq!(ConnectionKind::GithubSudoApp.to_string(), "github_sudo_app");
        assert_eq!(ScmKind::Github.as_str(), "github");
    }
}

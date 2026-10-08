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
    }
}

closed_enum! {
    /// How much a connection may call its SCM (`docs/RELIABILITY.md`,
    /// `DECISIONS.md` D77).
    pub enum ConnectionMode {
        /// Every call goes out.
        Normal,
        /// The budget is low: the speculation waits, the rest goes out.
        Throttled,
        /// The SCM keeps failing: only a probe goes out, the jobs wait.
        ScmUnavailable,
        /// An admin or the SCM suspended the connection: nothing goes out.
        Paused,
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_keep_their_colon() {
        assert_eq!(ApiScope::QueuesRead.as_str(), "queues:read");
        assert_eq!("prs:write".parse(), Ok(ApiScope::PrsWrite));
        assert_eq!(
            serde_json::to_string(&ApiScope::QueuesWrite).unwrap(),
            r#""queues:write""#
        );
        assert_eq!(ConnectionKind::GithubApp.to_string(), "github_app");
        assert_eq!(ScmKind::Github.as_str(), "github");
    }
}

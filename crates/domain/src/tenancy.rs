closed_enum! {
    /// How CIHub reaches the repositories of an SCM account.
    pub enum ConnectionKind {
        /// An installation of the App of the queue.
        GithubApp,
    }
}

closed_enum! {
    /// How much a connection may call its SCM (`specs/RELIABILITY.md`).
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
    /// What an API token may do (`specs/API.md`).
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

closed_enum! {
    /// The role of a user in an organization, from its membership on the
    /// SCM (`specs/SECURITY.md`).
    pub enum MembershipRole {
        /// An owner of the account on the SCM: also changes the state and the
        /// configuration of a queue, and reads the audit log.
        Admin,
        /// Reads the queues, and submits, cancels and restarts PRs.
        Member,
    }
}

impl MembershipRole {
    /// What a session of this role may do: the scopes of an API token.
    pub const fn scopes(self) -> &'static [ApiScope] {
        match self {
            Self::Admin => &[
                ApiScope::QueuesRead,
                ApiScope::QueuesWrite,
                ApiScope::PrsWrite,
            ],
            Self::Member => &[ApiScope::QueuesRead, ApiScope::PrsWrite],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_admin_changes_a_queue() {
        for scope in <ApiScope as strum::IntoEnumIterator>::iter() {
            assert!(MembershipRole::Admin.scopes().contains(&scope), "{scope}");
        }
        assert!(
            !MembershipRole::Member
                .scopes()
                .contains(&ApiScope::QueuesWrite)
        );
        assert!(
            MembershipRole::Member
                .scopes()
                .contains(&ApiScope::PrsWrite)
        );
    }

    #[test]
    fn scopes_keep_their_colon() {
        assert_eq!(ApiScope::QueuesRead.as_str(), "queues:read");
        assert_eq!("prs:write".parse(), Ok(ApiScope::PrsWrite));
        assert_eq!(
            serde_json::to_string(&ApiScope::QueuesWrite).unwrap(),
            r#""queues:write""#
        );
        assert_eq!(ConnectionKind::GithubApp.to_string(), "github_app");
    }
}

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
    /// Whether a repository has a configuration file on its default branch,
    /// and whether it is valid, at the last setup read (`specs/DATA_MODEL.md`).
    pub enum RepoConfigState {
        /// No file.
        None,
        /// A file that parses, with settings for the default branch.
        Valid,
        /// A file that does not parse, or two paths of the file.
        Invalid,
    }
}

closed_enum! {
    /// Where a proposal of a change of the configuration file stands: CIHub
    /// opens its PR, then a person merges or closes it (`specs/API.md`).
    pub enum ConfigProposalState {
        /// The job that opens the PR waits or runs.
        Requested,
        /// The PR is open.
        Opened,
        /// The PR merged: the file of the branch holds the change.
        Merged,
        /// The PR closed without a merge.
        Closed,
        /// CIHub could not open the PR; the reason says why.
        Failed,
    }
}

closed_enum! {
    /// Why CIHub could not open the PR of a proposal.
    pub enum ConfigProposalReason {
        /// The file of the branch changed since the person read it.
        ConfigChanged,
        /// A rule of the SCM refuses the branch of the proposal.
        ConfigBranchRefused,
        /// The SCM refused to write the file or to open the PR, or kept
        /// failing until CIHub gave up.
        PrNotOpened,
    }
}

impl ConfigProposalReason {
    /// The reason in English, for a person; `branch` is the target of the
    /// proposal.
    pub fn describe(self, branch: &str) -> String {
        match self {
            Self::ConfigChanged => format!(
                "the configuration file of `{branch}` changed since it was read: start again from \
                 the new file"
            ),
            Self::ConfigBranchRefused => format!(
                "a rule refuses the branch `cihub-config/{branch}`: exclude `cihub-config/**` from \
                 the rulesets, or change the file by hand"
            ),
            Self::PrNotOpened => format!(
                "the SCM did not open the pull request from `cihub-config/{branch}`: propose the \
                 change again"
            ),
        }
    }
}

closed_enum! {
    /// Where a sign-in of the command line stands (`specs/API.md`).
    pub enum DeviceCodeState {
        /// The command line waits for the person.
        Pending,
        /// The person approved the code: the command line takes its session.
        Approved,
        /// The person denied the code.
        Denied,
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
    fn a_proposal_says_why_it_failed() {
        assert!(
            ConfigProposalReason::ConfigChanged
                .describe("main")
                .starts_with("the configuration file of `main` changed")
        );
        assert!(
            ConfigProposalReason::ConfigBranchRefused
                .describe("main")
                .contains("`cihub-config/main`")
        );
        assert!(
            ConfigProposalReason::PrNotOpened
                .describe("main")
                .ends_with("propose the change again")
        );
    }

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

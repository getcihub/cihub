use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use cihub_domain::{MembershipRole, Permission};
use secrecy::{ExposeSecret, SecretString};

use super::{PendingSignIn, Sim, State};
use crate::{
    ScmAccount, ScmAccountProfile, ScmError, ScmInstallation, ScmMember, ScmPersonToken,
    ScmProfile, ScmUser, SignInRequest,
};

impl Sim {
    /// Installs the App on `account` as the connection
    /// `connection_external_id`, with `permissions`.
    pub fn add_installation(
        &self,
        connection_external_id: &str,
        account: ScmAccount,
        permissions: &[(&str, &str)],
    ) {
        let installation = ScmInstallation {
            external_id: connection_external_id.to_owned(),
            account,
            permissions: permission_levels(permissions),
            is_suspended: false,
        };
        self.with(|state| {
            state
                .installations
                .insert(connection_external_id.to_owned(), installation);
            state.connection_changed(connection_external_id);
        });
    }

    /// Gives the installation `connection_external_id` the permissions
    /// `permissions`, as an owner who accepts new permissions.
    pub fn set_permissions(&self, connection_external_id: &str, permissions: &[(&str, &str)]) {
        self.with(|state| {
            let Some(installation) = state.installations.get_mut(connection_external_id) else {
                return;
            };

            installation.permissions = permission_levels(permissions);
            state.connection_changed(connection_external_id);
        });
    }

    /// Deletes the installation `connection_external_id`: its connection
    /// fails every call with `AccessLost`.
    pub fn remove_installation(&self, connection_external_id: &str) {
        self.with(|state| {
            if state.installations.remove(connection_external_id).is_none() {
                return;
            }

            state
                .suspended_connections
                .insert(connection_external_id.to_owned());
            state.connection_changed(connection_external_id);
        });
    }

    /// Suspends the connection `connection_external_id`, or makes it active
    /// again: a suspended connection fails every call with `AccessLost`.
    pub fn set_access(&self, connection_external_id: &str, is_active: bool) {
        self.with(|state| {
            let is_changed = if is_active {
                state.suspended_connections.remove(connection_external_id)
            } else {
                state
                    .suspended_connections
                    .insert(connection_external_id.to_owned())
            };
            if is_changed {
                state.connection_changed(connection_external_id);
            }
        });
    }

    /// Gives `user` the role `role` in the account of the installation
    /// `connection_external_id`.
    pub fn set_member(&self, connection_external_id: &str, user: &ScmUser, role: MembershipRole) {
        self.with(|state| {
            let member = ScmMember {
                user: user.clone(),
                role,
                avatar_url: None,
            };
            state
                .members
                .entry(connection_external_id.to_owned())
                .or_default()
                .insert(user.external_id.clone(), member);
        });
    }

    /// Takes `user` out of the account of the installation
    /// `connection_external_id`.
    pub fn remove_member(&self, connection_external_id: &str, user: &ScmUser) {
        self.with(|state| {
            if let Some(members) = state.members.get_mut(connection_external_id) {
                members.remove(&user.external_id);
            }
        });
    }

    /// Gives `user` the name and the avatar of `profile`, at their next
    /// sign-in and in the list of the members.
    pub fn set_profile(&self, user: &ScmUser, profile: ScmProfile) {
        self.with(|state| {
            state.profiles.insert(user.external_id.clone(), profile);
        });
    }

    /// Gives `account` the name, the description and the avatar of
    /// `profile`.
    pub fn set_account_profile(&self, account: &ScmAccount, profile: ScmAccountProfile) {
        self.with(|state| {
            state
                .account_profiles
                .insert(account.external_id.clone(), profile);
        });
    }

    /// Makes the address of a sign-in a page that a browser opens: a test
    /// server serves it, and signs a person in with [`Sim::authorize`].
    pub fn serve_sign_in_at(&self, page: &str) {
        self.with(|state| state.sign_in_page = Some(page.to_owned()));
    }

    /// Makes the address that installs the App a page that a browser opens: a
    /// test server serves it, and installs the App with
    /// [`Sim::add_installation`].
    pub fn serve_install_at(&self, page: &str) {
        self.with(|state| state.install_page = Some(page.to_owned()));
    }

    /// Signs `user` in on the page of the SCM for `request`, as after their
    /// consent, and returns the code that the SCM sends to its
    /// `redirect_url`.
    pub fn authorize(&self, user: &ScmUser, request: &SignInRequest) -> String {
        self.with(|state| {
            state.next_sign_in_code += 1;
            let code = format!("code-{}", state.next_sign_in_code);
            let pending = PendingSignIn {
                user: user.clone(),
                request: request.clone(),
            };
            state.sign_in_codes.insert(code.clone(), pending);

            code
        })
    }

    /// Revokes every token of `user`, as a person who removes the
    /// authorization of the App: their calls fail with `AccessLost`.
    pub fn revoke_person(&self, user: &ScmUser) {
        self.with(|state| {
            state
                .person_tokens
                .retain(|_, person| *person != user.external_id);
            state
                .refresh_tokens
                .retain(|_, person| *person != user.external_id);
        });
    }
}

impl State {
    /// A new token for the person `user`, valid 8 hours, and its refresh.
    pub(super) fn person_token(&mut self, user: &str) -> ScmPersonToken {
        self.next_person_token += 1;
        let (access, refresh) = (
            format!("sim-token-{}", self.next_person_token),
            format!("sim-refresh-{}", self.next_person_token),
        );
        self.person_tokens.insert(access.clone(), user.to_owned());
        self.refresh_tokens.insert(refresh.clone(), user.to_owned());

        ScmPersonToken {
            access: access.into(),
            access_lifetime: Some(time::Duration::hours(8)),
            refresh: Some(refresh.into()),
            refresh_lifetime: Some(time::Duration::days(180)),
        }
    }

    /// The person of `token`.
    pub(super) fn person_of(&self, token: &SecretString) -> Result<String, ScmError> {
        self.person_tokens
            .get(token.expose_secret())
            .cloned()
            .ok_or_else(|| ScmError::AccessLost("the token of the person is refused".into()))
    }

    /// What `user` may do on the repository `repo_external_id`: the
    /// permission given to them, or `admin` for an admin of the account of
    /// its installation.
    pub(super) fn permission_of_person(&self, repo_external_id: &str, user: &str) -> Permission {
        let Some(repo) = self.repos.get(repo_external_id) else {
            return Permission::None;
        };
        if let Some(permission) = repo.permissions.get(user) {
            return *permission;
        }

        let is_admin = self
            .members
            .get(&repo.connection)
            .and_then(|members| members.get(user))
            .is_some_and(|member| member.role == MembershipRole::Admin);
        if is_admin {
            Permission::Admin
        } else {
            Permission::None
        }
    }

    /// The installations that `user` reaches: those of the accounts where
    /// they have a role, and those of the repositories where they have a
    /// permission, as an outside collaborator does.
    pub(super) fn reach_of_person(&self, user: &str) -> Vec<String> {
        let by_role = self
            .members
            .iter()
            .filter(|(_, members)| members.contains_key(user))
            .map(|(installation, _)| installation.as_str());
        let by_permission = self
            .repos
            .iter()
            .filter(|(external_id, _)| {
                self.permission_of_person(external_id, user) != Permission::None
            })
            .map(|(_, repo)| repo.connection.as_str());
        let reached: BTreeSet<&str> = by_role
            .chain(by_permission)
            .filter(|installation| self.installations.contains_key(*installation))
            .collect();

        reached.into_iter().map(str::to_owned).collect()
    }

    /// The installation `external_id`, suspended when its connection is.
    pub(super) fn installation(&self, external_id: &str) -> Option<ScmInstallation> {
        let mut installation = self.installations.get(external_id)?.clone();
        installation.is_suspended = self.suspended_connections.contains(external_id);

        Some(installation)
    }
}

/// `text` for the query of an address: every byte but the unreserved ones
/// of RFC 3986 is percent-encoded.
pub(super) fn encode_query(text: &str) -> String {
    text.bytes()
        .fold(String::with_capacity(text.len()), |mut encoded, byte| {
            if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
                encoded.push(char::from(byte));
            } else {
                let _ = write!(encoded, "%{byte:02X}");
            }
            encoded
        })
}

fn permission_levels(permissions: &[(&str, &str)]) -> BTreeMap<String, String> {
    permissions
        .iter()
        .map(|(name, level)| ((*name).to_owned(), (*level).to_owned()))
        .collect()
}

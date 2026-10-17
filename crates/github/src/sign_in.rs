//! A person on GitHub: the page that installs the App, the sign-in through the
//! App, which gives the installations that the person reaches and their role
//! in each one, and the members of the account of an installation
//! (`specs/GITHUB.md`).

use std::collections::BTreeMap;

use cihub_domain::{JobClass, MembershipRole};
use cihub_scm::{
    ConnectionRef, ScmAccount, ScmError, ScmInstallation, ScmMember, ScmMembership, ScmProfile,
    ScmUser, SignInCode, SignInRequest, SignedIn,
};
use reqwest::{Method, StatusCode, Url};
use secrecy::SecretString;
use serde::Deserialize;

use crate::GithubApp;
use crate::client::{JSON, Request};
use crate::scm::{encode, expect};

/// Where the dashboard sends a person to sign in through GitHub.
pub const SIGN_IN_PATH: &str = "/auth/github";
/// Where GitHub sends the person back; the *Callback URL* of the App.
pub const SIGN_IN_CALLBACK_PATH: &str = "/auth/github/callback";
/// Where GitHub sends an owner after an install or an update of the App; the
/// *Setup URL* of the App. Its `installation_id` proves nothing: the sign-in
/// that it starts again does.
pub const SETUP_PATH: &str = "/auth/github/installed";

const MEMBERS_PER_PAGE: usize = 100;

#[derive(Deserialize)]
struct Person {
    id: u64,
    login: String,
    /// Only `/user` gives it.
    name: Option<String>,
    avatar_url: Option<String>,
}

impl Person {
    fn user(&self) -> ScmUser {
        ScmUser {
            external_id: self.id.to_string(),
            login: self.login.clone(),
        }
    }
}

#[derive(Deserialize)]
struct Installation {
    id: u64,
    account: Account,
    #[serde(default)]
    permissions: BTreeMap<String, String>,
    suspended_at: Option<String>,
}

impl Installation {
    fn neutral(&self) -> ScmInstallation {
        ScmInstallation {
            external_id: self.id.to_string(),
            account: ScmAccount {
                external_id: self.account.id.to_string(),
                path: self.account.login.clone(),
            },
            permissions: self.permissions.clone(),
            is_suspended: self.suspended_at.is_some(),
        }
    }
}

#[derive(Deserialize)]
struct Account {
    id: u64,
    login: String,
    avatar_url: Option<String>,
    #[serde(rename = "type")]
    kind: String,
}

impl GithubApp {
    /// `<html_url of the App>/installations/new`: GitHub shows the accounts
    /// where the App is not installed yet, and the repositories of an
    /// installation that exists.
    pub(crate) async fn install_address(&self) -> Result<String, ScmError> {
        #[derive(Deserialize)]
        struct App {
            html_url: String,
        }

        if let Some(url) = self.install_url.get() {
            return Ok(url.clone());
        }

        let (answer, _) = self.send_as_app(Method::GET, "/app").await?;
        let app: App = expect(answer, &[StatusCode::OK])?.json()?;
        let url = format!("{}/installations/new", app.html_url.trim_end_matches('/'));

        Ok(self.install_url.get_or_init(|| url).clone())
    }

    pub(crate) fn sign_in_address(&self, request: &SignInRequest) -> Option<String> {
        let client = self.sign_in.as_ref()?;
        let address = Url::parse_with_params(
            &format!("{}/login/oauth/authorize", self.web_url),
            [
                ("client_id", client.client_id.as_str()),
                ("redirect_uri", request.redirect_url.as_str()),
                ("state", request.state.as_str()),
                ("code_challenge", request.code_challenge.as_str()),
                ("code_challenge_method", "S256"),
            ],
        )
        .ok()?;

        Some(address.into())
    }

    pub(crate) async fn sign_in_person(&self, code: &SignInCode) -> Result<SignedIn, ScmError> {
        let token = self.exchange_sign_in_code(code).await?;
        let (answer, _) = self.get_as_person(&token, "/user").await?;
        let user: Person = expect(answer, &[StatusCode::OK])?.json()?;
        let listed = self.installations_of_person(&token).await?;
        let installations = listed.iter().map(Installation::neutral).collect();
        let mut memberships = Vec::new();

        for installation in listed {
            let role = self
                .role_of_person(&token, &user, &installation.account)
                .await?;
            if let Some(role) = role {
                memberships.push(ScmMembership {
                    connection_external_id: installation.id.to_string(),
                    role,
                });
            }
        }

        Ok(SignedIn {
            user: user.user(),
            profile: ScmProfile {
                name: user.name,
                avatar_url: user.avatar_url,
            },
            installations,
            memberships,
        })
    }

    async fn installations_of_person(
        &self,
        token: &SecretString,
    ) -> Result<Vec<Installation>, ScmError> {
        #[derive(Deserialize)]
        struct Page {
            installations: Vec<Installation>,
        }

        let mut path = Some("/user/installations?per_page=100".to_owned());
        let mut installations = Vec::new();
        while let Some(page) = path.take() {
            let (answer, next) = self.get_as_person(token, &page).await?;
            let listed: Page = expect(answer, &[StatusCode::OK])?.json()?;
            installations.extend(listed.installations);
            path = next;
        }

        Ok(installations)
    }

    /// The role of `person` in `account`: the owner of the account of a
    /// person is its `admin`; an organization gives the role of an active
    /// member. `None` for anyone else, for another kind of account, and when
    /// GitHub refuses the read (single sign-on not authorized, allow list of
    /// addresses): the sign-in deletes nothing, so the hourly read of the
    /// members decides.
    async fn role_of_person(
        &self,
        token: &SecretString,
        person: &Person,
        account: &Account,
    ) -> Result<Option<MembershipRole>, ScmError> {
        #[derive(Deserialize)]
        struct Membership {
            state: String,
            role: String,
        }

        match account.kind.as_str() {
            "Organization" => {}
            "User" => return Ok((account.id == person.id).then_some(MembershipRole::Admin)),
            _ => return Ok(None),
        }

        let path = format!("/user/memberships/orgs/{}", encode(&account.login));
        let (answer, _) = self.get_as_person(token, &path).await?;
        if matches!(answer.status, StatusCode::NOT_FOUND | StatusCode::FORBIDDEN) {
            return Ok(None);
        }

        let membership: Membership = expect(answer, &[StatusCode::OK])?.json()?;
        if membership.state != "active" {
            return Ok(None);
        }

        Ok(membership_role(&membership.role))
    }

    /// The members of the account of `connection`. The account comes from a
    /// read of the installation, so that its kind and its login are those of
    /// now, not those of its first sync.
    pub(crate) async fn account_members(
        &self,
        connection: &ConnectionRef,
        class: JobClass,
    ) -> Result<Vec<ScmMember>, ScmError> {
        let path = format!("/app/installations/{}", encode(&connection.external_id));
        let (answer, _) = self.send_as_app(Method::GET, &path).await?;
        let installation: Installation = expect(answer, &[StatusCode::OK])?.json()?;
        let account = installation.account;

        match account.kind.as_str() {
            "Organization" => {}
            "User" => return Ok(vec![owner(account)]),
            _ => return Ok(Vec::new()),
        }

        let mut members = self
            .members_with_role(connection, &account, MembershipRole::Admin, class)
            .await?;
        let others = self
            .members_with_role(connection, &account, MembershipRole::Member, class)
            .await?;

        members.extend(others);
        Ok(members)
    }

    /// The members of the organization `account` with `role`, every page.
    async fn members_with_role(
        &self,
        connection: &ConnectionRef,
        account: &Account,
        role: MembershipRole,
        class: JobClass,
    ) -> Result<Vec<ScmMember>, ScmError> {
        let filter = match role {
            MembershipRole::Admin => "admin",
            MembershipRole::Member => "member",
        };
        let mut members = Vec::new();
        for page in 1.. {
            let path = format!(
                "/orgs/{}/members?role={filter}&per_page={MEMBERS_PER_PAGE}&page={page}",
                encode(&account.login)
            );
            let answer = self
                .send(Request {
                    connection,
                    class,
                    method: Method::GET,
                    path,
                    body: None,
                    accept: JSON,
                })
                .await?;
            let listed: Vec<Person> = expect(answer, &[StatusCode::OK])?.json()?;
            let is_last = listed.len() < MEMBERS_PER_PAGE;

            members.extend(listed.into_iter().map(|person| ScmMember {
                user: person.user(),
                role,
                avatar_url: person.avatar_url,
            }));
            if is_last {
                break;
            }
        }

        Ok(members)
    }
}

fn membership_role(name: &str) -> Option<MembershipRole> {
    match name {
        "admin" => Some(MembershipRole::Admin),
        "member" => Some(MembershipRole::Member),
        _ => None,
    }
}

/// The account of a person has no members: its owner is its only `admin`.
fn owner(account: Account) -> ScmMember {
    ScmMember {
        user: ScmUser {
            external_id: account.id.to_string(),
            login: account.login,
        },
        role: MembershipRole::Admin,
        avatar_url: account.avatar_url,
    }
}

//! A person on GitHub: the page that installs the App, the sign-in through the
//! App, which gives the installations that the person reaches and their role
//! in each one, and the members of the account of an installation
//! (`specs/GITHUB.md`).

use std::collections::BTreeMap;

use cihub_domain::{JobClass, MembershipRole, Permission};
use cihub_scm::{
    ConnectionRef, RepoRef, ScmAccount, ScmError, ScmInstallation, ScmMember, ScmMembership,
    ScmPersonReach, ScmPersonRepo, ScmPersonToken, ScmProfile, ScmUser, SignInCode, SignInRequest,
    SignedIn,
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
pub(crate) struct Installation {
    id: u64,
    account: Account,
    #[serde(default)]
    permissions: BTreeMap<String, String>,
    suspended_at: Option<String>,
}

impl Installation {
    pub(crate) fn neutral(&self) -> ScmInstallation {
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

/// A repository that a person reads, with the permissions of the person.
#[derive(Deserialize)]
struct Repository {
    id: u64,
    #[serde(default)]
    permissions: RepositoryPermissions,
}

#[derive(Default, Deserialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the permissions of a repository, as GitHub gives them"
)]
struct RepositoryPermissions {
    #[serde(default)]
    admin: bool,
    #[serde(default)]
    maintain: bool,
    #[serde(default)]
    push: bool,
    #[serde(default)]
    triage: bool,
    #[serde(default)]
    pull: bool,
}

impl RepositoryPermissions {
    /// `maintain` acts as `push`, and `triage` as `pull`: CIHub has no
    /// action between them.
    fn permission(&self) -> Permission {
        if self.admin {
            Permission::Admin
        } else if self.maintain || self.push {
            Permission::Write
        } else if self.triage || self.pull {
            Permission::Read
        } else {
            Permission::None
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

/// What `GET /app` gives, read once: it does not change while the server
/// runs.
pub(crate) struct AppPage {
    /// `<html_url of the App>/installations/new`: GitHub shows the accounts
    /// where the App is not installed yet, and the repositories of an
    /// installation that exists.
    pub(crate) install_url: String,
    /// `@<slug of the App>`, which starts a command in a comment.
    pub(crate) mention: String,
}

impl GithubApp {
    pub(crate) async fn read_app_page(&self) -> Result<&AppPage, ScmError> {
        #[derive(Deserialize)]
        struct App {
            html_url: String,
            slug: String,
        }

        if let Some(page) = self.app_page.get() {
            return Ok(page);
        }

        let (answer, _) = self.send_as_app(Method::GET, "/app").await?;
        let app: App = expect(answer, &[StatusCode::OK])?.json()?;
        let page = AppPage {
            install_url: format!("{}/installations/new", app.html_url.trim_end_matches('/')),
            mention: format!("@{}", app.slug),
        };

        Ok(self.app_page.get_or_init(|| page))
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

    pub(crate) async fn sign_in_person(
        &self,
        code: &SignInCode,
    ) -> Result<(SignedIn, ScmPersonToken), ScmError> {
        let token = self.exchange_sign_in_code(code).await?;
        let access = &token.access;
        let (answer, _) = self.get_as_person(access, "/user").await?;
        let user: Person = expect(answer, &[StatusCode::OK])?.json()?;
        let listed = self.installations_of_person(access).await?;
        let installations = listed.iter().map(Installation::neutral).collect();
        let mut memberships = Vec::new();

        for installation in listed {
            let role = self
                .role_of_person(access, &user, &installation.account)
                .await?;
            if let Some(role) = role {
                memberships.push(ScmMembership {
                    connection_external_id: installation.id.to_string(),
                    role,
                });
            }
        }

        let signed_in = SignedIn {
            user: user.user(),
            profile: ScmProfile {
                name: user.name,
                avatar_url: user.avatar_url,
            },
            installations,
            memberships,
        };
        Ok((signed_in, token))
    }

    /// The installations that the person of `token` reaches, and the
    /// repositories of each one with the permission of the person.
    pub(crate) async fn reach_of_person(
        &self,
        token: &SecretString,
    ) -> Result<ScmPersonReach, ScmError> {
        #[derive(Deserialize)]
        struct Page {
            repositories: Vec<Repository>,
        }

        let listed = self.installations_of_person(token).await?;
        let mut repos = Vec::new();
        for installation in &listed {
            let mut path = Some(format!(
                "/user/installations/{}/repositories?per_page=100",
                installation.id
            ));
            while let Some(page) = path.take() {
                let (answer, next) = self.get_as_person(token, &page).await?;
                let page: Page = expect(answer, &[StatusCode::OK])?.json()?;
                repos.extend(page.repositories.into_iter().map(|repo| ScmPersonRepo {
                    connection_external_id: installation.id.to_string(),
                    repo_external_id: repo.id.to_string(),
                    permission: repo.permissions.permission(),
                }));
                path = next;
            }
        }

        Ok(ScmPersonReach {
            installations: listed.iter().map(Installation::neutral).collect(),
            repos: repos
                .into_iter()
                .filter(|repo| repo.permission != Permission::None)
                .collect(),
        })
    }

    /// The permission of the person of `token` on `repo`: GitHub answers
    /// `404` for a repository that the person does not see.
    pub(crate) async fn permission_of_person(
        &self,
        token: &SecretString,
        repo: &RepoRef,
    ) -> Result<Permission, ScmError> {
        let path = format!("/repos/{}/{}", encode(&repo.namespace), encode(&repo.name));
        let (answer, _) = self.get_as_person(token, &path).await?;
        if answer.status == StatusCode::NOT_FOUND {
            return Ok(Permission::None);
        }

        let repo: Repository = expect(answer, &[StatusCode::OK])?.json()?;
        Ok(repo.permissions.permission())
    }

    /// The role of `person` in `account`, with the token of the person: the
    /// owner of the account of a person is its `admin`, and an organization
    /// gives the role of an active member.
    pub(crate) async fn role_in_account(
        &self,
        token: &SecretString,
        person: &ScmUser,
        account: &ScmAccount,
    ) -> Result<Option<MembershipRole>, ScmError> {
        if account.external_id == person.external_id {
            return Ok(Some(MembershipRole::Admin));
        }

        self.role_in_organization(token, &account.path).await
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
        match account.kind.as_str() {
            "Organization" => {}
            "User" => return Ok((account.id == person.id).then_some(MembershipRole::Admin)),
            _ => return Ok(None),
        }

        self.role_in_organization(token, &account.login).await
    }

    /// The role of the person of `token` in the organization `login`: an
    /// active membership gives it; `None` when GitHub refuses the read.
    async fn role_in_organization(
        &self,
        token: &SecretString,
        login: &str,
    ) -> Result<Option<MembershipRole>, ScmError> {
        #[derive(Deserialize)]
        struct Membership {
            state: String,
            role: String,
        }

        let path = format!("/user/memberships/orgs/{}", encode(login));
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

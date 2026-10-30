//! A person on GitHub: the sign-in through the App and its tokens, the
//! installations that the person reaches and their role in each one, the
//! permission of a person on a repository, and the members and the profile of
//! the account of an installation.

use cihub_domain::{JobClass, MembershipRole, Permission};
use cihub_scm::{
    ConnectionRef, RepoRef, ScmAccount, ScmAccountProfile, ScmError, ScmMember, ScmMembership,
    ScmPersonReach, ScmPersonRepo, ScmPersonToken, ScmProfile, ScmUser, SignInCode, SignInRequest,
    SignedIn,
};
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use reqwest::{Method, StatusCode, Url};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use time::Duration;

use crate::GithubApp;
use crate::apps::{Account, Installation, read_installation};
use crate::client::{Auth, PER_PAGE, encode, fetch};

/// Where the dashboard sends a person to sign in through GitHub.
pub const SIGN_IN_PATH: &str = "/auth/github";
/// Where GitHub sends the person back; the *Callback URL* of the App.
pub const SIGN_IN_CALLBACK_PATH: &str = "/auth/github/callback";
/// Where GitHub sends an owner after an install or an update of the App; the
/// *Setup URL* of the App. Its `installation_id` proves nothing: the sign-in
/// that it starts again does.
pub const SETUP_PATH: &str = "/auth/github/installed";

/// A user of GitHub.
#[derive(Deserialize)]
struct Person {
    id: u64,
    login: String,
    /// Only `/user` gives it.
    name: Option<String>,
    avatar_url: Option<String>,
}

impl Person {
    /// The person in the neutral terms of `cihub-scm`.
    fn user(&self) -> ScmUser {
        ScmUser {
            external_id: self.id.to_string(),
            login: self.login.clone(),
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

/// The permissions of a person on a repository, as GitHub gives them.
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

/// The permission of `user` on `repo`, kept a short time.
pub(crate) async fn permission(
    app: &GithubApp,
    repo: &RepoRef,
    user: &ScmUser,
    class: JobClass,
) -> Result<Permission, ScmError> {
    #[derive(Deserialize)]
    struct Answer {
        permission: String,
    }

    let key = (
        repo.connection.id,
        format!("{}/{}", repo.owner, repo.name),
        user.login.clone(),
    );
    if let Some(permission) = app.kept_permission(&key) {
        return Ok(permission);
    }

    let rest = format!("/collaborators/{}/permission", encode(&user.login));
    let answer = app.rest(repo, class, Method::GET, &rest, None).await?;
    let permission = if answer.status == StatusCode::NOT_FOUND {
        Permission::None
    } else {
        let answer = answer.expect_status(&[StatusCode::OK])?;
        match answer.json::<Answer>()?.permission.as_str() {
            "admin" => Permission::Admin,
            "write" | "maintain" => Permission::Write,
            "read" | "triage" => Permission::Read,
            _ => Permission::None,
        }
    };
    app.keep_permission(key, permission);

    Ok(permission)
}

/// The page of GitHub where a person signs in; `None` without a sign-in client.
pub(crate) fn sign_in_url(app: &GithubApp, request: &SignInRequest) -> Option<String> {
    let client = app.sign_in.as_ref()?;
    let address = Url::parse_with_params(
        &format!("{}/login/oauth/authorize", app.web_url),
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

/// Ends a sign-in: the token of the person, who they are, the installations
/// that they reach and their roles.
pub(crate) async fn sign_in(
    app: &GithubApp,
    code: &SignInCode,
) -> Result<(SignedIn, ScmPersonToken), ScmError> {
    let token = exchange_sign_in_code(app, code).await?;
    let access = &token.access;
    let answer = app
        .send(Auth::Person(access), Method::GET, "/user", None)
        .await?;
    let user: Person = answer.expect_status(&[StatusCode::OK])?.json()?;
    let listed = installations_of_person(app, access).await?;
    let installations = listed.iter().map(Installation::neutral).collect();
    let mut memberships = Vec::new();

    for installation in listed {
        let role = role_of_person(app, access, &user, &installation.account).await?;
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

/// Trades `refresh` for a new token of its person. GitHub answers a
/// refused refresh token with `200` and an `error` field.
pub(crate) async fn refresh_person_token(
    app: &GithubApp,
    refresh: &SecretString,
) -> Result<ScmPersonToken, ScmError> {
    let token = ask_person_token(
        app,
        serde_json::json!({
            "grant_type": "refresh_token",
            "refresh_token": refresh.expose_secret(),
        }),
    )
    .await?;

    token.map_err(|error| {
        ScmError::AccessLost(format!(
            "the refresh token of the person is refused: {error}"
        ))
    })
}

/// Exchanges the code of a sign-in for the token of the person. GitHub
/// answers a refused code with `200` and an `error` field.
async fn exchange_sign_in_code(
    app: &GithubApp,
    code: &SignInCode,
) -> Result<ScmPersonToken, ScmError> {
    let token = ask_person_token(
        app,
        serde_json::json!({
            "code": code.code.expose_secret(),
            "code_verifier": code.code_verifier.expose_secret(),
            "redirect_uri": code.redirect_url,
        }),
    )
    .await?;

    token.map_err(|error| {
        ScmError::Validation(format!("the code of the sign-in is refused: {error}"))
    })
}

/// Asks `/login/oauth/access_token` for a token of a person, with the
/// client of the App and `grant`; the inner error is the refusal of
/// GitHub.
async fn ask_person_token(
    app: &GithubApp,
    grant: serde_json::Value,
) -> Result<Result<ScmPersonToken, String>, ScmError> {
    #[derive(Deserialize)]
    struct Exchanged {
        access_token: Option<String>,
        expires_in: Option<i64>,
        refresh_token: Option<String>,
        refresh_token_expires_in: Option<i64>,
        error: Option<String>,
    }

    let client = app.sign_in.as_ref().ok_or_else(|| {
        ScmError::Forbidden("the GitHub App has no client for the sign-in".into())
    })?;
    let mut body = grant;
    body["client_id"] = client.client_id.clone().into();
    body["client_secret"] = client.client_secret.expose_secret().into();
    let request = app
        .http
        .post(format!("{}/login/oauth/access_token", app.web_url))
        .header(ACCEPT, "application/json")
        .header(CONTENT_TYPE, "application/json")
        .body(body.to_string());
    let answer = fetch(request).await?;
    if answer.status.is_server_error() {
        return Err(ScmError::Unavailable(answer.message()));
    }

    let exchanged: Exchanged = answer.json()?;
    Ok(match (exchanged.access_token, exchanged.error) {
        (Some(token), None) => Ok(ScmPersonToken {
            access: SecretString::from(token),
            access_lifetime: exchanged.expires_in.map(Duration::seconds),
            refresh: exchanged.refresh_token.map(SecretString::from),
            refresh_lifetime: exchanged.refresh_token_expires_in.map(Duration::seconds),
        }),
        (_, error) => Err(error.unwrap_or_else(|| "no token".to_owned())),
    })
}

/// The installations that the person of `token` reaches, and the
/// repositories of each one with the permission of the person.
pub(crate) async fn person_reach(
    app: &GithubApp,
    token: &SecretString,
) -> Result<ScmPersonReach, ScmError> {
    #[derive(Deserialize)]
    struct Page {
        repositories: Vec<Repository>,
    }

    let listed = installations_of_person(app, token).await?;
    let mut repos = Vec::new();
    for installation in &listed {
        let path = format!(
            "/user/installations/{}/repositories?per_page={PER_PAGE}",
            installation.id
        );
        let pages: Vec<Page> = app.linked_pages(Auth::Person(token), path).await?;
        let listed = pages.into_iter().flat_map(|page| page.repositories);
        repos.extend(listed.map(|repo| ScmPersonRepo {
            connection_external_id: installation.id.to_string(),
            repo_external_id: repo.id.to_string(),
            permission: repo.permissions.permission(),
        }));
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
pub(crate) async fn person_permission(
    app: &GithubApp,
    token: &SecretString,
    repo: &RepoRef,
) -> Result<Permission, ScmError> {
    let path = format!("/repos/{}/{}", encode(&repo.owner), encode(&repo.name));
    let answer = app
        .send(Auth::Person(token), Method::GET, &path, None)
        .await?;
    if answer.status == StatusCode::NOT_FOUND {
        return Ok(Permission::None);
    }

    let repo: Repository = answer.expect_status(&[StatusCode::OK])?.json()?;
    Ok(repo.permissions.permission())
}

/// The role of `person` in `account`, with the token of the person: the
/// owner of the account of a person is its `admin`, and an organization
/// gives the role of an active member.
pub(crate) async fn person_role(
    app: &GithubApp,
    token: &SecretString,
    person: &ScmUser,
    account: &ScmAccount,
) -> Result<Option<MembershipRole>, ScmError> {
    if account.external_id == person.external_id {
        return Ok(Some(MembershipRole::Admin));
    }

    role_in_organization(app, token, &account.path).await
}

/// The installations of the App that the person of `token` reaches.
async fn installations_of_person(
    app: &GithubApp,
    token: &SecretString,
) -> Result<Vec<Installation>, ScmError> {
    #[derive(Deserialize)]
    struct Page {
        installations: Vec<Installation>,
    }

    let path = format!("/user/installations?per_page={PER_PAGE}");
    let pages: Vec<Page> = app.linked_pages(Auth::Person(token), path).await?;

    Ok(pages
        .into_iter()
        .flat_map(|page| page.installations)
        .collect())
}

/// The role of `person` in `account`: the owner of the account of a
/// person is its `admin`; an organization gives the role of an active
/// member. `None` for anyone else, for another kind of account, and when
/// GitHub refuses the read (single sign-on not authorized, allow list of
/// addresses): the sign-in deletes nothing, so the hourly read of the
/// members decides.
async fn role_of_person(
    app: &GithubApp,
    token: &SecretString,
    person: &Person,
    account: &Account,
) -> Result<Option<MembershipRole>, ScmError> {
    match account.kind.as_str() {
        "Organization" => {}
        "User" => return Ok((account.id == person.id).then_some(MembershipRole::Admin)),
        _ => return Ok(None),
    }

    match role_in_organization(app, token, &account.login).await {
        Err(ScmError::Forbidden(_)) => Ok(None),
        role => role,
    }
}

/// The role of the person of `token` in the organization `login`: an
/// active membership gives it; `None` for no membership, and when the
/// membership is not active. A `403` (single sign-on not authorized,
/// allow list of addresses) does not tell: it is `Forbidden`.
async fn role_in_organization(
    app: &GithubApp,
    token: &SecretString,
    login: &str,
) -> Result<Option<MembershipRole>, ScmError> {
    #[derive(Deserialize)]
    struct Membership {
        state: String,
        role: String,
    }

    let path = format!("/user/memberships/orgs/{}", encode(login));
    let answer = app
        .send(Auth::Person(token), Method::GET, &path, None)
        .await?;
    if answer.status == StatusCode::NOT_FOUND {
        return Ok(None);
    }

    let membership: Membership = answer.expect_status(&[StatusCode::OK])?.json()?;
    if membership.state != "active" {
        return Ok(None);
    }

    Ok(membership_role(&membership.role))
}

/// The members of the account of `connection`. The account comes from a
/// read of the installation, so that its kind and its login are those of
/// now, not those of its first sync.
pub(crate) async fn members(
    app: &GithubApp,
    connection: &ConnectionRef,
    class: JobClass,
) -> Result<Vec<ScmMember>, ScmError> {
    let account = read_installation(app, &connection.external_id)
        .await?
        .account;

    match account.kind.as_str() {
        "Organization" => {}
        "User" => return Ok(vec![owner(account)]),
        _ => return Ok(Vec::new()),
    }

    let mut members =
        members_with_role(app, connection, &account, MembershipRole::Admin, class).await?;
    let others =
        members_with_role(app, connection, &account, MembershipRole::Member, class).await?;

    members.extend(others);
    Ok(members)
}

/// The profile of the account of `connection`: an organization gives its
/// description, a person their bio. The account comes from a read of the
/// installation, as for [`members`].
pub(crate) async fn account_profile(
    app: &GithubApp,
    connection: &ConnectionRef,
    class: JobClass,
) -> Result<ScmAccountProfile, ScmError> {
    #[derive(Deserialize)]
    struct Profile {
        name: Option<String>,
        #[serde(alias = "bio")]
        description: Option<String>,
        avatar_url: Option<String>,
    }

    let account = read_installation(app, &connection.external_id)
        .await?
        .account;

    let path = match account.kind.as_str() {
        "Organization" => format!("/orgs/{}", encode(&account.login)),
        "User" => format!("/users/{}", encode(&account.login)),
        _ => {
            return Ok(ScmAccountProfile {
                avatar_url: account.avatar_url,
                ..ScmAccountProfile::default()
            });
        }
    };
    let auth = Auth::Installation(connection, class);
    let answer = app.send(auth, Method::GET, &path, None).await?;
    let profile: Profile = answer.expect_status(&[StatusCode::OK])?.json()?;

    Ok(ScmAccountProfile {
        name: profile.name,
        description: profile.description,
        avatar_url: profile.avatar_url,
    })
}

/// The members of the organization `account` with `role`, every page.
async fn members_with_role(
    app: &GithubApp,
    connection: &ConnectionRef,
    account: &Account,
    role: MembershipRole,
    class: JobClass,
) -> Result<Vec<ScmMember>, ScmError> {
    let filter = match role {
        MembershipRole::Admin => "admin",
        MembershipRole::Member => "member",
    };
    let path = format!("/orgs/{}/members?role={filter}", encode(&account.login));
    let auth = Auth::Installation(connection, class);
    let listed: Vec<Person> = app.numbered_pages(auth, &path, usize::MAX).await?;

    Ok(listed
        .into_iter()
        .map(|person| ScmMember {
            user: person.user(),
            role,
            avatar_url: person.avatar_url,
        })
        .collect())
}

/// The role of a membership of an organization.
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

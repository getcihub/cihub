use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use cihub_domain::{JobClass, MembershipRole, Permission};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};

use crate::{RepoRef, ScmAccount, ScmFuture, ScmInstallation, ScmUser};

/// The calls for a person: the sign-in, and the reads with their token. No
/// connection makes them, so no budget of a connection applies: the calls
/// with the token of a person spend the rate limit of the person.
pub trait PersonCalls: Send + Sync {
    /// The address of the SCM where a person signs in to CIHub, or `None`
    /// when the operator configured no sign-in.
    fn sign_in_url(&self, request: &SignInRequest) -> Option<String>;

    /// Exchanges the code of a sign-in, reads the person and the
    /// installations of the App that they reach, with their role in each, and
    /// gives the token of the person. An installation where the person has no
    /// role has no membership.
    fn sign_in<'a>(
        &'a self,
        code: &'a SignInCode,
        class: JobClass,
    ) -> ScmFuture<'a, (SignedIn, ScmPersonToken)>;

    /// A new token for the person of `refresh`, which then works no more.
    /// `AccessLost` when the SCM refuses it: the person signs in again.
    fn refresh_person_token<'a>(
        &'a self,
        refresh: &'a SecretString,
        class: JobClass,
    ) -> ScmFuture<'a, ScmPersonToken>;

    /// The installations of the App that the person of `token` reaches, and
    /// the repositories of each one with their permission. `AccessLost` when
    /// the SCM refuses the token.
    fn person_reach<'a>(
        &'a self,
        token: &'a SecretString,
        class: JobClass,
    ) -> ScmFuture<'a, ScmPersonReach>;

    /// The permission of the person of `token` on `repo`; `Permission::None`
    /// when they do not see it. `AccessLost` when the SCM refuses the token.
    fn person_permission<'a>(
        &'a self,
        token: &'a SecretString,
        repo: &'a RepoRef,
        class: JobClass,
    ) -> ScmFuture<'a, Permission>;

    /// The role of `person`, whose token is `token`, in `account`, looked up
    /// by `account.path`, the login of the account now; `None` without one.
    /// The owner of the account of a person is its `admin`. `AccessLost` when
    /// the SCM refuses the token; `Forbidden` when it refuses the read of the
    /// membership (single sign-on not authorized): that is not `None`, so
    /// the caller keeps what it knows.
    fn person_role<'a>(
        &'a self,
        token: &'a SecretString,
        person: &'a ScmUser,
        account: &'a ScmAccount,
        class: JobClass,
    ) -> ScmFuture<'a, Option<MembershipRole>>;
}

/// What the address of a sign-in carries: the SCM sends the person back to
/// `redirect_url` with a code and `state`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignInRequest {
    /// The address of CIHub that receives the callback.
    pub redirect_url: String,
    /// A random value that the callback checks, against a forged callback.
    pub state: String,
    /// The SHA-256 of the code verifier, in base64url without padding
    /// (PKCE, method `S256`).
    pub code_challenge: String,
}

/// The challenge of `verifier` for PKCE, method `S256`: the SHA-256 of the
/// verifier, in base64url without padding (RFC 7636).
pub fn code_challenge(verifier: &SecretString) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.expose_secret().as_bytes()))
}

/// The code that the SCM gives on the callback of a sign-in, and the
/// verifier that proves that CIHub asked for it.
#[derive(Debug)]
pub struct SignInCode {
    /// The code of the callback, which works once.
    pub code: SecretString,
    /// The random value whose challenge the request carried (PKCE).
    pub code_verifier: SecretString,
    /// The `redirect_url` of the request, which the SCM checks again.
    pub redirect_url: String,
}

/// A person who signed in, the installations of the App that they reach,
/// and their role in each one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedIn {
    /// The person who signed in.
    pub user: ScmUser,
    /// The name and the avatar of the person.
    pub profile: ScmProfile,
    /// Every installation that the person reaches, as a sync reads it: the
    /// sign-in connects those that CIHub does not know yet.
    pub installations: Vec<ScmInstallation>,
    /// The role of the person in the account of each installation where
    /// they have one.
    pub memberships: Vec<ScmMembership>,
}

/// The token of a person on the SCM, which reads what the person reaches, and
/// how long it and its refresh last.
#[derive(Debug)]
pub struct ScmPersonToken {
    /// The token that the reads of the person send.
    pub access: SecretString,
    /// `None` for a token that does not expire.
    pub access_lifetime: Option<time::Duration>,
    /// Gives a new token once, then no more.
    pub refresh: Option<SecretString>,
    /// How long `refresh` works; `None` when it does not expire, or when
    /// there is no refresh.
    pub refresh_lifetime: Option<time::Duration>,
}

/// What the token of a person reaches: the installations of the App, and the
/// repositories of each one with the permission of the person.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScmPersonReach {
    /// The installations of the App that the person reaches.
    pub installations: Vec<ScmInstallation>,
    /// The repositories of those installations where the person has a
    /// permission.
    pub repos: Vec<ScmPersonRepo>,
}

/// A repository that a person reaches through an installation of the App.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScmPersonRepo {
    /// The installation, as the `external_id` of its connection.
    pub connection_external_id: String,
    /// The repository on the SCM.
    pub repo_external_id: String,
    /// What the person may do on the repository; never `Permission::None`.
    pub permission: Permission,
}

/// What a person shows of themselves on the SCM.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScmProfile {
    /// The display name; `None` when the person sets none.
    pub name: Option<String>,
    /// The address of the image of the person; `None` when the SCM gives
    /// none.
    pub avatar_url: Option<String>,
}

/// The role of a person in the account of an installation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScmMembership {
    /// The installation, as the `external_id` of its connection.
    pub connection_external_id: String,
    /// What the person may do in the account.
    pub role: MembershipRole,
}

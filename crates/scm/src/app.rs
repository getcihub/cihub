use std::collections::BTreeMap;

use cihub_domain::{JobClass, MembershipRole};
use time::OffsetDateTime;

use crate::{ConnectionRef, ScmFuture, ScmUser};

/// The calls of the App of CIHub: its installations, its webhook
/// deliveries, and what a connection reaches.
pub trait AppCalls: Send + Sync {
    /// The webhook deliveries that the SCM sent to CIHub since `since`, each
    /// attempt once with its outcome, for every connection. No connection makes
    /// the call, so no budget of a connection applies.
    fn recent_deliveries(
        &self,
        since: OffsetDateTime,
        class: JobClass,
    ) -> ScmFuture<'_, Vec<RecentDelivery>>;

    /// Asks the SCM to send the attempt `attempt_id` of a delivery again.
    fn redeliver<'a>(&'a self, attempt_id: &'a str, class: JobClass) -> ScmFuture<'a, ()>;

    /// The installation `external_id`, or `None` when the SCM does not know it.
    /// No connection of CIHub may exist yet, so no budget of a connection
    /// applies.
    fn installation<'a>(
        &'a self,
        external_id: &'a str,
        class: JobClass,
    ) -> ScmFuture<'a, Option<ScmInstallation>>;

    /// Every installation of the App, suspended ones included.
    fn installations(&self, class: JobClass) -> ScmFuture<'_, Vec<ScmInstallation>>;

    /// Every repository that `connection` reaches.
    fn list_repos<'a>(
        &'a self,
        connection: &'a ConnectionRef,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<ScmRepo>>;

    /// The members of the account of `connection`, with their role. The
    /// owner of the account of a person is its only member, as `admin`.
    fn members<'a>(
        &'a self,
        connection: &'a ConnectionRef,
        class: JobClass,
    ) -> ScmFuture<'a, Vec<ScmMember>>;

    /// The name, the description and the avatar of the account of
    /// `connection`.
    fn account_profile<'a>(
        &'a self,
        connection: &'a ConnectionRef,
        class: JobClass,
    ) -> ScmFuture<'a, ScmAccountProfile>;

    /// The page of the SCM where an owner installs the App on an account, or
    /// chooses the repositories of its installation. No connection makes the
    /// call, so no budget of a connection applies.
    fn app_install_url(&self, class: JobClass) -> ScmFuture<'_, String>;
}

/// An attempt of the SCM to send a webhook delivery to CIHub.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentDelivery {
    /// The identifier that the delivery carries; its new attempts share it.
    pub delivery_id: String,
    /// The identifier of this attempt, to ask for the delivery again.
    pub attempt_id: String,
    /// Whether CIHub answered this attempt with a 2xx status code, which it
    /// does only once it stores the delivery.
    pub is_delivered: bool,
}

/// The account of the SCM that a connection serves: an organization or a
/// user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScmAccount {
    /// The account on the SCM; it never changes.
    pub external_id: String,
    /// The name of the account in the paths of the SCM.
    pub path: String,
}

/// What an account shows of itself on the SCM.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScmAccountProfile {
    /// The display name; `None` when the account sets none.
    pub name: Option<String>,
    /// The short text that the account shows; `None` when it sets none.
    pub description: Option<String>,
    /// The address of the image of the account; `None` when the SCM gives
    /// none.
    pub avatar_url: Option<String>,
}

/// An installation of the App of CIHub on an account, as the SCM reads it: the
/// permission name gives its access level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScmInstallation {
    /// The installation on the SCM: the `external_id` of its connection.
    pub external_id: String,
    /// The account where the App is installed.
    pub account: ScmAccount,
    /// The access level of the App for each permission name, as the SCM
    /// gives them. CIHub stores them and does not read them.
    pub permissions: BTreeMap<String, String>,
    /// The owner of the account suspended the installation: its calls fail.
    pub is_suspended: bool,
}

/// A member of the account of a connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScmMember {
    /// The member.
    pub user: ScmUser,
    /// What the member may do in the account.
    pub role: MembershipRole,
    /// The list of the members does not give the name.
    pub avatar_url: Option<String>,
}

/// A repository that a connection reaches, as the SCM lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScmRepo {
    /// The repository on the SCM; it stays the same across a rename.
    pub external_id: String,
    /// The account that holds the repository, as the paths of the SCM name it.
    pub owner: String,
    /// The name of the repository in its account.
    pub name: String,
    /// The short name of the branch that the repository opens on.
    pub default_branch: String,
}

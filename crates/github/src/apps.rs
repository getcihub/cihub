//! The App itself: its installations, the repositories that each one
//! covers, and its webhook deliveries.

use std::collections::BTreeMap;

use cihub_domain::JobClass;
use cihub_scm::{ConnectionRef, RecentDelivery, ScmAccount, ScmError, ScmInstallation, ScmRepo};
use reqwest::{Method, StatusCode};
use serde::Deserialize;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::GithubApp;
use crate::client::{Auth, PER_PAGE, encode};

/// An installation of the App, as GitHub gives it.
#[derive(Deserialize)]
pub(crate) struct Installation {
    pub id: u64,
    pub account: Account,
    #[serde(default)]
    permissions: BTreeMap<String, String>,
    suspended_at: Option<String>,
}

impl Installation {
    /// The installation in the neutral terms of `cihub-scm`.
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

/// The account where the App is installed: an organization or a person.
#[derive(Deserialize)]
pub(crate) struct Account {
    pub id: u64,
    pub login: String,
    pub avatar_url: Option<String>,
    #[serde(rename = "type")]
    pub kind: String,
}

/// An attempt to deliver a webhook, as GitHub lists it.
#[derive(Deserialize)]
struct DeliveryAttempt {
    id: u64,
    guid: String,
    delivered_at: String,
    status_code: u16,
}

/// The attempts of a page delivered at `since` or later, and whether the page
/// reaches an older one: the pages after it hold only older attempts.
fn recent_attempts(
    attempts: Vec<DeliveryAttempt>,
    since: OffsetDateTime,
) -> Result<(Vec<RecentDelivery>, bool), ScmError> {
    let mut recent = Vec::new();
    for attempt in attempts {
        let at = OffsetDateTime::parse(&attempt.delivered_at, &Rfc3339)
            .map_err(|err| ScmError::Protocol(err.to_string()))?;
        if at < since {
            return Ok((recent, true));
        }

        recent.push(RecentDelivery {
            delivery_id: attempt.guid,
            attempt_id: attempt.id.to_string(),
            is_delivered: (200..300).contains(&attempt.status_code),
        });
    }

    Ok((recent, false))
}

/// The delivery attempts of the webhooks of the App since `since`.
pub(crate) async fn recent_deliveries(
    app: &GithubApp,
    since: OffsetDateTime,
) -> Result<Vec<RecentDelivery>, ScmError> {
    let mut path = Some(format!("/app/hook/deliveries?per_page={PER_PAGE}"));
    let mut recent = Vec::new();
    while let Some(page) = path.take() {
        let answer = app.send(Auth::App, Method::GET, &page, None).await?;
        let answer = answer.expect_status(&[StatusCode::OK])?;
        let (page_recent, has_older) = recent_attempts(answer.json()?, since)?;
        recent.extend(page_recent);

        if !has_older {
            path = answer.next;
        }
    }

    Ok(recent)
}

/// Asks GitHub to deliver the attempt `attempt_id` again.
pub(crate) async fn redeliver(app: &GithubApp, attempt_id: &str) -> Result<(), ScmError> {
    let path = format!("/app/hook/deliveries/{}/attempts", encode(attempt_id));
    let answer = app.send(Auth::App, Method::POST, &path, None).await?;
    answer.expect_status(&[StatusCode::ACCEPTED]).map(drop)
}

/// The installation `external_id`, as GitHub gives it.
pub(crate) async fn read_installation(
    app: &GithubApp,
    external_id: &str,
) -> Result<Installation, ScmError> {
    let path = format!("/app/installations/{}", encode(external_id));
    let answer = app.send(Auth::App, Method::GET, &path, None).await?;
    answer.expect_status(&[StatusCode::OK])?.json()
}

/// The installation `external_id`, or `None` when GitHub does not know it.
pub(crate) async fn installation(
    app: &GithubApp,
    external_id: &str,
) -> Result<Option<ScmInstallation>, ScmError> {
    match read_installation(app, external_id).await {
        Ok(installation) => Ok(Some(installation.neutral())),
        Err(ScmError::NotFound(_)) => Ok(None),
        Err(err) => Err(err),
    }
}

/// Every installation of the App.
pub(crate) async fn installations(app: &GithubApp) -> Result<Vec<ScmInstallation>, ScmError> {
    let path = format!("/app/installations?per_page={PER_PAGE}");
    let pages: Vec<Vec<Installation>> = app.linked_pages(Auth::App, path).await?;

    Ok(pages.iter().flatten().map(Installation::neutral).collect())
}

/// The repositories that the installation of `connection` covers.
pub(crate) async fn list_repos(
    app: &GithubApp,
    connection: &ConnectionRef,
    class: JobClass,
) -> Result<Vec<ScmRepo>, ScmError> {
    #[derive(Deserialize)]
    struct Page {
        total_count: usize,
        repositories: Vec<Repository>,
    }

    #[derive(Deserialize)]
    struct Repository {
        id: u64,
        name: String,
        owner: Owner,
        default_branch: String,
    }

    #[derive(Deserialize)]
    struct Owner {
        login: String,
    }

    let auth = Auth::Installation(connection, class);
    let mut repos = Vec::new();
    for page in 1.. {
        let path = format!("/installation/repositories?per_page={PER_PAGE}&page={page}");
        let answer = app.send(auth, Method::GET, &path, None).await?;
        let Page {
            total_count,
            repositories,
        } = answer.expect_status(&[StatusCode::OK])?.json()?;
        let last = repositories.is_empty();

        repos.extend(repositories.into_iter().map(|repo| ScmRepo {
            external_id: repo.id.to_string(),
            owner: repo.owner.login,
            name: repo.name,
            default_branch: repo.default_branch,
        }));

        if last || repos.len() >= total_count {
            break;
        }
    }

    Ok(repos)
}

/// `<html_url of the App>/installations/new`: GitHub shows the accounts
/// where the App is not installed yet, and the repositories of an
/// installation that exists.
pub(crate) async fn app_install_url(app: &GithubApp) -> Result<String, ScmError> {
    #[derive(Deserialize)]
    struct App {
        html_url: String,
    }

    if let Some(url) = app.install_url.get() {
        return Ok(url.clone());
    }

    let answer = app.send(Auth::App, Method::GET, "/app", None).await?;
    let page: App = answer.expect_status(&[StatusCode::OK])?.json()?;
    let url = format!("{}/installations/new", page.html_url.trim_end_matches('/'));

    Ok(app.install_url.get_or_init(|| url).clone())
}

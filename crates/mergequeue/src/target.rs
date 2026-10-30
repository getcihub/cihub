use cihub_config::{Layer, QueueSettings, RepoConfig};
use cihub_domain::{OrgId, QueueId, RepoId};
use cihub_scm::{ConnectionRef, RepoRef};
use sqlx::PgConnection;

use crate::{MergeQueueError, parse_id};

/// A repository, as a job calls it through its connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RepoTarget {
    pub repo_id: RepoId,
    pub org_id: OrgId,
    pub repo: RepoRef,
    pub default_branch: String,
}

/// What a queue acts on: its repository and its target branch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct QueueTarget {
    pub queue_id: QueueId,
    pub org_id: OrgId,
    pub repo_id: RepoId,
    pub repo: RepoRef,
    pub target_branch: String,
    /// The last valid configuration file, and the head of the target at the
    /// last read.
    pub config_text: Option<String>,
    pub config_sha: Option<String>,
}

type Row = (
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    String,
    String,
    String,
    String,
    String,
    String,
);

/// The columns of [`Row`]: the queue, its repository and its connection.
const COLUMNS: &str =
    "q.id, q.org_id, q.target_branch, q.config_text, q.config_sha, r.id, r.external_id,
    r.owner, r.name, c.id, c.external_id
    FROM queues q
    JOIN repos r ON r.id = q.repo_id
    JOIN scm_connections c ON c.id = r.connection_id";

fn decode(row: Row) -> Result<QueueTarget, MergeQueueError> {
    let (
        queue,
        org,
        branch,
        config_text,
        config_sha,
        repo,
        external_id,
        owner,
        name,
        connection,
        cext,
    ) = row;
    Ok(QueueTarget {
        queue_id: parse_id(&queue, &queue)?,
        org_id: parse_id(&org, &queue)?,
        repo_id: parse_id(&repo, &queue)?,
        repo: RepoRef {
            connection: ConnectionRef {
                id: parse_id(&connection, &queue)?,
                external_id: cext,
            },
            external_id,
            owner,
            name,
        },
        target_branch: branch,
        config_text,
        config_sha,
    })
}

/// The settings of the queue of `branch` in `config`, its last valid file;
/// the defaults without a file.
///
/// # Errors
///
/// The file does not give valid settings for the queue.
pub(crate) fn queue_settings(
    config: Option<&RepoConfig>,
    branch: &str,
) -> Result<QueueSettings, MergeQueueError> {
    match config {
        Some(config) => config.settings(branch),
        None => QueueSettings::resolve(&Layer::default(), &Layer::default()),
    }
    .map_err(MergeQueueError::Settings)
}

impl QueueTarget {
    /// # Errors
    ///
    /// Postgres fails, the queue does not exist, or a row does not match the
    /// identifiers of `cihub-domain`.
    pub(crate) async fn load(
        conn: &mut PgConnection,
        queue: QueueId,
    ) -> Result<Self, MergeQueueError> {
        let row: Option<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {COLUMNS} WHERE q.id = $1"
        )))
        .bind(queue.to_string())
        .fetch_optional(conn)
        .await?;
        decode(row.ok_or(MergeQueueError::UnknownQueue(queue))?)
    }

    /// The last valid file of the queue; `None` without a file.
    ///
    /// # Errors
    ///
    /// The file is no longer valid: it was valid when the queue read it, so
    /// only a new version of CIHub can refuse it.
    pub(crate) fn config(&self) -> Result<Option<RepoConfig>, MergeQueueError> {
        self.config_text
            .as_deref()
            .map(RepoConfig::parse)
            .transpose()
            .map_err(MergeQueueError::Settings)
    }

    /// The settings of the queue in `config`, its last valid file.
    ///
    /// # Errors
    ///
    /// The file does not give valid settings for the queue.
    pub(crate) fn settings_in(
        &self,
        config: Option<&RepoConfig>,
    ) -> Result<QueueSettings, MergeQueueError> {
        queue_settings(config, &self.target_branch)
    }

    /// The settings of the queue, from its last valid file.
    ///
    /// # Errors
    ///
    /// As [`Self::config`] and [`Self::settings_in`].
    pub(crate) fn settings(&self) -> Result<QueueSettings, MergeQueueError> {
        self.settings_in(self.config()?.as_ref())
    }

    /// Whether the queue takes new submissions: its file names its branch,
    /// or its branch is `default_branch`. A queue that the file no longer
    /// names finishes its items.
    ///
    /// # Errors
    ///
    /// As [`Self::config`].
    pub(crate) fn takes_submissions(&self, default_branch: &str) -> Result<bool, MergeQueueError> {
        let is_named = self
            .config()?
            .is_some_and(|config| config.queue(&self.target_branch).is_some());
        Ok(is_named || self.target_branch == default_branch)
    }

    /// The queue of the branch `branch` of the repository `repo`, if any.
    ///
    /// # Errors
    ///
    /// Postgres fails, or a row does not match the identifiers of
    /// `cihub-domain`.
    pub(crate) async fn of_branch(
        conn: &mut PgConnection,
        repo: RepoId,
        branch: &str,
    ) -> Result<Option<Self>, MergeQueueError> {
        let row: Option<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {COLUMNS} WHERE r.id = $1 AND q.target_branch = $2"
        )))
        .bind(repo.to_string())
        .bind(branch)
        .fetch_optional(conn)
        .await?;
        row.map(decode).transpose()
    }
}

impl RepoTarget {
    /// # Errors
    ///
    /// Postgres fails, the repository does not exist, or a row does not
    /// match the identifiers of `cihub-domain`.
    pub(crate) async fn load(
        conn: &mut PgConnection,
        repo: RepoId,
    ) -> Result<Self, MergeQueueError> {
        type RepoRow = (String, String, String, String, String, String, String);
        let row: Option<RepoRow> = sqlx::query_as(
            "SELECT r.org_id, r.external_id, r.owner, r.name, r.default_branch,
                    c.id, c.external_id
             FROM repos r JOIN scm_connections c ON c.id = r.connection_id
             WHERE r.id = $1",
        )
        .bind(repo.to_string())
        .fetch_optional(conn)
        .await?;
        let (org, external_id, owner, name, default_branch, connection, cext) =
            row.ok_or(MergeQueueError::UnknownRepo(repo))?;

        Ok(Self {
            repo_id: repo,
            org_id: parse_id(&org, repo)?,
            repo: RepoRef {
                connection: ConnectionRef {
                    id: parse_id(&connection, repo)?,
                    external_id: cext,
                },
                external_id,
                owner,
                name,
            },
            default_branch,
        })
    }
}

/// The queue of the unit tests of the jobs.
#[cfg(test)]
pub(crate) fn test_target() -> QueueTarget {
    use ulid::Ulid;
    QueueTarget {
        queue_id: QueueId::from_ulid(Ulid(2)),
        org_id: OrgId::from_ulid(Ulid(1)),
        repo: RepoRef {
            connection: ConnectionRef {
                id: cihub_domain::ConnectionId::from_ulid(Ulid(4)),
                external_id: "4".into(),
            },
            external_id: "5".into(),
            owner: "acme".into(),
            name: "app".into(),
        },
        target_branch: "main".into(),
        config_text: None,
        config_sha: None,
        repo_id: RepoId::from_ulid(Ulid(6)),
    }
}

/// A PR of the unit tests of the jobs, with the checks `checks` as (name,
/// passed).
#[cfg(test)]
pub(crate) fn test_pr(
    mergeability: cihub_domain::Mergeability,
    review_decision: cihub_domain::ReviewDecision,
    checks: &[(&str, bool)],
) -> cihub_scm::PrSnapshot {
    use std::collections::BTreeSet;

    use cihub_domain::{CheckConclusion, CheckKind, CheckReport, CheckStatus, PrState};
    use time::OffsetDateTime;

    cihub_scm::PrSnapshot {
        number: 1,
        url: "https://scm.example/acme/api/prs/1".into(),
        state: PrState::Open,
        head_sha: "a".into(),
        base_branch: "main".into(),
        head_branch: "feature".into(),
        is_from_fork: false,
        created_at: OffsetDateTime::UNIX_EPOCH,
        is_draft: false,
        title: "PR 1".into(),
        body: String::new(),
        labels: BTreeSet::new(),
        mergeability,
        review_decision,
        approved_by: BTreeSet::new(),
        changes_requested_by: BTreeSet::new(),
        review_requested: BTreeSet::new(),
        checks: checks
            .iter()
            .map(|(name, passed)| CheckReport {
                name: (*name).into(),
                kind: CheckKind::Status,
                status: CheckStatus::Completed,
                conclusion: Some(if *passed {
                    CheckConclusion::Success
                } else {
                    CheckConclusion::Failure
                }),
                details_url: None,
                started_at: None,
                completed_at: None,
            })
            .collect(),
        author: cihub_scm::ScmUser {
            external_id: "1".into(),
            login: "octocat".into(),
        },
        additions: 0,
        deletions: 0,
        changed_files: 0,
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Write;

    use super::*;

    #[test]
    fn a_queue_takes_submissions_while_the_file_names_it() {
        let file = |branches: &[&str]| {
            let mut text = "version: 1\nqueues:\n".to_owned();
            for branch in branches {
                writeln!(text, "  - branch: {branch}").unwrap();
            }
            Some(text)
        };
        let cases = [
            // No file: only the queue of the default branch.
            ("main", None, true),
            ("release", None, false),
            // The file does not name the branch.
            ("main", file(&["release"]), true),
            ("dev", file(&["release"]), false),
            // The file names the branch.
            ("main", file(&["main"]), true),
            ("release", file(&["release"]), true),
        ];
        for (n, (branch, config_text, want)) in cases.into_iter().enumerate() {
            let queue = QueueTarget {
                target_branch: branch.into(),
                config_text,
                ..test_target()
            };
            assert_eq!(queue.takes_submissions("main").unwrap(), want, "case {n}");
        }
    }
}

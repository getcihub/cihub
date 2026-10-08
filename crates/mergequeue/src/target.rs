use cihub_config::{Layer, QueueSettings, RepoConfig};
use cihub_domain::{OrgId, QueueId, RepoId, ScmInstanceId};
use cihub_scm::{ConnectionRef, RepoRef};
use sqlx::PgConnection;

use crate::{MergeQueueError, parse_id};

/// A repository, as a job calls it through its connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RepoTarget {
    pub repo_id: RepoId,
    pub org_id: OrgId,
    pub scm_instance_id: ScmInstanceId,
    pub repo: RepoRef,
}

/// What a queue acts on: its repository and its target branch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct QueueTarget {
    pub queue_id: QueueId,
    pub org_id: OrgId,
    pub scm_instance_id: ScmInstanceId,
    pub repo_id: RepoId,
    pub repo: RepoRef,
    pub target_branch: String,
    pub name: String,
    /// The last valid configuration file, and the head of the target at the
    /// last read (`DECISIONS.md` D136).
    pub config_text: Option<String>,
    pub config_sha: Option<String>,
}

type Row = (
    String,
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
    String,
);

/// The columns of [`Row`]: the queue, its repository and its connection.
const COLUMNS: &str = "q.id, q.org_id, q.target_branch, q.name, q.config_text, q.config_sha, r.id, r.scm_instance_id, r.external_id,
    r.namespace, r.name, c.id, c.external_id
    FROM queues q
    JOIN repos r ON r.id = q.repo_id
    JOIN scm_connections c ON c.id = r.connection_id";

fn decode(row: Row) -> Result<QueueTarget, MergeQueueError> {
    let (
        queue,
        org,
        branch,
        queue_name,
        config_text,
        config_sha,
        repo,
        instance,
        external_id,
        namespace,
        name,
        connection,
        cext,
    ) = row;
    Ok(QueueTarget {
        queue_id: parse_id(&queue, &queue)?,
        org_id: parse_id(&org, &queue)?,
        scm_instance_id: parse_id(&instance, &queue)?,
        repo_id: parse_id(&repo, &queue)?,
        repo: RepoRef {
            connection: ConnectionRef {
                id: parse_id(&connection, &queue)?,
                external_id: cext,
            },
            external_id,
            namespace,
            name,
        },
        target_branch: branch,
        name: queue_name,
        config_text,
        config_sha,
    })
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
        .await
        .map_err(MergeQueueError::Query)?;
        decode(row.ok_or(MergeQueueError::UnknownQueue(queue))?)
    }

    /// The last valid file of the queue (`DECISIONS.md` D136); `None` without
    /// a file.
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
        match config {
            Some(config) => config.settings(&self.target_branch, &self.name),
            None => QueueSettings::resolve(&Layer::default(), &Layer::default()),
        }
        .map_err(MergeQueueError::Settings)
    }

    /// The settings of the queue, from its last valid file.
    ///
    /// # Errors
    ///
    /// As [`Self::config`] and [`Self::settings_in`].
    pub(crate) fn settings(&self) -> Result<QueueSettings, MergeQueueError> {
        self.settings_in(self.config()?.as_ref())
    }

    /// Whether the queue takes new submissions (`DECISIONS.md` D172): its
    /// file names it, or names no queue of its branch and the queue is the
    /// one of `default_branch`, named after it. A queue that the file no
    /// longer names finishes its items.
    ///
    /// # Errors
    ///
    /// As [`Self::config`].
    pub(crate) fn takes_submissions(&self, default_branch: &str) -> Result<bool, MergeQueueError> {
        let config = self.config()?;
        let mut names = config
            .iter()
            .flat_map(|config| &config.queues)
            .filter(|queue| queue.branch == self.target_branch)
            .map(|queue| queue.name.as_str())
            .peekable();
        Ok(if names.peek().is_none() {
            self.target_branch == default_branch && self.name == self.target_branch
        } else {
            names.any(|name| name == self.name)
        })
    }

    /// The queues of the branch `branch` of the repository `repo`, in the
    /// order of the configuration file, which is the order of the routing
    /// (`DECISIONS.md` D148); the queues that the file does not name come last,
    /// by age.
    ///
    /// # Errors
    ///
    /// Postgres fails, or a row does not match the identifiers of
    /// `cihub-domain`.
    pub(crate) async fn queues_of_branch(
        conn: &mut PgConnection,
        repo: RepoId,
        branch: &str,
    ) -> Result<Vec<Self>, MergeQueueError> {
        let rows: Vec<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {COLUMNS} WHERE r.id = $1 AND q.target_branch = $2 ORDER BY q.created_at, q.id"
        )))
        .bind(repo.to_string())
        .bind(branch)
        .fetch_all(conn)
        .await
        .map_err(MergeQueueError::Query)?;
        let mut queues = rows
            .into_iter()
            .map(decode)
            .collect::<Result<Vec<_>, _>>()?;
        let order: Vec<String> = queues
            .iter()
            .find_map(|queue| queue.config_text.as_deref())
            .and_then(|text| RepoConfig::parse(text).ok())
            .map(|config| {
                config
                    .queues
                    .into_iter()
                    .filter(|queue| queue.branch == branch)
                    .map(|queue| queue.name)
                    .collect()
            })
            .unwrap_or_default();
        queues.sort_by_key(|queue| {
            order
                .iter()
                .position(|name| *name == queue.name)
                .unwrap_or(usize::MAX)
        });
        Ok(queues)
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
            "SELECT r.org_id, r.scm_instance_id, r.external_id, r.namespace, r.name, c.id,
                    c.external_id
             FROM repos r JOIN scm_connections c ON c.id = r.connection_id
             WHERE r.id = $1",
        )
        .bind(repo.to_string())
        .fetch_optional(conn)
        .await
        .map_err(MergeQueueError::Query)?;
        let (org, instance, external_id, namespace, name, connection, cext) =
            row.ok_or(MergeQueueError::UnknownRepo(repo))?;
        Ok(Self {
            repo_id: repo,
            org_id: parse_id(&org, repo)?,
            scm_instance_id: parse_id(&instance, repo)?,
            repo: RepoRef {
                connection: ConnectionRef {
                    id: parse_id(&connection, repo)?,
                    external_id: cext,
                },
                external_id,
                namespace,
                name,
            },
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
        scm_instance_id: ScmInstanceId::from_ulid(Ulid(3)),
        repo: RepoRef {
            connection: ConnectionRef {
                id: cihub_domain::ConnectionId::from_ulid(Ulid(4)),
                external_id: "4".into(),
            },
            external_id: "5".into(),
            namespace: "acme".into(),
            name: "app".into(),
        },
        target_branch: "main".into(),
        name: "main".into(),
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

    use cihub_domain::{CheckConclusion, CheckKind, CheckReport, PrState};
    use time::OffsetDateTime;

    cihub_scm::PrSnapshot {
        number: 1,
        state: PrState::Open,
        head_sha: "a".into(),
        base_branch: "main".into(),
        head_branch: "feature".into(),
        from_fork: false,
        created_at: OffsetDateTime::UNIX_EPOCH,
        draft: false,
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
                conclusion: Some(if *passed {
                    CheckConclusion::Success
                } else {
                    CheckConclusion::Failure
                }),
                details_url: None,
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
        let file = |names: &[(&str, &str)]| {
            let mut text = "version: 1\nqueues:\n".to_owned();
            for (name, branch) in names {
                writeln!(text, "  - name: {name}\n    branch: {branch}").unwrap();
            }
            Some(text)
        };
        let cases = [
            // No file: only the queue of the default branch, named after it.
            ("main", "main", None, true),
            ("other", "main", None, false),
            ("release", "release", None, false),
            // The file names no queue of the branch.
            ("main", "main", file(&[("rel", "release")]), true),
            // The file names queues of the branch.
            ("main", "main", file(&[("main", "main")]), true),
            (
                "hotfix",
                "main",
                file(&[("main", "main"), ("hotfix", "main")]),
                true,
            ),
            ("main", "main", file(&[("fast", "main")]), false),
            ("rel", "release", file(&[("rel", "release")]), true),
        ];
        for (n, (name, branch, config_text, want)) in cases.into_iter().enumerate() {
            let queue = QueueTarget {
                name: name.into(),
                target_branch: branch.into(),
                config_text,
                ..test_target()
            };
            assert_eq!(queue.takes_submissions("main").unwrap(), want, "case {n}");
        }
    }
}

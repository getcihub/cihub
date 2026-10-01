use cihub_domain::{ModuleName, OrgId, QueueId, RepoId, ScmInstanceId};
use cihub_scm::{ConnectionRef, RepoRef};
use sqlx::PgConnection;

use crate::MergeQueueError;

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
    /// The organization has the module (`AGENTS.md` D94).
    pub enabled: bool,
}

type Row = (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    bool,
);

/// The columns of [`Row`]: the queue, its repository, its connection, and
/// whether its organization has the module (`$1`).
const COLUMNS: &str = "q.id, q.org_id, q.target_branch, r.id, r.scm_instance_id, r.external_id,
    r.namespace, r.name, c.id, c.external_id,
    EXISTS (SELECT 1 FROM org_modules m WHERE m.org_id = q.org_id AND m.module = $1::module_name)
    FROM queues q
    JOIN repos r ON r.id = q.repo_id
    JOIN scm_connections c ON c.id = r.connection_id";

fn decode(row: Row) -> Result<QueueTarget, MergeQueueError> {
    let (queue, org, branch, repo, instance, external_id, namespace, name, connection, cext, on) =
        row;
    let read = |source| MergeQueueError::ReadQueue {
        id: queue.clone(),
        source,
    };
    Ok(QueueTarget {
        queue_id: queue.parse().map_err(read)?,
        org_id: org.parse().map_err(read)?,
        scm_instance_id: instance.parse().map_err(read)?,
        repo_id: repo.parse().map_err(read)?,
        repo: RepoRef {
            connection: ConnectionRef {
                id: connection.parse().map_err(read)?,
                external_id: cext,
            },
            external_id,
            namespace,
            name,
        },
        target_branch: branch,
        enabled: on,
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
            "SELECT {COLUMNS} WHERE q.id = $2"
        )))
        .bind(ModuleName::Mergequeue.as_str())
        .bind(queue.to_string())
        .fetch_optional(conn)
        .await
        .map_err(MergeQueueError::Query)?;
        decode(row.ok_or(MergeQueueError::UnknownQueue(queue))?)
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
            "SELECT {COLUMNS} WHERE r.id = $2 AND q.target_branch = $3"
        )))
        .bind(ModuleName::Mergequeue.as_str())
        .bind(repo.to_string())
        .bind(branch)
        .fetch_optional(conn)
        .await
        .map_err(MergeQueueError::Query)?;
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
            "SELECT r.org_id, r.scm_instance_id, r.external_id, r.namespace, r.name, c.id,
                    c.external_id
             FROM repos r JOIN scm_connections c ON c.id = r.connection_id
             WHERE r.id = $1",
        )
        .bind(repo.to_string())
        .fetch_optional(conn)
        .await
        .map_err(MergeQueueError::Query)?;
        let read = |source| MergeQueueError::ReadQueue {
            id: repo.to_string(),
            source,
        };
        let (org, instance, external_id, namespace, name, connection, cext) =
            row.ok_or(MergeQueueError::UnknownRepo(repo))?;
        Ok(Self {
            repo_id: repo,
            org_id: org.parse().map_err(read)?,
            scm_instance_id: instance.parse().map_err(read)?,
            repo: RepoRef {
                connection: ConnectionRef {
                    id: connection.parse().map_err(read)?,
                    external_id: cext,
                },
                external_id,
                namespace,
                name,
            },
        })
    }
}

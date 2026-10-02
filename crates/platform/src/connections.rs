use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use cihub_domain::{
    ApiScope, ApiTokenId, AuditEntryId, ConnectionId, ConnectionKind, JobClass, ModuleName, OrgId,
    RepoId, ScmAccountId, ScmInstanceId,
};
use cihub_scm::{ConnectionRef, Scm, ScmAccount, ScmError};
use cihub_store::{NewApiToken, PgPool, begin_system, create_api_token, new_api_token_secret};
use secrecy::SecretString;
use serde_json::json;
use sqlx::PgConnection;
use strum::IntoEnumIterator;
use time::OffsetDateTime;
use tracing::info;

use crate::{Job, Outcome, PlatformError, enqueue_job, error_chain};

/// The type of the job that reads the repositories of a connection
/// (`AGENTS.md` D155).
pub const SYNC_REPOS: &str = "sync_repos";

/// The name of the API token that [`bootstrap`] creates.
pub const BOOTSTRAP_TOKEN: &str = "bootstrap";

/// What [`bootstrap`] did.
pub struct Bootstrapped {
    pub org_id: OrgId,
    pub connection_id: ConnectionId,
    /// `false` when the connection was already known: only the token is new.
    pub created: bool,
    /// The secret of the new API token, shown once.
    pub token: SecretString,
}

/// Starts an organization from the connection `external_id` of the SCM
/// `instance` (`AGENTS.md` D154, D155): the organization, named by the
/// account of the connection, its account, the connection and the merge
/// queue module, then a request to read its repositories and a new API token
/// with every scope. On a connection already known, creates only the token,
/// so that an operator gets an organization back. The read has the key of the
/// hour: a second bootstrap in the same hour asks for no other read.
///
/// # Errors
///
/// The SCM does not know the connection, or Postgres refuses a row: another
/// organization already has the name of the account.
pub async fn bootstrap(
    pool: &PgPool,
    scm: &dyn Scm,
    instance: ScmInstanceId,
    kind: ConnectionKind,
    external_id: &str,
    now: OffsetDateTime,
) -> Result<Bootstrapped, PlatformError> {
    let account = scm
        .installation(external_id, JobClass::Interactive)
        .await
        .map_err(PlatformError::Scm)?;
    let mut tx = begin_system(pool).await.map_err(PlatformError::Store)?;
    let known: Option<(String, String)> = sqlx::query_as(
        "SELECT org_id, id FROM scm_connections
         WHERE scm_instance_id = $1 AND kind = $2::connection_kind AND external_id = $3",
    )
    .bind(instance.to_string())
    .bind(kind.as_str())
    .bind(external_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PlatformError::Query)?;
    let (org_id, connection_id, created) = if let Some((org, connection)) = known {
        let read = |id: String| {
            move |source| PlatformError::ReadConnection {
                id: id.clone(),
                source,
            }
        };
        let org_id = org.parse().map_err(read(org.clone()))?;
        let connection_id = connection.parse().map_err(read(connection.clone()))?;
        (org_id, connection_id, false)
    } else {
        let new = NewConnection {
            instance,
            kind,
            external_id,
            account: &account,
        };
        let (org_id, connection_id) = create_organization(&mut tx, &new, now).await?;
        (org_id, connection_id, true)
    };
    let job = sync_repos_job(org_id, instance, connection_id, &hour_window(now));
    enqueue_job(&mut tx, &job, now, now).await?;
    let token = new_api_token_secret().map_err(PlatformError::Store)?;
    #[expect(
        clippy::disallowed_methods,
        reason = "the bootstrap creates a new API token"
    )]
    let token_id = ApiTokenId::new();
    let scopes: Vec<ApiScope> = ApiScope::iter().collect();
    let new = NewApiToken {
        id: token_id,
        org_id,
        name: BOOTSTRAP_TOKEN,
        scopes: &scopes,
        created_by: None,
        expires_at: None,
    };
    create_api_token(&mut tx, &new, &token)
        .await
        .map_err(PlatformError::Store)?;
    #[expect(
        clippy::disallowed_methods,
        reason = "an entry of the audit log has a new identifier"
    )]
    let audit_id = AuditEntryId::new();
    sqlx::query(
        "INSERT INTO audit_log (id, org_id, action, target, after, occurred_at)
         VALUES ($1, $2, 'token.created', $3, $4, $5)",
    )
    .bind(audit_id.to_string())
    .bind(org_id.to_string())
    .bind(token_id.to_string())
    .bind(json!({ "name": BOOTSTRAP_TOKEN, "by": "bootstrap" }))
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(PlatformError::Query)?;
    tx.commit().await.map_err(PlatformError::Query)?;
    Ok(Bootstrapped {
        org_id,
        connection_id,
        created,
        token,
    })
}

/// A connection that no organization has yet, and its account.
struct NewConnection<'a> {
    instance: ScmInstanceId,
    kind: ConnectionKind,
    external_id: &'a str,
    account: &'a ScmAccount,
}

/// Creates the organization of `new`, named by its account, with the
/// account, the connection and the merge queue module.
async fn create_organization(
    conn: &mut PgConnection,
    new: &NewConnection<'_>,
    now: OffsetDateTime,
) -> Result<(OrgId, ConnectionId), PlatformError> {
    #[expect(
        clippy::disallowed_methods,
        reason = "the bootstrap creates the organization, its account and its connection"
    )]
    let (org_id, account_id, connection_id) =
        (OrgId::new(), ScmAccountId::new(), ConnectionId::new());
    sqlx::query("INSERT INTO orgs (id, slug, created_at) VALUES ($1, $2, $3)")
        .bind(org_id.to_string())
        .bind(&new.account.path)
        .bind(now)
        .execute(&mut *conn)
        .await
        .map_err(PlatformError::Query)?;
    sqlx::query(
        "INSERT INTO scm_accounts (id, org_id, scm_instance_id, external_id, path, created_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(account_id.to_string())
    .bind(org_id.to_string())
    .bind(new.instance.to_string())
    .bind(&new.account.external_id)
    .bind(&new.account.path)
    .bind(now)
    .execute(&mut *conn)
    .await
    .map_err(PlatformError::Query)?;
    sqlx::query(
        "INSERT INTO scm_connections (id, org_id, scm_instance_id, scm_account_id, kind,
                                      external_id, claimed_at, created_at)
         VALUES ($1, $2, $3, $4, $5::connection_kind, $6, $7, $7)",
    )
    .bind(connection_id.to_string())
    .bind(org_id.to_string())
    .bind(new.instance.to_string())
    .bind(account_id.to_string())
    .bind(new.kind.as_str())
    .bind(new.external_id)
    .bind(now)
    .execute(&mut *conn)
    .await
    .map_err(PlatformError::Query)?;
    sqlx::query(
        "INSERT INTO org_modules (org_id, module, enabled_at) VALUES ($1, $2::module_name, $3)",
    )
    .bind(org_id.to_string())
    .bind(ModuleName::Mergequeue.as_str())
    .bind(now)
    .execute(&mut *conn)
    .await
    .map_err(PlatformError::Query)?;
    Ok((org_id, connection_id))
}

/// The window of the hourly read and of the bootstrap: its hour, so that the
/// schedulers ask for one read per hour.
fn hour_window(now: OffsetDateTime) -> String {
    format!(
        "{:04}{:02}{:02}T{:02}",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour()
    )
}

/// The read of the repositories of `connection` for `cause`: the hour of the
/// hourly read, or the delivery that reported a change (`AGENTS.md` D155).
pub fn sync_repos_job(
    org_id: OrgId,
    instance: ScmInstanceId,
    connection: ConnectionId,
    cause: &str,
) -> Job {
    Job {
        idempotency_key: format!("{SYNC_REPOS}:{connection}-{cause}:1"),
        job_type: SYNC_REPOS.to_owned(),
        class: JobClass::Repair,
        org_id: Some(org_id),
        connection: Some((instance, connection)),
        queue_id: None,
        payload: json!({}),
        attempts: 0,
    }
}

/// Requests a read of the repositories of `connection` now, after the
/// delivery `delivery_id` reported a change: each delivery gets its own read.
///
/// # Errors
///
/// Postgres fails.
pub async fn request_repo_sync(
    conn: &mut PgConnection,
    org_id: OrgId,
    instance: ScmInstanceId,
    connection: ConnectionId,
    delivery_id: &str,
    now: OffsetDateTime,
) -> Result<bool, PlatformError> {
    let job = sync_repos_job(org_id, instance, connection, delivery_id);
    enqueue_job(conn, &job, now, now).await
}

/// Requests the hourly read of the repositories of each connection that is
/// not suspended, and returns the number of new requests. Each scheduler
/// does it; the key of the hour admits one job per connection.
///
/// # Errors
///
/// Postgres fails, or a row does not match the identifiers of `cihub-domain`.
pub async fn request_repo_syncs(
    conn: &mut PgConnection,
    now: OffsetDateTime,
) -> Result<usize, PlatformError> {
    let connections: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT org_id, scm_instance_id, id FROM scm_connections
         WHERE suspended_at IS NULL ORDER BY id",
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(PlatformError::Query)?;
    let window = hour_window(now);
    let mut requested = 0;
    for (org, instance, connection) in connections {
        let read = |source| PlatformError::ReadConnection {
            id: connection.clone(),
            source,
        };
        let job = sync_repos_job(
            org.parse().map_err(read)?,
            instance.parse().map_err(read)?,
            connection.parse().map_err(read)?,
            &window,
        );
        if enqueue_job(conn, &job, now, now).await? {
            requested += 1;
        }
    }
    Ok(requested)
}

/// Reads the repositories of the connection of `job`, creates those that
/// CIHub does not know and updates the others, and returns their number. A
/// repository that the connection no longer reaches stays (`AGENTS.md`
/// D155); one that another organization has is left to it.
///
/// # Errors
///
/// The job has no connection, or Postgres or the SCM fails.
pub async fn run_sync_repos(
    pool: &PgPool,
    scm: &dyn Scm,
    job: &Job,
) -> Result<usize, PlatformError> {
    let (Some(org_id), Some((instance, connection_id))) = (job.org_id, job.connection) else {
        return Err(PlatformError::JobWithoutConnection(
            job.idempotency_key.clone(),
        ));
    };
    let mut tx = begin_system(pool).await.map_err(PlatformError::Store)?;
    let external_id: Option<String> = sqlx::query_scalar(
        "SELECT external_id FROM scm_connections WHERE id = $1 AND suspended_at IS NULL",
    )
    .bind(connection_id.to_string())
    .fetch_optional(&mut *tx)
    .await
    .map_err(PlatformError::Query)?;
    tx.commit().await.map_err(PlatformError::Query)?;
    let Some(external_id) = external_id else {
        return Ok(0);
    };
    let connection = ConnectionRef {
        id: connection_id,
        external_id,
    };
    let repos = scm
        .list_repos(&connection, job.class)
        .await
        .map_err(PlatformError::Scm)?;
    #[expect(
        clippy::disallowed_methods,
        reason = "a repository new to CIHub gets an identifier; the others keep theirs"
    )]
    let ids: Vec<String> = repos.iter().map(|_| RepoId::new().to_string()).collect();
    let column = |field: fn(&cihub_scm::ScmRepo) -> &String| -> Vec<&String> {
        repos.iter().map(field).collect()
    };
    let mut tx = begin_system(pool).await.map_err(PlatformError::Store)?;
    let written = sqlx::query(
        "INSERT INTO repos (id, org_id, scm_instance_id, connection_id, external_id, namespace,
                            name, default_branch)
         SELECT r.id, $2, $3, $4, r.external_id, r.namespace, r.name, r.default_branch
         FROM unnest($1::text[], $5::text[], $6::text[], $7::text[], $8::text[])
              AS r (id, external_id, namespace, name, default_branch)
         ON CONFLICT (scm_instance_id, external_id) DO UPDATE
         SET connection_id = EXCLUDED.connection_id, namespace = EXCLUDED.namespace,
             name = EXCLUDED.name, default_branch = EXCLUDED.default_branch
         WHERE repos.org_id = EXCLUDED.org_id",
    )
    .bind(&ids)
    .bind(org_id.to_string())
    .bind(instance.to_string())
    .bind(connection_id.to_string())
    .bind(column(|repo| &repo.external_id))
    .bind(column(|repo| &repo.namespace))
    .bind(column(|repo| &repo.name))
    .bind(column(|repo| &repo.default_branch))
    .execute(&mut *tx)
    .await
    .map_err(PlatformError::Query)?;
    tx.commit().await.map_err(PlatformError::Query)?;
    let written = usize::try_from(written.rows_affected()).unwrap_or(usize::MAX);
    info!(
        org_id = %org_id,
        connection_id = %connection_id,
        repos = written,
        "repositories read"
    );
    Ok(written)
}

/// The executor of [`SYNC_REPOS`], with its SCM (`AGENTS.md` D71). A
/// connection that the SCM no longer serves fails the job: the next hour
/// tries again.
pub fn sync_repos_executor(
    scm: Arc<dyn Scm>,
) -> impl Fn(PgPool, Job, OffsetDateTime) -> Pin<Box<dyn Future<Output = Outcome> + Send>>
+ Send
+ Sync
+ 'static {
    move |pool, job, _now| {
        let scm = Arc::clone(&scm);
        Box::pin(async move {
            match run_sync_repos(&pool, scm.as_ref(), &job).await {
                Ok(_) => Outcome::Done,
                Err(err @ PlatformError::Scm(ScmError::AccessLost(_) | ScmError::NotFound(_))) => {
                    Outcome::Failed(error_chain(&err))
                }
                Err(err) => Outcome::Retry(error_chain(&err)),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use ulid::Ulid;

    use super::*;

    #[test]
    fn a_read_of_repositories_has_one_job_per_hour_or_per_delivery() {
        let at = |hour, minute| {
            OffsetDateTime::from_unix_timestamp(1_789_000_000)
                .unwrap()
                .replace_hour(hour)
                .unwrap()
                .replace_minute(minute)
                .unwrap()
        };
        let (org, instance, connection) = (
            OrgId::from_ulid(Ulid(1)),
            ScmInstanceId::from_ulid(Ulid(2)),
            ConnectionId::from_ulid(Ulid(3)),
        );
        let key = |cause: &str| sync_repos_job(org, instance, connection, cause).idempotency_key;
        assert_eq!(key(&hour_window(at(9, 1))), key(&hour_window(at(9, 59))));
        assert_ne!(key(&hour_window(at(9, 59))), key(&hour_window(at(10, 0))));
        assert_eq!(key("d-1"), format!("sync_repos:{connection}-d-1:1"));
        assert_eq!(
            key(&hour_window(at(9, 7))),
            format!("sync_repos:{connection}-20260910T09:1")
        );
    }
}

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use cihub_domain::{
    ConnectionId, ConnectionKind, JobClass, MembershipRole, OrgId, Permission, RepoId, UserId,
};
use cihub_scm::{ConnectionRef, RepoRef, Scm, ScmAccount, ScmError, ScmPersonToken, ScmUser};
use cihub_store::{
    DatabaseCipher, PgPool, Postgres, StoreError, Transaction, begin_org, begin_system,
};
use secrecy::SecretString;
use serde_json::json;
use sqlx::PgConnection;
use time::{Duration, OffsetDateTime};
use tracing::info;

use crate::connections::outcome_of;
use crate::installations::connect_installations;
use crate::jobs::hour_window;
use crate::members::{Membership, record_membership};
use crate::{Job, Outcome, PlatformError, enqueue_job};

pub const SYNC_USER: &str = "sync_user";

/// How long an access that the SCM gave serves before a request checks it
/// again: a person who loses a repository on the SCM loses it in CIHub
/// within this time.
pub const ACCESS_LIFETIME: Duration = Duration::hours(1);

/// A token that ends this soon is refreshed before a call.
const TOKEN_MARGIN: Duration = Duration::minutes(5);

/// What a check of the access of a person decides.
#[derive(Debug, PartialEq, Eq)]
pub enum Access<T> {
    Granted(T),
    /// CIHub has no token of the person, or the SCM refuses it: the person
    /// signs in again.
    SignInAgain,
}

pub fn sync_user_job(user_id: UserId, cause: &str) -> Job {
    Job {
        idempotency_key: format!("{SYNC_USER}:{user_id}-{cause}:1"),
        job_type: SYNC_USER.to_owned(),
        class: JobClass::Interactive,
        org_id: None,
        connection: None,
        queue_id: None,
        payload: json!({ "user": user_id.to_string() }),
        attempts: 0,
    }
}

/// Asks for the sync of `user_id`, once for `cause`, and returns whether the
/// request is new.
///
/// # Errors
///
/// Postgres fails.
pub async fn request_user_sync(
    conn: &mut PgConnection,
    user_id: UserId,
    cause: &str,
    now: OffsetDateTime,
) -> Result<bool, PlatformError> {
    enqueue_job(conn, &sync_user_job(user_id, cause), now, now).await
}

/// Asks for the sync of `user_id` when their last one is older than
/// [`ACCESS_LIFETIME`], at most once an hour, and returns whether the
/// request is new.
///
/// # Errors
///
/// Postgres fails.
pub async fn request_user_sync_when_old(
    pool: &PgPool,
    user_id: UserId,
    now: OffsetDateTime,
) -> Result<bool, PlatformError> {
    let mut tx = begin_system(pool).await.map_err(PlatformError::Store)?;
    let synced_at = person_synced_at(&mut tx, user_id)
        .await
        .map_err(PlatformError::Store)?;
    if synced_at.is_some_and(|synced_at| now - synced_at < ACCESS_LIFETIME) {
        return Ok(false);
    }

    let requested = request_user_sync(&mut tx, user_id, &hour_window(now), now).await?;
    tx.commit().await.map_err(PlatformError::Query)?;
    Ok(requested)
}

/// Keeps the token that a sign-in gave, sealed.
///
/// # Errors
///
/// The token cannot be sealed, or Postgres fails.
pub(crate) async fn keep_token(
    conn: &mut PgConnection,
    cipher: &DatabaseCipher,
    user_id: UserId,
    token: &ScmPersonToken,
    now: OffsetDateTime,
) -> Result<(), PlatformError> {
    keep_person_token(conn, cipher, user_id, &person_token(token, now))
        .await
        .map_err(PlatformError::Store)
}

fn person_token(token: &ScmPersonToken, now: OffsetDateTime) -> PersonToken {
    PersonToken {
        access: token.access.clone(),
        access_expires_at: token.access_lifetime.map(|lifetime| now + lifetime),
        refresh: token.refresh.clone(),
        refresh_expires_at: token.refresh_lifetime.map(|lifetime| now + lifetime),
    }
}

/// A valid token of `user_id`, refreshed when it ends within a few minutes.
/// `None` without a token, or when the SCM refuses the refresh: the token
/// and what it read are then forgotten. The row of the token stays locked
/// during the refresh, since a refresh token works once.
///
/// # Errors
///
/// Postgres fails, or the SCM does not answer the refresh.
pub async fn token_of_person(
    pool: &PgPool,
    scm: &dyn Scm,
    cipher: &DatabaseCipher,
    user_id: UserId,
    now: OffsetDateTime,
) -> Result<Option<SecretString>, PlatformError> {
    let mut tx = begin_system(pool).await.map_err(PlatformError::Store)?;
    let Some(token) = lock_person_token(&mut tx, cipher, user_id)
        .await
        .map_err(PlatformError::Store)?
    else {
        return Ok(None);
    };
    let is_valid = token
        .access_expires_at
        .is_none_or(|expires_at| expires_at - now > TOKEN_MARGIN);
    if is_valid {
        return Ok(Some(token.access));
    }

    let refresh = token
        .refresh
        .filter(|_| token.refresh_expires_at.is_none_or(|end| end > now));
    let refreshed = match refresh {
        Some(refresh) => Some(
            scm.refresh_person_token(&refresh, JobClass::Interactive)
                .await,
        ),
        None => None,
    };

    match refreshed {
        Some(Ok(refreshed)) => {
            keep_token(&mut tx, cipher, user_id, &refreshed, now).await?;
            tx.commit().await.map_err(PlatformError::Query)?;
            Ok(Some(refreshed.access))
        }
        None | Some(Err(ScmError::AccessLost(_))) => {
            forget_token_in(tx, user_id).await?;
            Ok(None)
        }
        Some(Err(err)) => Err(PlatformError::Scm(err)),
    }
}

/// Forgets the token of `user_id` and what it read, after the SCM refused it.
async fn forget_token(pool: &PgPool, user_id: UserId) -> Result<(), PlatformError> {
    let tx = begin_system(pool).await.map_err(PlatformError::Store)?;
    forget_token_in(tx, user_id).await
}

async fn forget_token_in(
    mut tx: Transaction<'static, Postgres>,
    user_id: UserId,
) -> Result<(), PlatformError> {
    forget_person_token(&mut tx, user_id)
        .await
        .map_err(PlatformError::Store)?;
    tx.commit().await.map_err(PlatformError::Query)?;
    info!(user_id = %user_id, "token of the person refused or ended");
    Ok(())
}

/// Reads with the token of the person of `job` the installations that they
/// reach, connects those that CIHub does not know, and replaces their
/// permissions on the repositories that CIHub knows. Without a token, or
/// when the SCM refuses it, the person keeps nothing: they sign in again.
///
/// # Errors
///
/// The job names no user, or Postgres or the SCM fails.
pub async fn run_sync_user(
    pool: &PgPool,
    scm: &dyn Scm,
    cipher: &DatabaseCipher,
    kind: ConnectionKind,
    job: &Job,
    now: OffsetDateTime,
) -> Result<usize, PlatformError> {
    let user_id = user_of_job(job)?;
    let Some(token) = token_of_person(pool, scm, cipher, user_id, now).await? else {
        return Ok(0);
    };

    let reach = match scm.person_reach(&token, job.class).await {
        Err(ScmError::AccessLost(_)) => {
            forget_token(pool, user_id).await?;
            return Ok(0);
        }
        reach => reach.map_err(PlatformError::Scm)?,
    };
    connect_installations(pool, kind, &reach.installations, now).await?;
    let column = |field: fn(&cihub_scm::ScmPersonRepo) -> String| -> Vec<String> {
        reach.repos.iter().map(field).collect()
    };
    let mut tx = begin_system(pool).await.map_err(PlatformError::Store)?;
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT r.org_id, r.id, p.permission
         FROM unnest($2::text[], $3::text[], $4::text[])
              AS p (connection_external_id, repo_external_id, permission)
         JOIN scm_connections c
           ON c.kind = $1::connection_kind AND c.external_id = p.connection_external_id
              AND c.suspended_at IS NULL
         JOIN repos r
           ON r.connection_id = c.id AND r.external_id = p.repo_external_id
              AND r.removed_at IS NULL",
    )
    .bind(kind.as_str())
    .bind(column(|repo| repo.connection_external_id.clone()))
    .bind(column(|repo| repo.repo_external_id.clone()))
    .bind(column(|repo| repo.permission.as_str().to_owned()))
    .fetch_all(&mut *tx)
    .await
    .map_err(PlatformError::Query)?;
    let permissions = rows
        .into_iter()
        .map(|(org, repo, permission)| {
            let read = || PlatformError::read("repository", &repo);
            Ok((
                org.parse::<OrgId>().map_err(read())?,
                repo.parse::<RepoId>().map_err(read())?,
                permission
                    .parse::<Permission>()
                    .map_err(PlatformError::read("permission", &permission))?,
            ))
        })
        .collect::<Result<Vec<_>, PlatformError>>()?;

    record_person_sync(&mut tx, user_id, &permissions, now)
        .await
        .map_err(PlatformError::Store)?;
    tx.commit().await.map_err(PlatformError::Query)?;
    info!(
        user_id = %user_id,
        repos = permissions.len(),
        "repositories of the person read"
    );

    Ok(permissions.len())
}

fn user_of_job(job: &Job) -> Result<UserId, PlatformError> {
    let user = job.payload["user"]
        .as_str()
        .ok_or_else(|| PlatformError::JobWithoutUser(job.idempotency_key.clone()))?;
    user.parse().map_err(PlatformError::read("user", user))
}

pub fn sync_user_executor(
    scm: Arc<dyn Scm>,
    cipher: Arc<DatabaseCipher>,
    kind: ConnectionKind,
) -> impl Fn(PgPool, Job, OffsetDateTime) -> Pin<Box<dyn Future<Output = Outcome> + Send>>
+ Send
+ Sync
+ 'static {
    move |pool, job, now| {
        let (scm, cipher) = (Arc::clone(&scm), Arc::clone(&cipher));
        Box::pin(async move {
            outcome_of(run_sync_user(&pool, scm.as_ref(), &cipher, kind, &job, now).await)
        })
    }
}

/// Asks for the sync of each person with a role in `org_id`, so that the
/// repositories that a sync of the connection found reach them.
///
/// # Errors
///
/// Postgres fails.
pub(crate) async fn request_member_user_syncs(
    conn: &mut PgConnection,
    org_id: OrgId,
    connection_id: ConnectionId,
    now: OffsetDateTime,
) -> Result<(), PlatformError> {
    let users: Vec<String> = sqlx::query_scalar(
        "SELECT m.user_id FROM memberships m JOIN users u ON u.id = m.user_id
         WHERE m.org_id = $1 AND u.scm_token IS NOT NULL ORDER BY m.user_id",
    )
    .bind(org_id.to_string())
    .fetch_all(&mut *conn)
    .await
    .map_err(PlatformError::Query)?;

    let cause = format!("repos-{connection_id}-{}", hour_window(now));
    for user in users {
        let user_id: UserId = user.parse().map_err(PlatformError::read("user", &user))?;
        request_user_sync(conn, user_id, &cause, now).await?;
    }
    Ok(())
}

/// The role of `user_id` in the organization of `membership`, checked again
/// with their token on the SCM when it is older than [`ACCESS_LIFETIME`];
/// `None` when they no longer have one, whose membership is then deleted.
/// The role is read by the login that the installation has now, not by the
/// slug, which an account renamed on the SCM leaves behind. An SCM that
/// refuses the read is an error, never a `None`: the membership stays, and
/// the hourly read of the members decides.
///
/// # Errors
///
/// Postgres fails, or the SCM does not answer or refuses the read.
pub async fn role_now(
    pool: &PgPool,
    scm: &dyn Scm,
    cipher: &DatabaseCipher,
    user_id: UserId,
    membership: &Membership,
    now: OffsetDateTime,
) -> Result<Access<Option<MembershipRole>>, PlatformError> {
    if now - membership.refreshed_at < ACCESS_LIFETIME {
        return Ok(Access::Granted(Some(membership.role)));
    }

    let Some(token) = token_of_person(pool, scm, cipher, user_id, now).await? else {
        return Ok(Access::SignInAgain);
    };
    let mut tx = begin_org(pool, membership.org_id)
        .await
        .map_err(PlatformError::Store)?;
    let (external_id, login): (String, String) =
        sqlx::query_as("SELECT external_id, login FROM users WHERE id = $1")
            .bind(user_id.to_string())
            .fetch_one(&mut *tx)
            .await
            .map_err(PlatformError::Query)?;
    let (account, installation): (String, String) = sqlx::query_as(
        "SELECT account_external_id, external_id FROM scm_connections ORDER BY created_at, id LIMIT 1",
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(PlatformError::Query)?;
    drop(tx);

    let path = match scm
        .installation(&installation, JobClass::Interactive)
        .await
        .map_err(PlatformError::Scm)?
    {
        Some(installation) => installation.account.path,
        None => membership.org_slug.clone(),
    };
    let person = ScmUser { external_id, login };
    let account = ScmAccount {
        external_id: account,
        path,
    };
    let role = match scm
        .person_role(&token, &person, &account, JobClass::Interactive)
        .await
    {
        Err(ScmError::AccessLost(_)) => {
            forget_token(pool, user_id).await?;
            return Ok(Access::SignInAgain);
        }
        role => role.map_err(PlatformError::Scm)?,
    };
    let mut tx = begin_org(pool, membership.org_id)
        .await
        .map_err(PlatformError::Store)?;
    match role {
        Some(role) => record_membership(&mut tx, membership.org_id, user_id, role, now)
            .await
            .map_err(PlatformError::Store)?,
        None => {
            sqlx::query("DELETE FROM memberships WHERE user_id = $1")
                .bind(user_id.to_string())
                .execute(&mut *tx)
                .await
                .map_err(PlatformError::Query)?;
        }
    }
    tx.commit().await.map_err(PlatformError::Query)?;

    Ok(Access::Granted(role))
}

/// The repository `name` of `org_id` and the permission of `user_id` on it,
/// checked again with their token on the SCM when it is older than
/// [`ACCESS_LIFETIME`]. `None` when the organization has no such
/// repository.
///
/// # Errors
///
/// Postgres fails, or the SCM does not answer.
pub async fn repo_permission_now(
    pool: &PgPool,
    scm: &dyn Scm,
    cipher: &DatabaseCipher,
    user_id: UserId,
    org_id: OrgId,
    name: &str,
    now: OffsetDateTime,
) -> Result<Access<Option<(RepoId, Permission)>>, PlatformError> {
    let mut tx = begin_org(pool, org_id)
        .await
        .map_err(PlatformError::Store)?;
    // The SCM does not tell names apart by their case.
    let row: Option<(String, String, String, String, String, String)> = sqlx::query_as(
        "SELECT r.id, r.external_id, r.owner, r.name, c.id, c.external_id
         FROM repos r JOIN scm_connections c ON c.id = r.connection_id
         WHERE lower(r.name) = lower($1) ORDER BY r.id LIMIT 1",
    )
    .bind(name)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PlatformError::Query)?;
    let Some((repo_id, external_id, owner, repo_name, connection_id, connection)) = row else {
        return Ok(Access::Granted(None));
    };
    let read = || PlatformError::read("repository", &repo_id);
    let repo_id: RepoId = repo_id.parse().map_err(read())?;
    let known = repo_permission_of(&mut tx, repo_id, user_id)
        .await
        .map_err(PlatformError::Store)?;
    if let Some((permission, checked_at)) = known
        && now - checked_at < ACCESS_LIFETIME
    {
        return Ok(Access::Granted(Some((repo_id, permission))));
    }
    drop(tx);

    let Some(token) = token_of_person(pool, scm, cipher, user_id, now).await? else {
        return Ok(Access::SignInAgain);
    };
    let repo = RepoRef {
        connection: ConnectionRef {
            id: connection_id.parse().map_err(read())?,
            external_id: connection,
        },
        external_id,
        owner,
        name: repo_name,
    };
    let permission = match scm
        .person_permission(&token, &repo, JobClass::Interactive)
        .await
    {
        Err(ScmError::AccessLost(_)) => {
            forget_token(pool, user_id).await?;
            return Ok(Access::SignInAgain);
        }
        permission => permission.map_err(PlatformError::Scm)?,
    };
    let mut tx = begin_org(pool, org_id)
        .await
        .map_err(PlatformError::Store)?;
    record_repo_permission(&mut tx, org_id, repo_id, user_id, permission, now)
        .await
        .map_err(PlatformError::Store)?;
    tx.commit().await.map_err(PlatformError::Query)?;

    Ok(Access::Granted(Some((repo_id, permission))))
}

/// The token of a person on the SCM, and when it and its refresh end.
pub struct PersonToken {
    pub access: SecretString,
    /// `None` for a token that does not expire.
    pub access_expires_at: Option<OffsetDateTime>,
    pub refresh: Option<SecretString>,
    pub refresh_expires_at: Option<OffsetDateTime>,
}

fn access_place(user_id: UserId) -> String {
    format!("users.scm_token:{user_id}")
}

fn refresh_place(user_id: UserId) -> String {
    format!("users.scm_refresh_token:{user_id}")
}

/// Keeps `token` for `user_id`, sealed, in place of the last one.
///
/// # Errors
///
/// The token cannot be sealed, or Postgres fails.
pub async fn keep_person_token(
    conn: &mut PgConnection,
    cipher: &DatabaseCipher,
    user_id: UserId,
    token: &PersonToken,
) -> Result<(), StoreError> {
    let access = cipher.seal(&token.access, &access_place(user_id))?;
    let refresh = token
        .refresh
        .as_ref()
        .map(|refresh| cipher.seal(refresh, &refresh_place(user_id)))
        .transpose()?;

    sqlx::query(
        "UPDATE users SET scm_token = $2, scm_token_expires_at = $3, scm_refresh_token = $4,
                          scm_refresh_token_expires_at = $5
         WHERE id = $1",
    )
    .bind(user_id.to_string())
    .bind(access)
    .bind(token.access_expires_at)
    .bind(refresh)
    .bind(token.refresh_expires_at)
    .execute(conn)
    .await
    .map_err(StoreError::Query)?;
    Ok(())
}

type TokenRow = (
    Vec<u8>,
    Option<OffsetDateTime>,
    Option<Vec<u8>>,
    Option<OffsetDateTime>,
);

/// The token of `user_id`, its row locked until the end of the transaction:
/// a refresh token works once, so two refreshes must not run together.
/// `None` without a token.
///
/// # Errors
///
/// Postgres fails, or the token does not open with `cipher`.
pub async fn lock_person_token(
    conn: &mut PgConnection,
    cipher: &DatabaseCipher,
    user_id: UserId,
) -> Result<Option<PersonToken>, StoreError> {
    let row: Option<TokenRow> = sqlx::query_as(
        "SELECT scm_token, scm_token_expires_at, scm_refresh_token, scm_refresh_token_expires_at
         FROM users WHERE id = $1 AND scm_token IS NOT NULL FOR UPDATE",
    )
    .bind(user_id.to_string())
    .fetch_optional(conn)
    .await
    .map_err(StoreError::Query)?;
    let Some((access, access_expires_at, refresh, refresh_expires_at)) = row else {
        return Ok(None);
    };

    Ok(Some(PersonToken {
        access: cipher.open(&access, &access_place(user_id))?,
        access_expires_at,
        refresh: refresh
            .map(|refresh| cipher.open(&refresh, &refresh_place(user_id)))
            .transpose()?,
        refresh_expires_at,
    }))
}

/// Forgets the token of `user_id` and what it read: their permissions and
/// the time of their last sync.
///
/// # Errors
///
/// Postgres fails.
pub async fn forget_person_token(
    conn: &mut PgConnection,
    user_id: UserId,
) -> Result<(), StoreError> {
    sqlx::query(
        "WITH forgotten AS (
             UPDATE users SET scm_token = NULL, scm_token_expires_at = NULL,
                              scm_refresh_token = NULL, scm_refresh_token_expires_at = NULL,
                              synced_at = NULL
             WHERE id = $1 RETURNING id
         )
         DELETE FROM repo_permissions WHERE user_id IN (SELECT id FROM forgotten)",
    )
    .bind(user_id.to_string())
    .execute(conn)
    .await
    .map_err(StoreError::Query)?;
    Ok(())
}

/// Forgets the token of each person without a session and returns their
/// number. What the token read stays: the next sign-in shows it at once,
/// while its sync reads it again. Runs in the system context.
///
/// # Errors
///
/// Postgres fails.
pub async fn forget_tokens_without_session(conn: &mut PgConnection) -> Result<u64, StoreError> {
    let forgotten = sqlx::query(
        "UPDATE users SET scm_token = NULL, scm_token_expires_at = NULL,
                          scm_refresh_token = NULL, scm_refresh_token_expires_at = NULL
         WHERE scm_token IS NOT NULL
           AND NOT EXISTS (SELECT 1 FROM sessions s WHERE s.user_id = users.id)",
    )
    .execute(conn)
    .await
    .map_err(StoreError::Query)?;
    Ok(forgotten.rows_affected())
}

/// Forgets the permissions that the SCM gave before `before`, and the sync
/// of each person whose last one is older, and returns the number of
/// permissions. Runs in the system context.
///
/// # Errors
///
/// Postgres fails.
pub async fn forget_permissions_before(
    conn: &mut PgConnection,
    before: OffsetDateTime,
) -> Result<u64, StoreError> {
    let forgotten = sqlx::query("DELETE FROM repo_permissions WHERE checked_at < $1")
        .bind(before)
        .execute(&mut *conn)
        .await
        .map_err(StoreError::Query)?;
    sqlx::query("UPDATE users SET synced_at = NULL WHERE synced_at < $1")
        .bind(before)
        .execute(conn)
        .await
        .map_err(StoreError::Query)?;
    Ok(forgotten.rows_affected())
}

/// When CIHub last read the installations and the repositories of
/// `user_id`; `None` before the first read.
///
/// # Errors
///
/// Postgres fails.
pub async fn person_synced_at(
    conn: &mut PgConnection,
    user_id: UserId,
) -> Result<Option<OffsetDateTime>, StoreError> {
    let synced_at: Option<Option<OffsetDateTime>> =
        sqlx::query_scalar("SELECT synced_at FROM users WHERE id = $1")
            .bind(user_id.to_string())
            .fetch_optional(conn)
            .await
            .map_err(StoreError::Query)?;
    Ok(synced_at.flatten())
}

/// Replaces the permissions of `user_id` on every repository by those of a
/// read of the SCM at `now`, and records the read. Runs in the system
/// context: a person reaches several organizations.
///
/// # Errors
///
/// Postgres fails.
pub async fn record_person_sync(
    conn: &mut PgConnection,
    user_id: UserId,
    permissions: &[(OrgId, RepoId, Permission)],
    now: OffsetDateTime,
) -> Result<(), StoreError> {
    let column = |field: fn(&(OrgId, RepoId, Permission)) -> String| -> Vec<String> {
        permissions.iter().map(field).collect()
    };
    let user = user_id.to_string();

    sqlx::query("DELETE FROM repo_permissions WHERE user_id = $1")
        .bind(&user)
        .execute(&mut *conn)
        .await
        .map_err(StoreError::Query)?;
    // A request records a permission in its own transaction: one committed
    // between the delete and this insert must not fail the sync.
    sqlx::query(
        "INSERT INTO repo_permissions (org_id, repo_id, user_id, permission, checked_at)
         SELECT p.org_id, p.repo_id, $4, p.permission::permission, $5
         FROM unnest($1::text[], $2::text[], $3::text[]) AS p (org_id, repo_id, permission)
         ON CONFLICT (repo_id, user_id) DO UPDATE
         SET permission = EXCLUDED.permission, checked_at = EXCLUDED.checked_at",
    )
    .bind(column(|(org, _, _)| org.to_string()))
    .bind(column(|(_, repo, _)| repo.to_string()))
    .bind(column(|(_, _, permission)| permission.as_str().to_owned()))
    .bind(&user)
    .bind(now)
    .execute(&mut *conn)
    .await
    .map_err(StoreError::Query)?;
    sqlx::query("UPDATE users SET synced_at = $2 WHERE id = $1")
        .bind(&user)
        .bind(now)
        .execute(conn)
        .await
        .map_err(StoreError::Query)?;
    Ok(())
}

/// Records the permission of `user_id` on `repo_id` of the organization of
/// the transaction, as the SCM gave it at `now`. A `None` is recorded too:
/// a refusal serves as long as a grant before the SCM is asked again.
///
/// # Errors
///
/// Postgres fails.
pub async fn record_repo_permission(
    conn: &mut PgConnection,
    org_id: OrgId,
    repo_id: RepoId,
    user_id: UserId,
    permission: Permission,
    now: OffsetDateTime,
) -> Result<(), StoreError> {
    sqlx::query(
        "INSERT INTO repo_permissions (org_id, repo_id, user_id, permission, checked_at)
         VALUES ($1, $2, $3, $4::permission, $5)
         ON CONFLICT (repo_id, user_id) DO UPDATE
         SET permission = EXCLUDED.permission, checked_at = EXCLUDED.checked_at",
    )
    .bind(org_id.to_string())
    .bind(repo_id.to_string())
    .bind(user_id.to_string())
    .bind(permission.as_str())
    .bind(now)
    .execute(conn)
    .await
    .map_err(StoreError::Query)?;
    Ok(())
}

/// The permission of `user_id` on `repo_id` and when the SCM gave it;
/// `None` when CIHub knows none.
///
/// # Errors
///
/// Postgres fails, or the row does not match `cihub-domain`.
pub async fn repo_permission_of(
    conn: &mut PgConnection,
    repo_id: RepoId,
    user_id: UserId,
) -> Result<Option<(Permission, OffsetDateTime)>, StoreError> {
    let row: Option<(String, OffsetDateTime)> = sqlx::query_as(
        "SELECT permission::text, checked_at FROM repo_permissions
         WHERE repo_id = $1 AND user_id = $2",
    )
    .bind(repo_id.to_string())
    .bind(user_id.to_string())
    .fetch_optional(conn)
    .await
    .map_err(StoreError::Query)?;

    row.map(|(permission, checked_at)| {
        let permission = permission
            .parse()
            .map_err(StoreError::read("repo permission", &permission))?;
        Ok((permission, checked_at))
    })
    .transpose()
}

/// The repositories of the organization of the transaction that `user_id`
/// reads.
///
/// # Errors
///
/// Postgres fails, or a row does not match `cihub-domain`.
pub async fn readable_repos(
    conn: &mut PgConnection,
    user_id: UserId,
) -> Result<Vec<RepoId>, StoreError> {
    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT repo_id FROM repo_permissions
         WHERE user_id = $1 AND permission <> 'none' ORDER BY repo_id",
    )
    .bind(user_id.to_string())
    .fetch_all(conn)
    .await
    .map_err(StoreError::Query)?;

    ids.iter()
        .map(|id| id.parse().map_err(StoreError::read("repository", id)))
        .collect()
}

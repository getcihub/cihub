use cihub_domain::{OrgId, Permission, RepoId, UserId};
use secrecy::SecretString;
use sqlx::PgConnection;
use time::OffsetDateTime;

use crate::{DatabaseCipher, StoreError};

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

/// Forgets the token of each person without a session, and what it read,
/// and returns their number. Runs in the system context.
///
/// # Errors
///
/// Postgres fails.
pub async fn forget_tokens_without_session(conn: &mut PgConnection) -> Result<u64, StoreError> {
    let forgotten: Vec<String> = sqlx::query_scalar(
        "UPDATE users SET scm_token = NULL, scm_token_expires_at = NULL,
                          scm_refresh_token = NULL, scm_refresh_token_expires_at = NULL,
                          synced_at = NULL
         WHERE scm_token IS NOT NULL
           AND NOT EXISTS (SELECT 1 FROM sessions s WHERE s.user_id = users.id)
         RETURNING id",
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(StoreError::Query)?;

    sqlx::query("DELETE FROM repo_permissions WHERE user_id = ANY($1)")
        .bind(&forgotten)
        .execute(conn)
        .await
        .map_err(StoreError::Query)?;
    Ok(u64::try_from(forgotten.len()).unwrap_or(u64::MAX))
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

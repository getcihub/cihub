use cihub_domain::{MembershipRole, OrgId, UserId};
use sqlx::{PgConnection, PgPool};
use time::OffsetDateTime;

use crate::{StoreError, begin_system};

/// An organization that a user reaches, with the role of the user in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Membership {
    pub org_id: OrgId,
    pub org_slug: String,
    pub role: MembershipRole,
    /// Every connection of the organization is suspended.
    pub is_org_suspended: bool,
    /// When the SCM last gave the role.
    pub refreshed_at: OffsetDateTime,
}

/// Every connection of the organization `o` is suspended: CIHub reaches none
/// of its accounts.
pub const ORG_IS_SUSPENDED: &str = "EXISTS (SELECT 1 FROM scm_connections c WHERE c.org_id = o.id)
     AND NOT EXISTS (
         SELECT 1 FROM scm_connections c WHERE c.org_id = o.id AND c.suspended_at IS NULL
     )";

/// Records that `user_id` has `role` in the organization of the
/// transaction, as the SCM gave it at `now`.
///
/// # Errors
///
/// Postgres refuses the row: another organization than the context, or an
/// unknown user.
pub async fn record_membership(
    conn: &mut PgConnection,
    org_id: OrgId,
    user_id: UserId,
    role: MembershipRole,
    now: OffsetDateTime,
) -> Result<(), StoreError> {
    sqlx::query(
        "INSERT INTO memberships (org_id, user_id, role, refreshed_at)
         VALUES ($1, $2, $3::membership_role, $4)
         ON CONFLICT (org_id, user_id)
         DO UPDATE SET role = excluded.role, refreshed_at = excluded.refreshed_at",
    )
    .bind(org_id.to_string())
    .bind(user_id.to_string())
    .bind(role.as_str())
    .bind(now)
    .execute(conn)
    .await
    .map_err(StoreError::Query)?;
    Ok(())
}

/// Deletes the memberships of `org_id` that no refresh saw since `before`:
/// the user left the organization on the SCM. Gives the number deleted.
///
/// # Errors
///
/// Postgres fails.
pub async fn forget_memberships_before(
    conn: &mut PgConnection,
    org_id: OrgId,
    before: OffsetDateTime,
) -> Result<u64, StoreError> {
    let deleted = sqlx::query("DELETE FROM memberships WHERE org_id = $1 AND refreshed_at < $2")
        .bind(org_id.to_string())
        .bind(before)
        .execute(conn)
        .await
        .map_err(StoreError::Query)?
        .rows_affected();
    Ok(deleted)
}

/// The organizations of `user_id`, by slug. A user is not tied to one
/// organization, so the read runs in the system context.
///
/// # Errors
///
/// Postgres fails, or a row does not match the vocabulary of `cihub-domain`.
pub async fn memberships_of(pool: &PgPool, user_id: UserId) -> Result<Vec<Membership>, StoreError> {
    let mut tx = begin_system(pool).await?;
    let rows: Vec<(String, String, String, bool, OffsetDateTime)> =
        sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT o.id, o.slug, m.role::text, {ORG_IS_SUSPENDED}, m.refreshed_at
         FROM memberships m JOIN orgs o ON o.id = m.org_id
         WHERE m.user_id = $1 ORDER BY o.slug"
        )))
        .bind(user_id.to_string())
        .fetch_all(&mut *tx)
        .await
        .map_err(StoreError::Query)?;

    tx.commit().await.map_err(StoreError::Query)?;
    rows.into_iter()
        .map(|(org_id, org_slug, role, is_org_suspended, refreshed_at)| {
            Ok(Membership {
                org_id: org_id
                    .parse()
                    .map_err(StoreError::read("membership", &org_id))?,
                role: role
                    .parse()
                    .map_err(StoreError::read("membership", &org_id))?,
                org_slug,
                is_org_suspended,
                refreshed_at,
            })
        })
        .collect()
}

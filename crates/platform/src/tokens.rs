use cihub_domain::{ApiScope, ApiTokenId, AuditEntryId, OrgId};
use cihub_store::{NewApiToken, PgPool, begin_system, create_api_token, new_api_token_secret};
use secrecy::SecretString;
use serde_json::json;
use strum::IntoEnumIterator;
use time::OffsetDateTime;

use crate::PlatformError;

const TOKEN_NAME: &str = "token";

/// Creates an API token with every scope for the organization `slug`, writes
/// it in the audit log and returns it with its organization (`DECISIONS.md`
/// D248).
///
/// # Errors
///
/// No organization has the slug `slug`, or Postgres fails.
pub async fn create_token(
    pool: &PgPool,
    slug: &str,
    now: OffsetDateTime,
) -> Result<(OrgId, SecretString), PlatformError> {
    let mut tx = begin_system(pool).await.map_err(PlatformError::Store)?;
    let org: Option<String> = sqlx::query_scalar("SELECT id FROM orgs WHERE slug = $1")
        .bind(slug)
        .fetch_optional(&mut *tx)
        .await
        .map_err(PlatformError::Query)?;
    let Some(org) = org else {
        return Err(PlatformError::UnknownOrganization(slug.to_owned()));
    };

    let org_id: OrgId = org
        .parse()
        .map_err(PlatformError::read("organization", &org))?;

    let token = new_api_token_secret().map_err(PlatformError::Store)?;
    #[expect(
        clippy::disallowed_methods,
        reason = "an operator creates a new API token"
    )]
    let token_id = ApiTokenId::new();
    let scopes: Vec<ApiScope> = ApiScope::iter().collect();
    let new = NewApiToken {
        id: token_id,
        org_id,
        name: TOKEN_NAME,
        scopes: &scopes,
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
    .bind(json!({ "name": TOKEN_NAME, "by": "server token create" }))
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(PlatformError::Query)?;

    tx.commit().await.map_err(PlatformError::Query)?;
    Ok((org_id, token))
}

use cihub_domain::{ApiScope, ApiTokenId, AuditEntryId, OrgId, format_api_token, is_api_token};
use cihub_store::{PgPool, StoreError, begin_system, draw_secret_bytes, secret_hash};
use secrecy::{ExposeSecret, SecretString};
use serde_json::json;
use sqlx::PgConnection;
use strum::IntoEnumIterator;
use time::{Duration, OffsetDateTime};

use crate::PlatformError;

const TOKEN_NAME: &str = "token";
const LAST_USE_PRECISION: Duration = Duration::minutes(1);

/// Creates an API token with every scope for the organization `slug`, writes
/// it in the audit log and returns it with its organization.
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

pub struct NewApiToken<'a> {
    pub id: ApiTokenId,
    pub org_id: OrgId,
    pub name: &'a str,
    pub scopes: &'a [ApiScope],
    pub expires_at: Option<OffsetDateTime>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiTokenGrant {
    pub id: ApiTokenId,
    pub org_id: OrgId,
    pub scopes: Vec<ApiScope>,
}

/// # Errors
///
/// The system has no random source.
pub fn new_api_token_secret() -> Result<SecretString, StoreError> {
    Ok(format_api_token(&draw_secret_bytes()?).into())
}

/// Stores `token` with the hash of `secret`, in a transaction of its
/// organization.
///
/// # Errors
///
/// Postgres refuses the row: another organization than the context, or a
/// secret already stored.
pub async fn create_api_token(
    conn: &mut PgConnection,
    token: &NewApiToken<'_>,
    secret: &SecretString,
) -> Result<(), StoreError> {
    let scopes: Vec<&str> = token.scopes.iter().map(|scope| scope.as_str()).collect();
    sqlx::query(
        "INSERT INTO api_tokens (id, org_id, name, token_hash, scopes, expires_at)
         VALUES ($1, $2, $3, $4, $5::api_scope[], $6)",
    )
    .bind(token.id.to_string())
    .bind(token.org_id.to_string())
    .bind(token.name)
    .bind(secret_hash(secret))
    .bind(scopes)
    .bind(token.expires_at)
    .execute(conn)
    .await
    .map_err(StoreError::Query)?;
    Ok(())
}

/// The grant of `secret` at `now`: `None` when it is not a token, unknown or
/// expired. The organization is not known yet, so the lookup runs in the
/// system context.
///
/// # Errors
///
/// Postgres fails, or a row does not match the vocabulary of `cihub-domain`.
pub async fn authenticate(
    pool: &PgPool,
    secret: &SecretString,
    now: OffsetDateTime,
) -> Result<Option<ApiTokenGrant>, StoreError> {
    if !is_api_token(secret.expose_secret()) {
        return Ok(None);
    }

    let mut tx = begin_system(pool).await?;
    let row: Option<(String, String, Vec<String>, Option<OffsetDateTime>)> = sqlx::query_as(
        "SELECT id, org_id, scopes::text[], last_used_at FROM api_tokens
         WHERE token_hash = $1 AND (expires_at IS NULL OR expires_at > $2)",
    )
    .bind(secret_hash(secret))
    .bind(now)
    .fetch_optional(&mut *tx)
    .await
    .map_err(StoreError::Query)?;
    let Some((id, org_id, scopes, last_used_at)) = row else {
        return Ok(None);
    };

    if last_used_at.is_none_or(|at| at <= now - LAST_USE_PRECISION) {
        sqlx::query("UPDATE api_tokens SET last_used_at = $2 WHERE id = $1")
            .bind(&id)
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(StoreError::Query)?;
    }

    tx.commit().await.map_err(StoreError::Query)?;
    let grant = decode(&id, &org_id, &scopes).map_err(StoreError::read("api token", &id))?;
    Ok(Some(grant))
}

fn decode(
    id: &str,
    org_id: &str,
    scopes: &[String],
) -> Result<ApiTokenGrant, Box<dyn std::error::Error + Send + Sync>> {
    Ok(ApiTokenGrant {
        id: id.parse()?,
        org_id: org_id.parse()?,
        scopes: scopes
            .iter()
            .map(|scope| scope.parse())
            .collect::<Result<_, _>>()?,
    })
}

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use cihub_domain::{ConnectionId, ConnectionKind, JobClass, OrgId, UserId};
use cihub_scm::{Scm, ScmPersonToken, ScmUser, SignedIn};
use cihub_store::{
    DatabaseCipher, PgPool, begin_org, begin_system, ensure_user, forget_memberships_before,
    record_avatar, record_membership, record_profile,
};
use serde_json::json;
use sqlx::PgConnection;
use time::OffsetDateTime;
use tracing::info;

use crate::connections::{
    active_connection, outcome_of, request_for_each_connection, sync_installation_after,
};
use crate::installations::connect_installations;
use crate::persons::{keep_token, request_user_sync};
use crate::{Job, Outcome, PlatformError};

pub const SYNC_MEMBERS: &str = "sync_members";

pub fn sync_members_job(org_id: OrgId, connection: ConnectionId, cause: &str) -> Job {
    Job {
        idempotency_key: format!("{SYNC_MEMBERS}:{connection}-{cause}:1"),
        job_type: SYNC_MEMBERS.to_owned(),
        class: JobClass::Repair,
        org_id: Some(org_id),
        connection: Some(connection),
        queue_id: None,
        payload: json!({}),
        attempts: 0,
    }
}

/// Requests the hourly read of the members of the account of each connection
/// that is not suspended, and returns the number of new requests.
///
/// # Errors
///
/// Postgres fails, or a row does not match the identifiers of `cihub-domain`.
pub async fn request_member_syncs(
    conn: &mut PgConnection,
    now: OffsetDateTime,
) -> Result<usize, PlatformError> {
    request_for_each_connection(conn, now, sync_members_job).await
}

/// Reads the members of the account of the connection of `job`, records the
/// role of each, deletes the memberships that the read no longer gives, and
/// returns the number of members. When the SCM answers that the access is
/// lost or the installation is not found, requests the sync of the
/// installation.
///
/// # Errors
///
/// The job has no connection, or Postgres or the SCM fails.
pub async fn run_sync_members(
    pool: &PgPool,
    scm: &dyn Scm,
    job: &Job,
    now: OffsetDateTime,
) -> Result<usize, PlatformError> {
    let (Some(org_id), Some(connection_id)) = (job.org_id, job.connection) else {
        return Err(PlatformError::JobWithoutConnection(
            job.idempotency_key.clone(),
        ));
    };

    let Some(connection) = active_connection(pool, connection_id).await? else {
        return Ok(0);
    };

    let listed = scm.members(&connection, job.class).await;

    sync_installation_after(pool, &connection, &listed, now).await?;
    let members = listed.map_err(PlatformError::Scm)?;
    let mut tx = begin_org(pool, org_id)
        .await
        .map_err(PlatformError::Store)?;

    for member in &members {
        let user_id = user_of(&mut tx, &member.user).await?;
        record_avatar(&mut tx, user_id, member.avatar_url.as_deref())
            .await
            .map_err(PlatformError::Store)?;
        record_membership(&mut tx, org_id, user_id, member.role, now)
            .await
            .map_err(PlatformError::Store)?;
    }
    // An organization has one connection: `installations.rs` creates one
    // organization per installation. A second connection would need the
    // memberships of each connection, or it would forget the members of the
    // first.
    let forgotten = forget_memberships_before(&mut tx, org_id, now)
        .await
        .map_err(PlatformError::Store)?;

    tx.commit().await.map_err(PlatformError::Query)?;
    info!(
        org_id = %org_id,
        connection_id = %connection_id,
        members = members.len(),
        forgotten,
        "members read"
    );
    Ok(members.len())
}

pub fn sync_members_executor(
    scm: Arc<dyn Scm>,
) -> impl Fn(PgPool, Job, OffsetDateTime) -> Pin<Box<dyn Future<Output = Outcome> + Send>>
+ Send
+ Sync
+ 'static {
    move |pool, job, now| {
        let scm = Arc::clone(&scm);
        Box::pin(async move { outcome_of(run_sync_members(&pool, scm.as_ref(), &job, now).await) })
    }
}

/// Records the person of a sign-in, their profile, their token, sealed, and
/// their role in each organization among those that the SCM gave, then asks
/// for the read of their repositories. The sign-in first connects
/// the installations of the person that CIHub does not know yet, as a sync
/// would; a suspended installation gives no organization. The sign-in deletes
/// no membership: it reads only the installations that the person reaches, so
/// the hourly read of the members decides who left. Returns the user.
///
/// # Errors
///
/// Postgres fails, or a row does not match the identifiers of `cihub-domain`.
pub async fn record_sign_in(
    pool: &PgPool,
    kind: ConnectionKind,
    cipher: &DatabaseCipher,
    (signed_in, token): (&SignedIn, &ScmPersonToken),
    now: OffsetDateTime,
) -> Result<UserId, PlatformError> {
    let connected = connect_installations(pool, kind, &signed_in.installations, now).await?;
    if connected > 0 {
        info!(
            login = signed_in.user.login,
            organizations = connected,
            "installations connected at a sign-in"
        );
    }

    let installations: Vec<&str> = signed_in
        .memberships
        .iter()
        .map(|membership| membership.connection_external_id.as_str())
        .collect();
    let mut tx = begin_system(pool).await.map_err(PlatformError::Store)?;
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT external_id, org_id FROM scm_connections
         WHERE external_id = ANY($1) AND suspended_at IS NULL",
    )
    .bind(&installations)
    .fetch_all(&mut *tx)
    .await
    .map_err(PlatformError::Query)?;
    let orgs: HashMap<String, String> = rows.into_iter().collect();

    let user_id = user_of(&mut tx, &signed_in.user).await?;
    let profile = &signed_in.profile;
    record_profile(
        &mut tx,
        user_id,
        profile.name.as_deref(),
        profile.avatar_url.as_deref(),
    )
    .await
    .map_err(PlatformError::Store)?;
    for membership in &signed_in.memberships {
        let Some(org) = orgs.get(&membership.connection_external_id) else {
            continue;
        };
        let org_id: OrgId = org
            .parse()
            .map_err(PlatformError::read("organization", org))?;
        record_membership(&mut tx, org_id, user_id, membership.role, now)
            .await
            .map_err(PlatformError::Store)?;
    }

    keep_token(&mut tx, cipher, user_id, token, now).await?;
    request_user_sync(
        &mut tx,
        user_id,
        &format!("signed-in-{}", now.unix_timestamp()),
        now,
    )
    .await?;
    tx.commit().await.map_err(PlatformError::Query)?;
    Ok(user_id)
}

async fn user_of(conn: &mut PgConnection, user: &ScmUser) -> Result<UserId, PlatformError> {
    #[expect(
        clippy::disallowed_methods,
        reason = "the identifier of an account of the SCM, used only when it is new"
    )]
    let candidate = UserId::new();
    ensure_user(conn, candidate, &user.external_id, &user.login)
        .await
        .map_err(PlatformError::Store)
}

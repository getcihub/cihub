use std::error::Error;
use std::future::Future;
use std::pin::Pin;

use cihub_domain::{ModuleName, OrgId};
use sqlx::PgConnection;
use tokio_util::sync::CancellationToken;

use crate::{Executors, PlatformError};

/// What a loop of a module gives when it cannot start.
pub type LoopError = Box<dyn Error + Send + Sync>;

/// A loop of a module, which runs until its token is cancelled.
pub type ModuleLoop = Box<
    dyn FnOnce(CancellationToken) -> Pin<Box<dyn Future<Output = Result<(), LoopError>> + Send>>
        + Send,
>;

/// What a module gives to the roles of the server (`DECISIONS.md` D94): its
/// executors for the worker, its loop for the scheduler, and its routes for
/// the web role (D159). The subscriptions come with lot 11.
pub struct Module {
    pub name: ModuleName,
    /// Registers the executors of the module in the registry of the worker.
    pub executors: Box<dyn FnOnce(&mut Executors) + Send>,
    /// The loop of the module in the scheduler role.
    pub scheduler: ModuleLoop,
    pub api: ModuleApi,
}

/// The routes of a module under `/api/v1`, and their `OpenAPI` document. The
/// web role authenticates each request before them: a route reads the
/// [`ApiGrant`](crate::ApiGrant) of its request.
pub struct ModuleApi {
    pub routes: axum::Router,
    pub openapi: utoipa::openapi::OpenApi,
}

/// Whether `org` has the module `module` at the moment of the transaction of
/// `conn`.
///
/// # Errors
///
/// Postgres fails.
pub async fn module_enabled(
    conn: &mut PgConnection,
    org: OrgId,
    module: ModuleName,
) -> Result<bool, PlatformError> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM org_modules WHERE org_id = $1 AND module = $2::module_name)",
    )
    .bind(org.to_string())
    .bind(module.as_str())
    .fetch_one(conn)
    .await
    .map_err(PlatformError::Query)
}

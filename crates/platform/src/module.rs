use std::error::Error;
use std::future::Future;
use std::pin::Pin;

use tokio_util::sync::CancellationToken;

use crate::{Executors, Subscription};

pub type LoopError = Box<dyn Error + Send + Sync>;

pub type ModuleLoop = Box<
    dyn FnOnce(CancellationToken) -> Pin<Box<dyn Future<Output = Result<(), LoopError>> + Send>>
        + Send,
>;

/// What a module gives to the roles of the server: its
/// executors for the worker, its loop for the scheduler, its routes for
/// the web role, and its subscriptions to the logs for the notifier.
pub struct Module {
    pub executors: Box<dyn FnOnce(&mut Executors) + Send>,
    pub scheduler: ModuleLoop,
    pub api: ModuleApi,
    pub subscriptions: Vec<Subscription>,
}

/// The routes of a module under `/api/v1`, and their `OpenAPI` document. The
/// web role authenticates each request before them: a route reads the
/// [`ApiGrant`](crate::ApiGrant) of its request.
pub struct ModuleApi {
    pub routes: axum::Router,
    pub openapi: utoipa::openapi::OpenApi,
}

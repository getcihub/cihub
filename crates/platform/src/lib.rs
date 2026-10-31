//! Internal transport of CIHub on Postgres: the queue leases, the inbox of the
//! webhook deliveries, the wake-up of the schedulers, the jobs, their worker
//! loop, the daily maintenance (lot 3), the budgets of the connections, the
//! routing and the sweep of the deliveries, the sync of the installations of
//! the App into organizations and connections, the read of the repositories
//! of the connections, the roles of the people who sign in and of the members
//! of each account, the users and their profile, the token of a person and what it reaches, the API tokens, the contract of a
//! module, and what the routes of the modules share. The roles talk only through these tables and their notifications.
//! It holds no queue logic: it decides who works on a queue and in which order
//! its deliveries come, not what the queue does.

mod api;
mod budget;
mod connections;
mod inbox;
mod installations;
mod jobs;
mod lease;
mod maintenance;
mod members;
mod module;
mod news;
mod page;
mod persons;
mod routing;
mod subscriptions;
mod sweep;
mod tokens;
mod users;
mod wake;
mod worker;

pub use api::{
    ApiError, ApiGrant, ApiJson, ApiPath, ApiQuery, GrantActor, document_error, error_examples,
    error_responses,
};
pub use budget::{ConnectionBudget, record_budgets, run_budget_writer};
pub use connections::{
    SYNC_REPOS, request_repo_syncs, run_sync_repos, sync_repos_executor, sync_repos_job,
};
pub use inbox::{
    Delivery, MAX_DELIVERY_ATTEMPTS, delete_delivery, pending_deliveries, record_delivery_failure,
    store_delivery,
};
pub use installations::{
    SYNC_INSTALLATION, SYNC_INSTALLATIONS, request_installations_sync,
    request_installations_sync_at_start, run_sync_installation, run_sync_installations,
    sync_installation_executor, sync_installation_job, sync_installations_executor,
};
pub use jobs::{
    JOB_LOCK, JOBS_PER_CONNECTION, Job, MAX_ATTEMPTS, Retry, backoff, claim_jobs, complete_job,
    enqueue_job, fail_job,
};
pub use lease::{LEASE_TTL, acquire_leases, lock_queue, release_leases};
pub use maintenance::{
    MAINTENANCE, execute_maintenance, maintenance_job, request_maintenance, run_maintenance,
};
pub use members::{
    Membership, ORG_IS_SUSPENDED, SYNC_MEMBERS, forget_memberships_before, memberships_of,
    record_membership, record_sign_in, request_member_syncs, run_sync_members,
    sync_members_executor, sync_members_job,
};
pub use module::{LoopError, Module, ModuleApi, ModuleLoop};
pub use news::{QueueChange, QueueNews, QueueNewsFeed, notify_queue_changed};
pub use page::{CursorTime, PAGE_ERRORS, Page, cursor_of};
pub use persons::{
    ACCESS_LIFETIME, Access, PersonToken, SYNC_USER, forget_permissions_before,
    forget_tokens_without_session, keep_person_token, readable_repos, repo_permission_now,
    request_user_sync, request_user_sync_when_old, role_now, run_sync_user, sync_user_executor,
    sync_user_job, token_of_person,
};
pub use routing::{ROUTE_POLL, Route, find_route, run_router};
pub use subscriptions::{
    SUBSCRIPTIONS_POLL, Subscription, SubscriptionFuture, SubscriptionHandler, deliver_all,
    run_subscriptions,
};
pub use sweep::{SWEEP_DELIVERIES, SWEEP_EVERY, request_sweep, run_sweep, sweep_executor};
pub use tokens::{
    ApiTokenGrant, NewApiToken, authenticate, create_api_token, create_token, new_api_token_secret,
};
pub use users::{UserProfile, ensure_user, profile_of, record_profile};
pub use wake::{
    MIN_WAIT, QueueWakeup, TICK, Wake, begin_reconciliation, mark_dirty, schedule_tick,
    until_next_tick,
};
pub use worker::{Executors, JOB_POLL, Outcome, error_chain, job_span, run_worker};

#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("query failed")]
    Query(#[source] sqlx::Error),
    #[error("store failed")]
    Store(#[source] cihub_store::StoreError),
    #[error("SCM call failed")]
    Scm(#[source] cihub_scm::ScmError),
    #[error("{what} {id} does not match cihub-domain")]
    Read {
        what: &'static str,
        id: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("job {0} has no connection")]
    JobWithoutConnection(String),
    #[error("job {0} has no installation")]
    JobWithoutInstallation(String),
    #[error("job {0} has no user")]
    JobWithoutUser(String),
    #[error(
        "the installation {installation} cannot create the organization {slug}: another organization has this slug"
    )]
    SlugTaken { slug: String, installation: String },
    #[error("no organization has the slug {0}")]
    UnknownOrganization(String),
    #[error("the subscription {name} failed")]
    Subscription {
        name: &'static str,
        #[source]
        source: crate::LoopError,
    },
}

impl PlatformError {
    pub(crate) fn read<E: Into<Box<dyn std::error::Error + Send + Sync>>>(
        what: &'static str,
        id: &str,
    ) -> impl FnOnce(E) -> Self {
        let id = id.to_owned();
        move |source| Self::Read {
            what,
            id,
            source: source.into(),
        }
    }
}

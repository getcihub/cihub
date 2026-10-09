//! The `worker` role: the jobs of the modules, run by `cihub-platform`, and
//! the record of the budgets of the GitHub App.

use std::sync::Arc;

use cihub_domain::{ConnectionId, ConnectionKind};
use cihub_github::GithubApp;
use cihub_platform::{
    ConnectionBudget, Executors, JOB_POLL, MAINTENANCE, SWEEP_DELIVERIES, SYNC_INSTALLATION,
    SYNC_INSTALLATIONS, SYNC_REPOS, execute_maintenance, run_budget_writer, sweep_executor,
    sync_installation_executor, sync_installations_executor, sync_repos_executor,
};
use cihub_scm::Scm;

use super::start::Shared;
use super::{merge_queue, now};

pub(super) async fn run(shared: Shared) -> anyhow::Result<()> {
    let Shared {
        pool,
        process,
        github,
        cancel,
        ..
    } = shared;
    let mut executors = Executors::default();
    executors.register(MAINTENANCE, execute_maintenance);
    (merge_queue(&pool, &github, &process).executors)(&mut executors);
    let scm: Arc<dyn Scm> = github.app.clone();
    executors.register(SWEEP_DELIVERIES, sweep_executor(Arc::clone(&scm)));
    executors.register(SYNC_REPOS, sync_repos_executor(Arc::clone(&scm)));
    let kind = ConnectionKind::GithubApp;
    executors.register(
        SYNC_INSTALLATION,
        sync_installation_executor(Arc::clone(&scm), kind),
    );
    executors.register(SYNC_INSTALLATIONS, sync_installations_executor(scm, kind));
    let worker = cihub_platform::run_worker(
        pool.clone(),
        process,
        Arc::new(executors),
        now,
        JOB_POLL,
        cancel.clone(),
    );
    let app = github.app;
    let urgent = app.budget_urgent();
    let changed = move || connection_budgets(&app);
    let writer = run_budget_writer(pool, changed, urgent, cancel);
    tokio::join!(worker, writer).0?;
    Ok(())
}

fn connection_budgets(app: &GithubApp) -> Vec<(ConnectionId, ConnectionBudget)> {
    app.take_changed_budgets()
        .into_iter()
        .map(|(connection, budget)| {
            let rate = budget.lowest();
            let recorded = ConnectionBudget {
                mode: budget.mode,
                mode_reason: budget.mode_reason,
                rate_remaining: rate.map(|rate| rate.remaining),
                rate_limit: rate.map(|rate| rate.limit),
                rate_reset_at: rate.map(|rate| rate.reset_at),
                paused_until: budget.paused_until,
                consecutive_failures: budget.failures,
            };
            (connection, recorded)
        })
        .collect()
}

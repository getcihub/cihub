//! The `web` role: the public address, served by `cihub-web` (webhooks, then
//! the API of the modules), and the routing of the stored
//! deliveries.

use std::sync::Arc;

use anyhow::Context;
use cihub_platform::{QueueNewsFeed, ROUTE_POLL, run_router};
use cihub_scm::Scm;
use cihub_web::{Dashboard, GithubWebhooks};
use tokio::net::TcpListener;
use tracing::info;

use super::start::Shared;
use super::{merge_queue, now};

pub(super) async fn run(shared: Shared) -> anyhow::Result<()> {
    let modules = vec![merge_queue(&shared).api];
    let Shared {
        pool,
        http_addr,
        public_url,
        github,
        cancel,
        ..
    } = shared;
    let listener = TcpListener::bind(http_addr)
        .await
        .with_context(|| format!("cannot listen on {http_addr}"))?;

    info!(http_addr = %listener.local_addr()?, "public address served");
    let webhooks = GithubWebhooks {
        secret: github.webhook_secret,
    };
    let scm: Arc<dyn Scm> = github.app;
    let news = QueueNewsFeed::start(pool.clone(), cancel.clone())
        .await
        .context("the changes of the queues cannot be heard")?;
    let dashboard = Dashboard {
        scm,
        public_url,
        news,
    };
    let router = cihub_web::router(pool.clone(), now, webhooks, modules, dashboard);
    let serve = async {
        cihub_web::serve(listener, router, cancel.clone())
            .await
            .context("public address server failed")
    };
    let route = async {
        run_router(
            pool,
            cihub_github::translate,
            now,
            ROUTE_POLL,
            cancel.clone(),
        )
        .await
        .context("the routing of the deliveries cannot start")
    };

    tokio::try_join!(serve, route)?;
    Ok(())
}

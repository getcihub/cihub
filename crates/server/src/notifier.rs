//! The `notifier` role: the subscriptions of the modules to the logs of the
//! queues, the facts of the analytics first (lot 11a).

use anyhow::Context;
use cihub_platform::run_subscriptions;

use super::merge_queue;
use super::start::Shared;

pub(super) async fn run(shared: Shared) -> anyhow::Result<()> {
    let module = merge_queue(&shared);
    run_subscriptions(shared.pool, module.subscriptions, shared.cancel)
        .await
        .context("the subscriptions to the logs stopped")
}

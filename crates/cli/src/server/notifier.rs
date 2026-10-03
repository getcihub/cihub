//! The `notifier` role: the delivery of the outbox, which arrives with Lot 11
//! (`DECISIONS.md` D65). Until then, the role waits for the shutdown.

use super::start::Shared;

pub(super) async fn run(shared: Shared) {
    shared.cancel.cancelled().await;
}

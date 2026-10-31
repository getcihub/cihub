//! Rules on the requests, the queue mode and the changes of the PRs.

use cihub_config::QueueSettings;
use cihub_domain::{ItemId, ItemState, JobError, PrState, QueueState, Reason, RunId, RunStatus};
use time::{Duration, OffsetDateTime};

use crate::Snapshot;
use crate::ids::IdGen;
use crate::snapshot::Request;

use crate::plan::Step;
use crate::plan::events::{
    ItemExtras, assembly_reason, item_to, item_to_with, queue_to, run_to, submitted,
};
use crate::plan::rules::formation::{direct_merge, merge_ready, window_to};
use crate::plan::rules::readiness::not_ready_items;

/// A decision for the first request that can receive one. In `paused`,
/// only the requests on the queue itself.
pub(in crate::plan) fn requests(
    s: &Snapshot,
    _: &QueueSettings,
    _: OffsetDateTime,
    ids: &mut IdGen,
) -> Option<Step> {
    let (id, request) = s.requests.iter().find(|(_, r)| match r {
        Request::Submit { pr_number, .. } => {
            s.state != QueueState::Paused && s.current_item(*pr_number).is_none()
        }
        Request::Control(_) | Request::AccessLost | Request::AccessRestored => true,
        _ => s.state != QueueState::Paused,
    })?;

    let item = |item_id: &ItemId| s.items.get(item_id);
    let run = |run_id: &RunId| s.runs.get(run_id);

    let kind = match request {
        Request::Submit { .. } => submitted(s, ids),
        Request::Cancel { item_id, by_admin } => {
            let reason = if *by_admin {
                Reason::AdminRequested
            } else {
                Reason::UserRequested
            };
            item_to(item(item_id)?, ItemState::Cancelled, reason)
        }
        Request::Restart { item_id, by_api } => {
            let reason = if *by_api {
                Reason::TestsRestartedByApiRequest
            } else {
                Reason::TestsRestartedByUser
            };
            item_to(item(item_id)?, ItemState::Pending, reason)
        }
        Request::Assembled { run_id } => run_to(
            run(run_id)?,
            RunStatus::InProgress,
            Reason::TestBranchAssembled,
            Vec::new(),
        ),
        Request::AssemblyFailed { run_id, error } => run_to(
            run(run_id)?,
            RunStatus::Failed,
            assembly_reason(*error),
            Vec::new(),
        ),
        Request::MergeSucceeded {
            item_id,
            merged_sha,
        } => {
            let item = item(item_id)?;
            let is_direct = s
                .direct_merge_candidate()
                .is_some_and(|candidate| candidate.id == item.id);
            let reason = match item.state {
                ItemState::Pending if is_direct => Reason::MergeGraphDirectMerge,
                ItemState::Pending => Reason::ManualMerge,
                _ => Reason::MergeGraphMerge,
            };
            item_to_with(
                item,
                ItemState::Merged,
                reason,
                ItemExtras {
                    merged_sha: Some(merged_sha.clone()),
                    ..ItemExtras::default()
                },
            )
        }
        Request::MergeFailed {
            item_id,
            error: JobError::PrAlreadyMerged,
            ..
        } => item_to(item(item_id)?, ItemState::Merged, Reason::ManualMerge),
        Request::MergeFailed {
            item_id,
            direct,
            error,
        } => {
            let reason = match error {
                JobError::RateLimitExceeded => Reason::MergeRateLimitExceeded,
                JobError::BranchProtectionUnmet => Reason::MergeBranchProtectionUnmet,
                JobError::MergeAlreadyInProgress => Reason::MergeAlreadyInProgress,
                JobError::RepeatedServerErrors => Reason::MergeRepeatedServerErrors,
                JobError::HeadChanged => Reason::PrUpdatedAtMergeTime,
                _ if *direct => Reason::DirectMergeRejected,
                _ => Reason::MergeRejected,
            };

            item_to(item(item_id)?, ItemState::Failed, reason)
        }
        Request::Control(control) => queue_to(s, control.target(), Reason::AdminRequested),
        Request::AccessLost => queue_to(s, QueueState::Paused, Reason::AccessLost),
        Request::AccessRestored => queue_to(s, QueueState::Running, Reason::AccessRestored),
    };
    Step::one(Some(*id), kind)
}

/// A mode change waits in `switching_modes` until no item is live.
pub(in crate::plan) fn mode_switch(
    s: &Snapshot,
    _: &QueueSettings,
    _: OffsetDateTime,
    _: &mut IdGen,
) -> Option<Step> {
    match s.state {
        QueueState::Running if s.requested_mode != s.mode => Step::one(
            None,
            queue_to(s, QueueState::SwitchingModes, Reason::ModeChanged),
        ),
        QueueState::SwitchingModes if s.items.values().all(|i| i.state.is_terminal()) => Step::one(
            None,
            queue_to(s, QueueState::Running, Reason::ModeSwitchCompleted),
        ),
        _ => None,
    }
}

/// The window comes back within the bounds of the settings when they
/// change.
pub(in crate::plan) fn window_bounds(
    s: &Snapshot,
    settings: &QueueSettings,
    _: OffsetDateTime,
    _: &mut IdGen,
) -> Option<Step> {
    let window = s.window?;
    Step::one(None, window_to(s, settings, window)?)
}

/// The item leaves the queue when its PR changes on the SCM.
pub(in crate::plan) fn pr_changes(
    s: &Snapshot,
    settings: &QueueSettings,
    _: OffsetDateTime,
    _: &mut IdGen,
) -> Option<Step> {
    s.live_items().find_map(|item| {
        let observed = s.prs.get(&item.pr_number)?;
        let pr = &observed.value;

        let (to, reason) = if pr.state == PrState::Merged {
            if merge_ready(s, item) || direct_merge(s, settings).is_some_and(|d| d.id == item.id) {
                return None;
            }

            (ItemState::Merged, Reason::ManualMerge)
        } else if pr.state == PrState::Closed {
            (ItemState::Cancelled, Reason::PrClosed)
        } else if pr.head_sha != item.head_sha {
            (ItemState::Cancelled, Reason::PrPushedTo)
        } else if pr.base_branch != s.target_branch {
            (ItemState::Cancelled, Reason::PrBaseBranchChanged)
        } else if pr.draft {
            (ItemState::Cancelled, Reason::PrConvertedToDraft)
        } else if item.submitted_via == cihub_domain::SubmittedVia::Label
            && !pr.labels.contains(&settings.enqueue_label.value)
        {
            (ItemState::Cancelled, Reason::LabelRemoved)
        } else {
            return None;
        };

        Step::one(Some(observed.fact_id), item_to(item, to, reason))
    })
}

/// An item that stays `not_ready` too long leaves the queue.
pub(in crate::plan) fn not_ready_timeout(
    s: &Snapshot,
    settings: &QueueSettings,
    now: OffsetDateTime,
    _: &mut IdGen,
) -> Option<Step> {
    let timeout = Duration::hours(i64::from(settings.not_ready_timeout_hours.value));
    let item = not_ready_items(s).find(|i| now >= i.submitted_at + timeout)?;
    Step::one(
        None,
        item_to(item, ItemState::Cancelled, Reason::NotReadyTimeoutExpired),
    )
}

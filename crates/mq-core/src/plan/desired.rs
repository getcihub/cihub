//! The desired state and what each PR must show.

use std::collections::BTreeMap;

use cihub_config::QueueSettings;
use cihub_domain::ItemId;
use time::OffsetDateTime;

use crate::Snapshot;
use crate::surface::{BatchWait, Surface, surface};

use super::{CheckRead, Desired, Head, Merge, TestBranch};
use crate::plan::reads::{CHECK_READ_EVERY, grid, last_report, read_due, target_points};
use crate::plan::rules::formation::{Next, direct_merge, merge_ready, next_group};

pub(super) fn desired(
    s: &Snapshot,
    settings: &QueueSettings,
    now: OffsetDateTime,
    surfaces: Vec<Surface>,
) -> Desired {
    let heads = |ids: &[ItemId]| -> Vec<Head> {
        ids.iter()
            .filter_map(|id| s.items.get(id))
            .map(|item| Head {
                pr_number: item.pr_number,
                sha: item.head_sha.clone(),
            })
            .collect()
    };

    let test_branches = s
        .runs
        .values()
        .filter(|r| !r.status.is_terminal())
        .filter_map(|run| {
            let group = s.groups.get(&run.group_id)?;
            Some(TestBranch {
                run_id: run.id,
                name: run.test_branch.clone(),
                base_sha: group.base_sha.clone(),
                predecessors: heads(&group.predecessor_ids),
                members: heads(&group.item_ids),
            })
        })
        .collect();

    let merges = if s.is_stopped() {
        Vec::new()
    } else {
        s.queue_order()
            .into_iter()
            .filter(|i| merge_ready(s, i))
            .filter_map(|i| {
                let run = s.runs.get(&i.verified_by_run_id?)?;
                Some(Merge {
                    item_id: i.id,
                    pr_number: i.pr_number,
                    head_sha: i.head_sha.clone(),
                    run_id: Some(run.id),
                    tested_sha: run.test_sha.clone()?,
                })
            })
            .chain(direct_merge(s, settings).map(|i| Merge {
                item_id: i.id,
                pr_number: i.pr_number,
                head_sha: i.head_sha.clone(),
                run_id: None,
                tested_sha: i.head_sha.clone(),
            }))
            .collect()
    };

    let observe = s
        .live_items()
        .filter_map(|item| read_due(s, item, now))
        .collect();
    let check_reads = s
        .runs_in_progress()
        .filter_map(|run| {
            let (point, _) = grid(last_report(s, run), CHECK_READ_EVERY, now);
            Some(CheckRead {
                run_id: run.id,
                test_sha: run.test_sha.clone()?,
                test_branch: run.test_branch.clone(),
                point: point?,
            })
        })
        .collect();

    let target_read = target_points(s, now).and_then(|(point, _)| point);

    Desired {
        test_branches,
        merges,
        observe,
        check_reads,
        target_read,
        surfaces,
    }
}

/// The surface of each PR: its live item, else the last item that this
/// reconciliation concluded. The members of the batch that waits show when it
/// closes, with the time that `wake_at` uses.
pub(super) fn surfaces(
    s: &Snapshot,
    settings: &QueueSettings,
    now: OffsetDateTime,
    concluded: Vec<Surface>,
) -> Vec<Surface> {
    let mut by_pr: BTreeMap<u64, Surface> = concluded
        .into_iter()
        .map(|surface| (surface.pr_number, surface))
        .collect();

    let order = s.queue_order();
    let waiting = match next_group(s, &order, settings, now) {
        Some(Next::Wait { members, until }) => Some((members, until)),
        _ => None,
    };

    for item in s.live_items() {
        let mut surface = surface(s, &order, item);
        if let Some((members, until)) = &waiting
            && members.iter().any(|m| m.id == item.id)
        {
            surface.batch_wait = Some(BatchWait {
                members: members.len(),
                size: settings
                    .of_profile(item.profile.as_deref())
                    .batching_size
                    .value,
                closes_at: *until,
            });
        }
        by_pr.insert(item.pr_number, surface);
    }

    by_pr.into_values().collect()
}

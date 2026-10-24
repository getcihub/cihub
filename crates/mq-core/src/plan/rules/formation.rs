//! Rules that form the groups, the batches and the window, and merge directly.

use cihub_config::QueueSettings;
use cihub_domain::{
    DirectMergeMode, EventKind, GroupId, GroupKind, GroupState, Item, ItemId, ItemState,
    QueueState, Reason,
};
use time::{Duration, OffsetDateTime};

use crate::Snapshot;
use crate::graph::{overlap, predecessors, targets};
use crate::ids::IdGen;
use crate::snapshot::Subject;

use crate::plan::Step;
use crate::plan::events::new_run;

/// Forms the next group, while the window has room: a `single` group, or a
/// batch with batching enabled.
pub(in crate::plan) fn formation(
    s: &Snapshot,
    settings: &QueueSettings,
    now: OffsetDateTime,
    ids: &mut IdGen,
) -> Option<Step> {
    let base = &s.target.head.as_ref()?.value;

    if !window_has_room(s, settings) || direct_merge(s, settings).is_some() {
        return None;
    }

    let order = s.queue_order();
    let Next::Form { index, members } = next_group(s, &order, settings, now)? else {
        return None;
    };

    let first = members.first()?;
    let group_id = GroupId::from_ulid(ids.next());
    let kind = if members.len() > 1 {
        GroupKind::Batch
    } else {
        GroupKind::Single
    };

    let group_formed = EventKind::GroupFormed {
        group_id,
        kind,
        item_ids: members.iter().map(|i| i.id).collect(),
        predecessor_ids: predecessors(s, &order, index),
        base_sha: base.clone(),
        parent_group_id: None,
    };
    let run = new_run(
        group_id,
        first.pr_number,
        1,
        Reason::TestRunStarted,
        settings,
        ids,
    );

    Some(Step {
        cause: s.cause(Subject::Item(first.id)),
        kinds: vec![group_formed, run],
    })
}

/// The item is in a group of several PRs that failed, or in one of its halves:
/// no group forms on top of it until the bisection ends.
fn in_bisection(s: &Snapshot, item: &Item) -> bool {
    item.group_id
        .and_then(|g| s.groups.get(&g))
        .is_some_and(|g| {
            !g.state.is_terminal()
                && (g.kind == GroupKind::Bisection
                    || (g.state == GroupState::PendingFailure && g.item_ids.len() > 1))
        })
}

/// The next group of the queue, or its batch that waits until `until`.
pub(in crate::plan) enum Next<'a> {
    Form {
        index: usize,
        members: Vec<&'a Item>,
    },
    Wait {
        members: Vec<&'a Item>,
        until: OffsetDateTime,
    },
}

/// The first pending item whose predecessors are all in groups, at `index` in
/// `order`. With the batching of its profile, the pending items of the same
/// profile and of the same lane that follow it join it, up to
/// `batching.size`, until an item that is not pending, is `no_batch`, has
/// another profile or is in another lane: a batch merges with one set of
/// settings, and its run tests every member on top of the predecessors of the
/// first. A batch that is not full waits until its oldest member has waited
/// `batching.max_wait_minutes` in the queue, unless the item right behind it
/// closes it: the batch cannot grow, and that item would wait for nothing.
pub(in crate::plan) fn next_group<'a>(
    s: &Snapshot,
    order: &[&'a Item],
    settings: &QueueSettings,
    now: OffsetDateTime,
) -> Option<Next<'a>> {
    let index = order.iter().enumerate().position(|(index, item)| {
        item.state == ItemState::Pending
            && predecessors(s, order, index).iter().all(|id| {
                s.items
                    .get(id)
                    .is_some_and(|p| p.state != ItemState::Pending && !in_bisection(s, p))
            })
    })?;

    let first = order[index];
    let settings = settings.of_profile(first.profile.as_deref());
    if !settings.batching_enabled.value || first.no_batch {
        return Some(Next::Form {
            index,
            members: vec![first],
        });
    }

    let size = usize::try_from(settings.batching_size.value).unwrap_or(usize::MAX);
    let lane = predecessors(s, order, index);
    let mut members = vec![first];
    while members.len() < size && joins_batch(s, order, index + members.len(), &members, &lane) {
        members.push(order[index + members.len()]);
    }

    let closed = order.get(index + members.len()).is_some_and(|i| {
        i.state == ItemState::Pending
            && !joins_batch(s, order, index + members.len(), &members, &lane)
    });
    if closed {
        return Some(Next::Form { index, members });
    }

    let wait = Duration::minutes(i64::from(settings.batching_max_wait_minutes.value));
    let ready = members
        .iter()
        .filter_map(|i| i.admitted_at)
        .min()
        .map(|t| t + wait);

    match ready {
        Some(ready) if members.len() < size && now < ready => Some(Next::Wait {
            members,
            until: ready,
        }),
        _ => Some(Next::Form { index, members }),
    }
}

/// Whether `order[position]` joins the batch of `members`, which the run of
/// the batch tests on top of `lane`, the predecessors of its first member:
/// the item waits, accepts a batch, has the profile of the batch, overlaps
/// a member, and depends on nothing outside the batch that `lane` does not
/// contain, so that its test contains all its predecessors.
fn joins_batch(
    s: &Snapshot,
    order: &[&Item],
    position: usize,
    members: &[&Item],
    lane: &[ItemId],
) -> bool {
    let Some(item) = order.get(position) else {
        return false;
    };
    let first = members[0];
    let is_member = |id: &ItemId| members.iter().any(|m| m.id == *id);

    item.state == ItemState::Pending
        && !item.no_batch
        && item.profile == first.profile
        && members
            .iter()
            .any(|m| overlap(targets(s, m), targets(s, item)))
        && predecessors(s, order, position)
            .iter()
            .filter(|id| !is_member(id))
            .eq(lane)
}

/// The speculation window of a queue whose last change set it to `window`:
/// the floor until its first change.
pub fn current_window(window: Option<u32>, settings: &QueueSettings) -> u32 {
    window.unwrap_or(settings.window_floor.value)
}

/// The decision that moves the window to `after`, within the bounds of the
/// settings, or `None` when the window does not change.
pub(in crate::plan) fn window_to(
    s: &Snapshot,
    settings: &QueueSettings,
    after: u32,
) -> Option<EventKind> {
    let before = current_window(s.window, settings);
    let after = after.clamp(settings.window_floor.value, settings.window_ceiling.value);
    (after != before).then_some(EventKind::QueueWindowChanged { before, after })
}

/// The window is the number of runs in progress outside bisections.
pub(in crate::plan) fn window_has_room(s: &Snapshot, settings: &QueueSettings) -> bool {
    let window = current_window(s.window, settings);
    let running = s
        .live_groups()
        .filter(|g| g.kind != GroupKind::Bisection && s.live_run(g.id).is_some())
        .count();
    running < usize::try_from(window).unwrap_or(usize::MAX)
}

/// The bisection runs draw from their own pool, `bisection_concurrency`.
pub(in crate::plan) fn bisection_room(s: &Snapshot, settings: &QueueSettings) -> usize {
    let running = s
        .live_groups()
        .filter(|g| g.kind == GroupKind::Bisection && s.live_run(g.id).is_some())
        .count();
    usize::try_from(settings.bisection_concurrency.value)
        .unwrap_or(usize::MAX)
        .saturating_sub(running)
}

/// The item that the queue merges directly, without a test branch: with
/// `direct_merge_mode: always`, the first item of a running queue without a
/// live group, when it is `pending` and no direct merge of it was tried.
pub(in crate::plan) fn direct_merge<'a>(
    s: &'a Snapshot,
    settings: &QueueSettings,
) -> Option<&'a Item> {
    if settings.direct_merge_mode.value != DirectMergeMode::Always
        || s.state != QueueState::Running
        || s.live_groups().next().is_some()
    {
        return None;
    }

    let first = *s.queue_order().first()?;
    (first.state == ItemState::Pending && !first.direct_merge_tried).then_some(first)
}

/// A tested item merges once its predecessors, and the members ahead of it
/// in its group, have merged.
pub(in crate::plan) fn merge_ready(s: &Snapshot, item: &Item) -> bool {
    let merged = |id: &ItemId| {
        s.items
            .get(id)
            .is_some_and(|i| i.state == ItemState::Merged)
    };
    item.state == ItemState::TestsPassed
        && item
            .group_id
            .and_then(|g| s.groups.get(&g))
            .is_some_and(|g| {
                g.state == GroupState::TestsPassed
                    && g.predecessor_ids.iter().all(merged)
                    && g.item_ids
                        .iter()
                        .take_while(|id| **id != item.id)
                        .all(merged)
            })
}

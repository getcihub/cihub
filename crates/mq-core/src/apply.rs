//! `apply`: the pure function that applies one event to the snapshot. Replay
//! uses it alone and never calls `plan`.

use cihub_domain::{
    Actor, CheckObservation, Event, EventId, EventKind, GroupId, GroupState, Item, ItemId,
    ItemState, JobError, Mergeability, QueueState, Reason, RunId, RunStatus, TestGroup, TestRun,
    TransitionError, check_transition,
};

use crate::Snapshot;
use crate::snapshot::{Observed, PrObservation, RECENT_HEADS, Request, Subject, TargetsUpload};

/// What `apply` did with an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Applied,
    /// The fact changes nothing, or is a request that cannot receive a
    /// decision: the caller does not write it.
    Ignored,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ApplyError {
    #[error("event {event_id} is for queue {event_queue}, not this queue")]
    WrongQueue {
        event_id: EventId,
        event_queue: cihub_domain::QueueId,
    },
    #[error("item creation {event_id} cites no pending submission")]
    NoSubmission { event_id: EventId },
    #[error("unknown item {0}")]
    UnknownItem(ItemId),
    #[error("unknown group {0}")]
    UnknownGroup(GroupId),
    #[error("unknown run {0}")]
    UnknownRun(RunId),
    #[error("group {0} already exists")]
    GroupExists(GroupId),
    #[error("run {0} already exists")]
    RunExists(RunId),
    #[error("item {item_id} is {actual}, not {expected}")]
    ItemStateMismatch {
        item_id: ItemId,
        expected: ItemState,
        actual: ItemState,
    },
    #[error("group {group_id} is {actual}, not {expected}")]
    GroupStateMismatch {
        group_id: GroupId,
        expected: GroupState,
        actual: GroupState,
    },
    #[error("run {run_id} is {actual}, not {expected}")]
    RunStatusMismatch {
        run_id: RunId,
        expected: RunStatus,
        actual: RunStatus,
    },
    #[error("queue is {actual}, not {expected}")]
    QueueStateMismatch {
        expected: QueueState,
        actual: QueueState,
    },
    #[error(transparent)]
    Item(#[from] TransitionError<ItemState>),
    #[error(transparent)]
    Group(#[from] TransitionError<GroupState>),
    #[error(transparent)]
    Run(#[from] TransitionError<RunStatus>),
    #[error(transparent)]
    Queue(#[from] TransitionError<QueueState>),
}

/// Applies `event` to `snapshot`.
///
/// # Errors
///
/// The event is for another queue, or a decision does not fit the snapshot:
/// unknown entity, state before that differs, or a transition that the
/// tables refuse. The snapshot is unchanged on error.
pub fn apply(snapshot: &mut Snapshot, event: &Event) -> Result<Outcome, ApplyError> {
    apply_seen(snapshot, event, &mut |_| {})
}

/// [`apply`], which shows to `decided` the snapshot after a decision and
/// before the eviction of the entities that it concluded.
pub(crate) fn apply_seen(
    snapshot: &mut Snapshot,
    event: &Event,
    decided: &mut dyn FnMut(&Snapshot),
) -> Result<Outcome, ApplyError> {
    if event.queue_id != snapshot.queue_id {
        return Err(ApplyError::WrongQueue {
            event_id: event.id,
            event_queue: event.queue_id,
        });
    }

    let mut next = snapshot.clone();
    let outcome = if event.kind.event_type().is_decision() {
        apply_decision(&mut next, event)?;
        if let Some(cause) = event.cause_id {
            next.requests.remove(&cause);
        }
        decided(&next);
        evict(&mut next);
        prune_requests(&mut next);
        Outcome::Applied
    } else {
        apply_fact(&mut next, event)
    };

    if outcome == Outcome::Applied {
        next.events_applied += 1;
        *snapshot = next;
    }
    Ok(outcome)
}

fn request(snapshot: &mut Snapshot, event: &Event, request: Request) -> Outcome {
    if snapshot.requests.values().any(|r| *r == request) {
        return Outcome::Ignored;
    }

    // A later submission of a PR carries the head and the settings that the
    // person wants now; a second cancellation or restart of an item adds
    // nothing to the first.
    let pending: Vec<EventId> = snapshot
        .requests
        .iter()
        .filter(|(_, r)| same_subject(r, &request))
        .map(|(id, _)| *id)
        .collect();
    if !pending.is_empty() && !matches!(request, Request::Submit { .. }) {
        return Outcome::Ignored;
    }

    for id in pending {
        snapshot.requests.remove(&id);
    }

    snapshot.requests.insert(event.id, request);
    if request_possible(snapshot, &snapshot.requests[&event.id]) {
        Outcome::Applied
    } else {
        snapshot.requests.remove(&event.id);
        Outcome::Ignored
    }
}

/// Whether two requests of a person act on the same PR or the same item.
fn same_subject(a: &Request, b: &Request) -> bool {
    match (a, b) {
        (Request::Submit { pr_number: x, .. }, Request::Submit { pr_number: y, .. }) => x == y,
        (Request::Cancel { item_id: x, .. }, Request::Cancel { item_id: y, .. })
        | (Request::Restart { item_id: x, .. }, Request::Restart { item_id: y, .. }) => x == y,
        _ => false,
    }
}

fn observe<T: PartialEq>(slot: Option<&mut Observed<T>>, value: &T) -> bool {
    slot.is_some_and(|o| o.value == *value)
}

#[allow(
    clippy::too_many_lines,
    reason = "one arm per fact, each short and independent"
)]
fn apply_fact(snapshot: &mut Snapshot, event: &Event) -> Outcome {
    match &event.kind {
        EventKind::ItemSubmitted {
            pr_number,
            head_sha,
            priority,
            no_batch,
            submitted_via,
            profile,
            ..
        } => request(
            snapshot,
            event,
            Request::Submit {
                pr_number: *pr_number,
                head_sha: head_sha.clone(),
                priority: *priority,
                no_batch: *no_batch,
                submitted_via: *submitted_via,
                profile: profile.clone(),
            },
        ),
        EventKind::ItemCancelRequested { item_id, by_admin } => request(
            snapshot,
            event,
            Request::Cancel {
                item_id: *item_id,
                by_admin: *by_admin,
            },
        ),
        EventKind::ItemRestartRequested { item_id, by_api } => request(
            snapshot,
            event,
            Request::Restart {
                item_id: *item_id,
                by_api: *by_api,
            },
        ),
        EventKind::PrObserved {
            pr_number,
            url,
            state,
            head_sha,
            base_branch,
            draft,
            labels,
            mergeability,
            review_decision,
            checks,
            unmet_conditions,
            required_checks,
        } => {
            if !tracks_pr(snapshot, *pr_number) {
                return Outcome::Ignored;
            }

            let value = PrObservation {
                url: url.clone(),
                state: *state,
                head_sha: head_sha.clone(),
                base_branch: base_branch.clone(),
                draft: *draft,
                labels: labels.clone(),
                mergeability: *mergeability,
                review_decision: *review_decision,
                checks: checks.clone(),
                unmet_conditions: unmet_conditions.clone(),
                required_checks: required_checks.clone(),
            };

            if observe(snapshot.prs.get_mut(pr_number), &value) {
                return Outcome::Ignored;
            }

            snapshot.prs.insert(
                *pr_number,
                Observed {
                    value,
                    fact_id: event.id,
                    observed_at: event.occurred_at,
                },
            );

            Outcome::Applied
        }
        EventKind::TargetPushed { before, after, .. } => {
            target_pushed(snapshot, event, before.as_deref(), after)
        }
        EventKind::CheckObserved { test_sha, check } => {
            let Some(run) = snapshot
                .runs
                .values()
                .find(|r| !r.status.is_terminal() && r.test_sha.as_deref() == Some(test_sha))
            else {
                return Outcome::Ignored;
            };

            let run_id = run.id;
            let required = run.required_statuses.contains(&check.name);
            let checks = snapshot.checks.entry(run_id).or_default();
            let key = (check.name.clone(), check.kind);

            let same = checks.get(&key).is_some_and(|o| {
                let seen = &o.value;
                seen.status == check.status
                    && seen.conclusion == check.conclusion
                    && seen.details_url == check.details_url
                    && seen.started_at == check.started_at
                    && seen.completed_at == check.completed_at
            });
            if same {
                return Outcome::Ignored;
            }

            let observation = CheckObservation {
                org_id: snapshot.org_id,
                run_id,
                name: check.name.clone(),
                kind: check.kind,
                required,
                status: check.status,
                conclusion: check.conclusion,
                details_url: check.details_url.clone(),
                started_at: check.started_at,
                completed_at: check.completed_at,
                observed_at: event.occurred_at,
            };

            checks.insert(
                key,
                Observed {
                    value: observation,
                    fact_id: event.id,
                    observed_at: event.occurred_at,
                },
            );

            Outcome::Applied
        }
        EventKind::TargetsReceived {
            pr_number,
            head_sha,
            targets,
        } => {
            if !tracks_pr(snapshot, *pr_number) {
                return Outcome::Ignored;
            }

            let value = TargetsUpload {
                head_sha: head_sha.clone(),
                targets: targets.clone(),
            };

            if observe(snapshot.targets.get_mut(pr_number), &value) {
                return Outcome::Ignored;
            }

            snapshot.targets.insert(
                *pr_number,
                Observed {
                    value,
                    fact_id: event.id,
                    observed_at: event.occurred_at,
                },
            );

            Outcome::Applied
        }
        EventKind::TestBranchAssembled {
            run_id,
            test_sha,
            draft_pr_number,
            required_statuses,
            required_statuses_source,
        } => {
            let outcome = request(snapshot, event, Request::Assembled { run_id: *run_id });
            if outcome == Outcome::Applied
                && let Some(run) = snapshot.runs.get_mut(run_id)
            {
                run.test_sha = Some(test_sha.clone());
                run.draft_pr_number = *draft_pr_number;
                run.required_statuses.clone_from(required_statuses);
                run.required_statuses_source = Some(*required_statuses_source);
            }
            outcome
        }
        EventKind::TestBranchAssemblyFailed {
            run_id,
            error,
            conflict_culprit,
        } => {
            let outcome = request(
                snapshot,
                event,
                Request::AssemblyFailed {
                    run_id: *run_id,
                    error: *error,
                },
            );
            if outcome == Outcome::Applied
                && let Some(run) = snapshot.runs.get_mut(run_id)
            {
                run.conflict_culprit.clone_from(conflict_culprit);
            }
            outcome
        }
        EventKind::MergeSucceeded {
            item_id,
            merged_sha,
        } => {
            let outcome = request(
                snapshot,
                event,
                Request::MergeSucceeded {
                    item_id: *item_id,
                    merged_sha: merged_sha.clone(),
                },
            );
            if outcome == Outcome::Applied {
                snapshot.target.own_shas.insert(merged_sha.clone());
            }
            outcome
        }
        EventKind::MergeFailed {
            item_id,
            direct: true,
            error: JobError::DirectMergeUnavailable,
        } => match snapshot.items.get_mut(item_id) {
            Some(item) if item.state == ItemState::Pending && !item.direct_merge_tried => {
                item.direct_merge_tried = true;
                Outcome::Applied
            }
            _ => Outcome::Ignored,
        },
        EventKind::MergeFailed {
            item_id,
            direct,
            error,
        } => request(
            snapshot,
            event,
            Request::MergeFailed {
                item_id: *item_id,
                direct: *direct,
                error: *error,
            },
        ),
        EventKind::QueueControlRequested { control } => {
            request(snapshot, event, Request::Control(*control))
        }
        EventKind::QueueSettingsChanged { after, .. } => {
            if snapshot.requested_mode == *after {
                return Outcome::Ignored;
            }

            snapshot.requested_mode = *after;
            Outcome::Applied
        }
        EventKind::AccessLost => request(snapshot, event, Request::AccessLost),
        EventKind::AccessRestored => request(snapshot, event, Request::AccessRestored),
        EventKind::ItemTransitioned { .. }
        | EventKind::ItemReadinessChanged { .. }
        | EventKind::QueueRenumbered { .. }
        | EventKind::GroupFormed { .. }
        | EventKind::GroupTransitioned { .. }
        | EventKind::RunTransitioned { .. }
        | EventKind::QueueTransitioned { .. }
        | EventKind::QueueWindowChanged { .. } => Outcome::Ignored,
    }
}

/// A PR is tracked while it has a live item or a pending submission.
fn tracks_pr(snapshot: &Snapshot, pr_number: u64) -> bool {
    snapshot.current_item(pr_number).is_some()
        || snapshot
            .requests
            .values()
            .any(|r| matches!(r, Request::Submit { pr_number: n, .. } if *n == pr_number))
}

/// A push that arrives late (its `after`, or its `before`, is a past head)
/// changes nothing: the tick reads the real head.
fn target_pushed(
    snapshot: &mut Snapshot,
    event: &Event,
    before: Option<&str>,
    after: &str,
) -> Outcome {
    let target = &mut snapshot.target;
    let head = target.head.as_ref().map(|h| h.value.as_str());
    let past = |sha: &str| target.recent_heads.iter().any(|h| h == sha);

    if head == Some(after) || past(after) || before.is_some_and(|b| Some(b) != head && past(b)) {
        return Outcome::Ignored;
    }

    let own = event.actor == Actor::Bot || target.own_shas.remove(after);

    if let Some(old) = target.head.take() {
        target.recent_heads.push_back(old.value);
        if target.recent_heads.len() > RECENT_HEADS {
            target.recent_heads.pop_front();
        }
    }

    target.head = Some(Observed {
        value: after.to_owned(),
        fact_id: event.id,
        observed_at: event.occurred_at,
    });

    if !own {
        target.valid_bases.clear();
    }

    target.valid_bases.insert(after.to_owned());
    let used: Vec<String> = snapshot.live_groups().map(|g| g.base_sha.clone()).collect();
    snapshot
        .target
        .valid_bases
        .retain(|b| b == after || used.contains(b));

    Outcome::Applied
}

#[allow(
    clippy::too_many_lines,
    reason = "one arm per decision, each short and independent"
)]
fn apply_decision(snapshot: &mut Snapshot, event: &Event) -> Result<(), ApplyError> {
    match &event.kind {
        EventKind::ItemTransitioned {
            item_id,
            from: None,
            to,
            reason,
            ..
        } => {
            check_transition(None, *to, *reason)?;

            let submission = event.cause_id.and_then(|c| snapshot.requests.get(&c));
            let Some(Request::Submit {
                pr_number,
                head_sha,
                priority,
                no_batch,
                submitted_via,
                profile,
            }) = submission.cloned()
            else {
                return Err(ApplyError::NoSubmission { event_id: event.id });
            };

            let is_current = !to.is_terminal();
            if is_current {
                for item in snapshot.items.values_mut() {
                    if item.pr_number == pr_number {
                        item.is_current = false;
                    }
                }
            }

            snapshot.items.insert(
                *item_id,
                Item {
                    id: *item_id,
                    org_id: snapshot.org_id,
                    queue_id: snapshot.queue_id,
                    pr_number,
                    head_sha,
                    state: *to,
                    state_reason: *reason,
                    priority,
                    no_batch,
                    profile,
                    submitted_via,
                    position: None,
                    is_current,
                    mergeability: Mergeability::Unknown,
                    block_reasons: Vec::new(),
                    group_id: None,
                    verified_by_run_id: None,
                    merged_sha: None,
                    submitted_at: event.occurred_at,
                    admitted_at: None,
                    direct_merge_tried: false,
                },
            );

            set_cause(snapshot, Subject::Item(*item_id), event);
        }
        EventKind::ItemTransitioned {
            item_id,
            from: Some(from),
            to,
            reason,
            position,
            verified_by_run_id,
            merged_sha,
        } => {
            let item = snapshot
                .items
                .get_mut(item_id)
                .ok_or(ApplyError::UnknownItem(*item_id))?;

            if item.state != *from {
                return Err(ApplyError::ItemStateMismatch {
                    item_id: *item_id,
                    expected: *from,
                    actual: item.state,
                });
            }

            check_transition(Some(*from), *to, *reason)?;

            if *from == ItemState::NotReady && *to == ItemState::Pending {
                item.admitted_at.get_or_insert(event.occurred_at);
            }
            item.state = *to;
            item.state_reason = *reason;
            if position.is_some() {
                item.position = *position;
            }
            if verified_by_run_id.is_some() {
                item.verified_by_run_id = *verified_by_run_id;
            }
            if merged_sha.is_some() {
                item.merged_sha.clone_from(merged_sha);
            }
            if *to == ItemState::Pending {
                item.group_id = None;
                item.verified_by_run_id = None;
            }

            set_cause(snapshot, Subject::Item(*item_id), event);
        }
        EventKind::ItemReadinessChanged {
            item_id,
            mergeability,
            block_reasons,
        } => {
            let item = snapshot
                .items
                .get_mut(item_id)
                .ok_or(ApplyError::UnknownItem(*item_id))?;

            item.mergeability = *mergeability;
            item.block_reasons.clone_from(block_reasons);

            set_cause(snapshot, Subject::Item(*item_id), event);
        }
        EventKind::QueueRenumbered { positions } => {
            if let Some(unknown) = positions.keys().find(|i| !snapshot.items.contains_key(i)) {
                return Err(ApplyError::UnknownItem(*unknown));
            }

            for (item_id, position) in positions {
                if let Some(item) = snapshot.items.get_mut(item_id) {
                    item.position = Some(*position);
                }
            }
        }
        EventKind::GroupFormed {
            group_id,
            kind,
            item_ids,
            predecessor_ids,
            base_sha,
            parent_group_id,
        } => {
            if snapshot.groups.contains_key(group_id) {
                return Err(ApplyError::GroupExists(*group_id));
            }

            if let Some(unknown) = item_ids
                .iter()
                .chain(predecessor_ids)
                .find(|i| !snapshot.items.contains_key(i))
            {
                return Err(ApplyError::UnknownItem(*unknown));
            }

            for item_id in item_ids {
                if let Some(item) = snapshot.items.get_mut(item_id) {
                    item.group_id = Some(*group_id);
                }
            }

            snapshot.groups.insert(
                *group_id,
                TestGroup {
                    id: *group_id,
                    org_id: snapshot.org_id,
                    queue_id: snapshot.queue_id,
                    kind: *kind,
                    state: GroupState::Testing,
                    state_reason: Reason::TestRunStarted,
                    item_ids: item_ids.clone(),
                    parent_group_id: *parent_group_id,
                    predecessor_ids: predecessor_ids.clone(),
                    base_sha: base_sha.clone(),
                },
            );

            set_cause(snapshot, Subject::Group(*group_id), event);
        }
        EventKind::GroupTransitioned {
            group_id,
            from,
            to,
            reason,
        } => {
            let group = snapshot
                .groups
                .get_mut(group_id)
                .ok_or(ApplyError::UnknownGroup(*group_id))?;

            if group.state != *from {
                return Err(ApplyError::GroupStateMismatch {
                    group_id: *group_id,
                    expected: *from,
                    actual: group.state,
                });
            }

            check_transition(Some(*from), *to, *reason)?;
            group.state = *to;
            group.state_reason = *reason;

            set_cause(snapshot, Subject::Group(*group_id), event);
        }
        EventKind::RunTransitioned {
            run_id,
            group_id,
            from,
            to,
            reason,
            attempt,
            test_branch,
            failure_causing_statuses,
        } => {
            check_transition(*from, *to, *reason)?;

            match from {
                None => {
                    if snapshot.runs.contains_key(run_id) {
                        return Err(ApplyError::RunExists(*run_id));
                    }

                    if !snapshot.groups.contains_key(group_id) {
                        return Err(ApplyError::UnknownGroup(*group_id));
                    }

                    snapshot.runs.insert(
                        *run_id,
                        TestRun {
                            id: *run_id,
                            org_id: snapshot.org_id,
                            queue_id: snapshot.queue_id,
                            group_id: *group_id,
                            attempt: *attempt,
                            test_branch: test_branch.clone(),
                            test_sha: None,
                            draft_pr_number: None,
                            status: *to,
                            required_statuses: Vec::new(),
                            required_statuses_source: None,
                            started_at: event.occurred_at,
                            ended_at: None,
                            failure_reason: None,
                            failure_causing_statuses: Vec::new(),
                            conflict_culprit: None,
                        },
                    );
                }
                Some(from) => {
                    let run = snapshot
                        .runs
                        .get_mut(run_id)
                        .ok_or(ApplyError::UnknownRun(*run_id))?;

                    if run.status != *from {
                        return Err(ApplyError::RunStatusMismatch {
                            run_id: *run_id,
                            expected: *from,
                            actual: run.status,
                        });
                    }

                    run.status = *to;
                    if to.is_terminal() {
                        run.ended_at = Some(event.occurred_at);
                    }
                    if matches!(to, RunStatus::Failed | RunStatus::TimedOut) {
                        run.failure_reason = Some(*reason);
                        run.failure_causing_statuses
                            .clone_from(failure_causing_statuses);
                    }
                }
            }

            set_cause(snapshot, Subject::Run(*run_id), event);
        }
        EventKind::QueueTransitioned { from, to, reason } => {
            if snapshot.state != *from {
                return Err(ApplyError::QueueStateMismatch {
                    expected: *from,
                    actual: snapshot.state,
                });
            }

            check_transition(Some(*from), *to, *reason)?;

            snapshot.state = *to;
            if *reason == Reason::ModeSwitchCompleted {
                snapshot.mode = snapshot.requested_mode;
            }
        }
        EventKind::QueueWindowChanged { after, .. } => snapshot.window = Some(*after),
        _ => {}
    }
    Ok(())
}

fn set_cause(snapshot: &mut Snapshot, subject: Subject, event: &Event) {
    match event.cause_id {
        Some(cause) => snapshot.causes.insert(subject, cause),
        None => snapshot.causes.remove(&subject),
    };
}

/// Whether a decision can still answer the request.
fn request_possible(snapshot: &Snapshot, request: &Request) -> bool {
    let item_in = |item_id: &ItemId, states: &[ItemState]| {
        snapshot
            .items
            .get(item_id)
            .is_some_and(|i| states.contains(&i.state))
    };
    let run_assembling = |run_id: &RunId| {
        snapshot
            .runs
            .get(run_id)
            .is_some_and(|r| r.status == RunStatus::Assembling)
    };

    match request {
        Request::Submit {
            pr_number,
            head_sha,
            ..
        } => snapshot
            .current_item(*pr_number)
            .is_none_or(|item| item.head_sha != *head_sha),
        Request::Cancel { item_id, .. } => snapshot
            .items
            .get(item_id)
            .is_some_and(|i| !i.state.is_terminal()),
        Request::Restart { item_id, .. } => {
            item_in(item_id, &[ItemState::Testing, ItemState::PendingFailure])
        }
        Request::Assembled { run_id } | Request::AssemblyFailed { run_id, .. } => {
            run_assembling(run_id)
        }
        Request::MergeSucceeded { item_id, .. } => {
            item_in(item_id, &[ItemState::TestsPassed, ItemState::Pending])
        }
        Request::MergeFailed {
            item_id, direct, ..
        } => item_in(
            item_id,
            &[if *direct {
                ItemState::Pending
            } else {
                ItemState::TestsPassed
            }],
        ),
        Request::Control(control) => check_transition(
            Some(snapshot.state),
            control.target(),
            Reason::AdminRequested,
        )
        .is_ok(),
        Request::AccessLost => snapshot.state != QueueState::Paused,
        Request::AccessRestored => snapshot.state == QueueState::Paused,
    }
}

fn prune_requests(snapshot: &mut Snapshot) {
    let impossible: Vec<EventId> = snapshot
        .requests
        .iter()
        .filter(|(_, r)| !request_possible(snapshot, r))
        .map(|(id, _)| *id)
        .collect();

    for id in impossible {
        snapshot.requests.remove(&id);
    }
}

/// Removes the concluded entities that no live group cites.
fn evict(snapshot: &mut Snapshot) {
    let finished_groups: Vec<GroupId> = snapshot
        .groups
        .values()
        .filter(|g| {
            g.state.is_terminal()
                && snapshot.live_run(g.id).is_none()
                && !snapshot.items.values().any(|i| {
                    !i.state.is_terminal()
                        && i.verified_by_run_id
                            .and_then(|r| snapshot.runs.get(&r))
                            .is_some_and(|r| r.group_id == g.id)
                })
                && g.parent_group_id
                    .and_then(|p| snapshot.groups.get(&p))
                    .is_none_or(|p| p.state.is_terminal())
                && !snapshot
                    .items
                    .values()
                    .any(|i| i.group_id == Some(g.id) && !i.state.is_terminal())
        })
        .map(|g| g.id)
        .collect();

    for group_id in finished_groups {
        snapshot.groups.remove(&group_id);
        snapshot.causes.remove(&Subject::Group(group_id));

        let runs: Vec<RunId> = snapshot
            .runs
            .values()
            .filter(|r| r.group_id == group_id)
            .map(|r| r.id)
            .collect();

        for run_id in runs {
            snapshot.runs.remove(&run_id);
            snapshot.checks.remove(&run_id);
            snapshot.causes.remove(&Subject::Run(run_id));
        }
    }

    let cited: Vec<ItemId> = snapshot
        .groups
        .values()
        .flat_map(|g| g.item_ids.iter().chain(&g.predecessor_ids))
        .copied()
        .collect();
    let finished_items: Vec<ItemId> = snapshot
        .items
        .values()
        .filter(|i| i.state.is_terminal() && !cited.contains(&i.id))
        .map(|i| i.id)
        .collect();

    for item_id in finished_items {
        snapshot.items.remove(&item_id);
        snapshot.causes.remove(&Subject::Item(item_id));
    }

    let tracked: Vec<u64> = snapshot
        .prs
        .keys()
        .chain(snapshot.targets.keys())
        .copied()
        .filter(|pr| !tracks_pr(snapshot, *pr))
        .collect();

    for pr in tracked {
        snapshot.prs.remove(&pr);
        snapshot.targets.remove(&pr);
    }
}

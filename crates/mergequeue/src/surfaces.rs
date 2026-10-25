//! What each PR shows of its item: the status of the queue on its head, the
//! comment of the queue and the state labels. The reconciliation renders the
//! surfaces of `plan` into `pr_surfaces`; the job `publish_surfaces` makes
//! the SCM show the last rendering. The words are in `wording`; this module
//! reads, writes and publishes them.

use std::collections::BTreeMap;

use cihub_config::QueueSettings;
use cihub_domain::{EventType, JobClass, RepoId};
use cihub_mq_core::Surface;
use cihub_platform::Job;
use cihub_scm::{Scm, ScmError, StatusUpdate};
use cihub_store::{PgPool, begin_system, event_kind};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgConnection;
use time::OffsetDateTime;
use tracing::info;

use crate::MergeQueueError;
use crate::jobs::{Failure, job, queue_status_context, read_payload, target_of};
use crate::target::QueueTarget;
use crate::wording::{HistoryEvent, Rendered, StatusState, hash, path_segment, render, welcome};

/// The type of the job that publishes the surfaces of a PR.
pub(crate) const PUBLISH_SURFACES: &str = "publish_surfaces";

/// The events of its items that the history of a PR shows: the decisions. A
/// request of a user is not a row: the decision that answers it names the
/// user. The history also shows the test branches of the runs of the PR.
const HISTORY_EVENTS: [EventType; 2] =
    [EventType::ItemTransitioned, EventType::ItemReadinessChanged];

/// The status of the queue on `sha`.
fn queue_status(
    target_branch: &str,
    sha: &str,
    state: StatusState,
    description: &str,
    details_url: Option<&str>,
) -> StatusUpdate {
    StatusUpdate {
        sha: sha.to_owned(),
        context: queue_status_context(target_branch),
        status: state.into(),
        description: description.to_owned(),
        details_url: details_url.map(str::to_owned),
    }
}

/// The page of the queue of `target` in the dashboard at `dashboard`, as the
/// routes of the dashboard name it.
async fn queue_page(
    conn: &mut PgConnection,
    target: &QueueTarget,
    dashboard: &str,
) -> Result<String, MergeQueueError> {
    let slug: String = sqlx::query_scalar("SELECT slug FROM orgs WHERE id = $1")
        .bind(target.org_id.to_string())
        .fetch_one(conn)
        .await?;

    Ok(format!(
        "{}/orgs/{}/repos/{}/queues/{}",
        dashboard.trim_end_matches('/'),
        path_segment(&slug),
        path_segment(&target.repo.name),
        path_segment(&target.target_branch)
    ))
}

/// Writes the rendering of each surface that changed in `pr_surfaces`, and
/// returns the publication jobs of every PR of the queue whose SCM does not
/// show its last rendering.
///
/// # Errors
///
/// Postgres fails.
pub(crate) async fn converge_surfaces(
    conn: &mut PgConnection,
    target: &QueueTarget,
    settings: &QueueSettings,
    surfaces: &[Surface],
    dashboard_url: Option<&str>,
) -> Result<Vec<Job>, MergeQueueError> {
    let numbers: Vec<i64> = surfaces
        .iter()
        .map(|surface| i64::try_from(surface.pr_number).unwrap_or(i64::MAX))
        .collect();
    // Without a comment, the history has no use.
    let histories = if settings.comments_enabled.value {
        read_histories(conn, target, &numbers).await?
    } else {
        BTreeMap::new()
    };
    // The surfaces of these PRs only: the queue keeps the surfaces of every PR
    // that it ever saw.
    let known: Vec<(i64, String)> = sqlx::query_as(
        "SELECT pr_number, desired_hash FROM pr_surfaces
         WHERE queue_id = $1 AND pr_number = ANY($2)",
    )
    .bind(target.queue_id.to_string())
    .bind(&numbers)
    .fetch_all(&mut *conn)
    .await?;

    let page_of_queue = match dashboard_url {
        Some(dashboard_url) => Some(queue_page(conn, target, dashboard_url).await?),
        None => None,
    };

    for (surface, number) in surfaces.iter().zip(numbers) {
        let history = histories.get(&number).map_or(&[][..], Vec::as_slice);
        let page = page_of_queue
            .as_ref()
            .map(|queue| format!("{queue}/prs/{number}"));
        let rendered = render(
            surface,
            settings,
            &target.target_branch,
            history,
            page.as_deref(),
        );
        let desired_hash = hash(&rendered);
        if known
            .iter()
            .any(|(n, h)| *n == number && *h == desired_hash)
        {
            continue;
        }
        let desired = serde_json::to_value(&rendered).map_err(MergeQueueError::Snapshot)?;
        sqlx::query(
            "INSERT INTO pr_surfaces (org_id, queue_id, pr_number, desired, desired_hash)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (queue_id, pr_number) DO UPDATE
             SET desired = EXCLUDED.desired, desired_hash = EXCLUDED.desired_hash",
        )
        .bind(target.org_id.to_string())
        .bind(target.queue_id.to_string())
        .bind(number)
        .bind(desired)
        .bind(&desired_hash)
        .execute(&mut *conn)
        .await?;
    }

    let unpublished: Vec<(i64, String)> = sqlx::query_as(
        "SELECT pr_number, desired_hash FROM pr_surfaces
         WHERE queue_id = $1 AND published_hash IS DISTINCT FROM desired_hash
         ORDER BY pr_number",
    )
    .bind(target.queue_id.to_string())
    .fetch_all(&mut *conn)
    .await?;

    Ok(unpublished
        .into_iter()
        .map(|(number, hash)| publish_job(target, number, &hash))
        .collect())
}

/// An event of the history, as `read_histories` selects it: its PR, id, type,
/// payload and time, the login of the user whose request caused it, and the
/// PRs tested with it and before it.
type HistoryRow = (
    i64,
    String,
    String,
    Value,
    OffsetDateTime,
    Option<String>,
    Vec<i64>,
    Vec<i64>,
);

/// The events of the history of the PRs `numbers` of the queue, by PR, in the
/// order of the log. All the items of a PR count, not only the current one.
/// The reading starts from these items: their events come through
/// `events.item_id`. A decision has the system as actor; the user is the
/// actor of its cause. The test branches come through the runs that tested
/// the PR, which started after its first submission: the bound reads the
/// recent runs of the queue rather than all of them. The two times come from
/// the clocks of two replicas, so the bound keeps an hour of margin; the
/// leases of the queues already need these clocks within seconds.
async fn read_histories(
    conn: &mut PgConnection,
    target: &QueueTarget,
    numbers: &[i64],
) -> Result<BTreeMap<i64, Vec<HistoryEvent>>, MergeQueueError> {
    let types: Vec<&str> = HISTORY_EVENTS.into_iter().map(EventType::as_str).collect();
    let rows: Vec<HistoryRow> = sqlx::query_as(
        "SELECT pr_number, id, type, payload, occurred_at, login, tested_with, tested_on_top_of
         FROM (
         SELECT i.pr_number, e.id, e.type::text AS type, e.payload, e.occurred_at, u.login,
                '{}'::bigint[] AS tested_with, '{}'::bigint[] AS tested_on_top_of, e.seq
         FROM queue_items i
         JOIN events e ON e.item_id = i.id AND e.type = ANY($2::event_type[])
         LEFT JOIN events c ON c.id = e.cause_id
         LEFT JOIN users u ON u.id = c.actor_user_id
         WHERE i.queue_id = $1 AND i.pr_number = ANY($3)
         UNION ALL
         SELECT n.pr_number, e.id, e.type::text, e.payload, e.occurred_at, NULL,
                array_remove(r.tested_pr_numbers, n.pr_number),
                ARRAY(SELECT p.pr_number
                      FROM unnest(g.predecessor_ids) WITH ORDINALITY AS a (id, rank)
                      JOIN queue_items p ON p.id = a.id
                      ORDER BY a.rank),
                e.seq
         FROM test_runs r
         CROSS JOIN LATERAL unnest(r.tested_pr_numbers) AS n (pr_number)
         JOIN test_groups g ON g.id = r.group_id
         JOIN events e ON e.run_id = r.id AND e.type = 'test_branch.assembled'
         WHERE r.queue_id = $1
           AND r.started_at >= (SELECT min(f.submitted_at) FROM queue_items f
                                WHERE f.queue_id = $1 AND f.pr_number = ANY($3))
                               - interval '1 hour'
           AND r.tested_pr_numbers && $3 AND n.pr_number = ANY($3)
         ) history
         ORDER BY seq",
    )
    .bind(target.queue_id.to_string())
    .bind(&types)
    .bind(numbers)
    .fetch_all(conn)
    .await?;
    let mut histories: BTreeMap<i64, Vec<HistoryEvent>> = BTreeMap::new();

    for (number, id, kind, payload, occurred_at, login, tested_with, tested_on_top_of) in rows {
        let kind = event_kind(&kind, &payload)
            .map_err(|source| MergeQueueError::ReadEvent { id, source })?;
        histories.entry(number).or_default().push(HistoryEvent {
            occurred_at,
            kind,
            login,
            tested_with,
            tested_on_top_of,
        });
    }

    Ok(histories)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct PublishSurfaces {
    pr_number: i64,
}

fn publish_job(target: &QueueTarget, pr_number: i64, hash: &str) -> Job {
    let key = format!(
        "{PUBLISH_SURFACES}:{}-{pr_number}-{hash}:1",
        target.queue_id
    );
    let payload = json!(PublishSurfaces { pr_number });
    job(
        target,
        key,
        PUBLISH_SURFACES,
        JobClass::Interactive,
        payload,
    )
}

/// The rendering that a PR must show, what it shows, and its comment.
type SurfaceRow = (serde_json::Value, String, Option<String>, Option<String>);

/// Makes the SCM show the last rendering of the PR. One job at a time
/// publishes for a PR, so that an older rendering never lands after a newer
/// one: the job takes a lock of the PR for its whole run.
pub(crate) async fn publish(
    pool: &PgPool,
    scm: &dyn Scm,
    job: &Job,
    _now: OffsetDateTime,
) -> Result<(), Failure> {
    let request: PublishSurfaces = read_payload(job)?;
    let target = target_of(pool, job).await?;
    let mut lock = begin_system(pool).await?;

    lock_pr(&mut lock, &comment_lock(target.repo_id), request.pr_number).await?;

    let row: Option<SurfaceRow> = sqlx::query_as(
        "SELECT s.desired, s.desired_hash, s.published_hash, c.comment_external_id
         FROM pr_surfaces s
         LEFT JOIN pr_comments c ON c.repo_id = $3 AND c.pr_number = s.pr_number
         WHERE s.queue_id = $1 AND s.pr_number = $2",
    )
    .bind(target.queue_id.to_string())
    .bind(request.pr_number)
    .bind(target.repo_id.to_string())
    .fetch_optional(&mut *lock)
    .await?;

    let Some((desired, desired_hash, published, comment_id)) = row else {
        return Ok(());
    };

    if published.as_deref() == Some(desired_hash.as_str()) {
        return Ok(());
    }

    let rendered: Rendered = serde_json::from_value(desired)
        .map_err(|err| Failure::Never(MergeQueueError::Payload(err)))?;
    let number = u64::try_from(request.pr_number).unwrap_or_default();
    let repo = &target.repo;
    let class = job.class;

    if let Some(status) = &rendered.status {
        let update = queue_status(
            &target.target_branch,
            &rendered.sha,
            status.state,
            &status.description,
            status.details_url.as_deref(),
        );
        scm.set_status(repo, &update, class).await?;
    }

    for label in &rendered.add_labels {
        scm.add_label(repo, number, label, class).await?;
    }

    for label in &rendered.remove_labels {
        scm.remove_label(repo, number, label, class).await?;
    }

    // The comment comes last, just before the commit that keeps its
    // identifier: a stop between them gives a second comment.
    if let Some(body) = &rendered.comment {
        let id = publish_comment(scm, &target, number, body, comment_id, class).await?;
        keep_comment(&mut lock, &target, request.pr_number, &id).await?;
    }

    sqlx::query(
        "UPDATE pr_surfaces SET published_hash = $3 WHERE queue_id = $1 AND pr_number = $2",
    )
    .bind(target.queue_id.to_string())
    .bind(request.pr_number)
    .bind(&desired_hash)
    .execute(&mut *lock)
    .await?;

    lock.commit().await?;

    info!(
        pr_number = number,
        hash = desired_hash,
        "surfaces published"
    );

    Ok(())
}

/// Takes the lock `scope` of the PR `pr_number` for the transaction: one job
/// at a time writes what `scope` names on a PR.
pub(crate) async fn lock_pr(
    conn: &mut PgConnection,
    scope: &str,
    pr_number: i64,
) -> Result<(), Failure> {
    let locked: bool =
        sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtext($1), $2::integer)")
            .bind(scope)
            .bind(pr_number)
            .fetch_one(conn)
            .await?;
    if locked {
        Ok(())
    } else {
        Err(Failure::retry(format!(
            "another job holds the lock `{scope}` of this PR"
        )))
    }
}

/// The lock of the comment of a PR of `repo`, whatever its queue.
fn comment_lock(repo: RepoId) -> String {
    format!("pr_comments:{repo}")
}

/// Keeps `id` as the comment of the PR.
async fn keep_comment(
    conn: &mut PgConnection,
    target: &QueueTarget,
    pr_number: i64,
    id: &str,
) -> Result<(), Failure> {
    sqlx::query(
        "INSERT INTO pr_comments (org_id, repo_id, pr_number, comment_external_id)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (repo_id, pr_number) DO UPDATE
         SET comment_external_id = EXCLUDED.comment_external_id",
    )
    .bind(target.org_id.to_string())
    .bind(target.repo_id.to_string())
    .bind(pr_number)
    .bind(id)
    .execute(conn)
    .await?;
    Ok(())
}

/// Updates the comment of the queue, or creates it, and returns its
/// identifier.
async fn publish_comment(
    scm: &dyn Scm,
    target: &QueueTarget,
    number: u64,
    body: &str,
    known: Option<String>,
    class: JobClass,
) -> Result<String, Failure> {
    let repo = &target.repo;
    if let Some(id) = known {
        match scm.update_comment(repo, &id, body, class).await {
            Ok(()) => return Ok(id),
            // Someone deleted the comment: the queue writes a new one.
            Err(ScmError::NotFound(_)) => {}
            Err(err) => return Err(err.into()),
        }
    }
    Ok(scm.create_comment(repo, number, body, class).await?)
}

/// The status `success` before the merge of the PR: a rule that requires
/// the status of the queue lets the merge through.
pub(crate) fn merging_status(target_branch: &str, sha: &str) -> StatusUpdate {
    // The merge job has no page: the status has no link while the PR merges,
    // until the rendering of `merged` gives it back.
    queue_status(
        target_branch,
        sha,
        StatusState::Success,
        "Tests passed, merging",
        None,
    )
}

/// The type of the job that posts the welcome comment of a PR.
pub(crate) const WELCOME_PR: &str = "welcome_pr";

/// The payload of `welcome_pr`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct WelcomePr {
    pub repo_id: RepoId,
    pub pr_number: u64,
    pub base_branch: String,
}

/// Whether CIHub has a comment on the PR `pr_number` of `repo_id`.
pub(crate) async fn has_comment(
    conn: &mut PgConnection,
    repo_id: RepoId,
    pr_number: u64,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pr_comments WHERE repo_id = $1 AND pr_number = $2)",
    )
    .bind(repo_id.to_string())
    .bind(i64::try_from(pr_number).unwrap_or(i64::MAX))
    .fetch_one(conn)
    .await
}

/// Posts the welcome comment of the PR, once: a PR that has a comment of
/// CIHub already keeps it. The queue of the branch decides whether the
/// comment and the commands are on.
pub(crate) async fn post_welcome(
    pool: &PgPool,
    scm: &dyn Scm,
    job: &Job,
    _now: OffsetDateTime,
) -> Result<(), Failure> {
    let request: WelcomePr = read_payload(job)?;
    let mut lock = begin_system(pool).await?;
    let queue = QueueTarget::of_branch(&mut lock, request.repo_id, &request.base_branch).await?;

    let Some(target) = queue else {
        return Ok(());
    };

    let settings = target.settings()?;
    if !(settings.comments_enabled.value && settings.commands_enabled.value) {
        return Ok(());
    }

    let number = i64::try_from(request.pr_number).unwrap_or(i64::MAX);
    lock_pr(&mut lock, &comment_lock(target.repo_id), number).await?;

    if has_comment(&mut lock, target.repo_id, request.pr_number).await? {
        return Ok(());
    }

    let body = welcome(&target.target_branch);
    let id = scm
        .create_comment(&target.repo, request.pr_number, &body, job.class)
        .await?;

    keep_comment(&mut lock, &target, number, &id).await?;
    lock.commit().await?;
    info!(pr_number = request.pr_number, "welcome comment posted");

    Ok(())
}

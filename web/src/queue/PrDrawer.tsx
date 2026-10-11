import { useSuspenseQueries, useSuspenseQuery } from "@tanstack/react-query";
import { Suspense, useEffect, useState } from "react";

import { getPrOptions, getRunOptions, listEventsOptions } from "../client/@tanstack/react-query.gen";
import type { Pr, Event } from "../client/types.gen";
import { onQueueChanged } from "../live";
import { ItemStateBadge, itemStateLabel } from "../ui/StateBadge";
import { PrActions } from "./Actions";
import { DrawerFrame } from "./DrawerFrame";
import styles from "./PrDrawer.module.css";
import { Prose } from "./Prose";
import type { QueuePath } from "./reads";

const STEPS = ["Submitted", "Queued", "Testing", "Merged"];

const STEP_OF: Record<string, number> = {
  not_ready: 0,
  pending: 1,
  testing: 2,
  pending_failure: 2,
  tests_passed: 2,
  merged: 3,
  failed: 2,
  cancelled: 1,
};

const TONE_OF: Record<string, string> = {
  pending_failure: styles.amber,
  failed: styles.red,
  cancelled: styles.muted,
};

const RUN_STATUSES: Record<string, string> = {
  assembling: "Assembling",
  in_progress: "Testing",
  succeeded: "Passed",
  failed: "Failed",
  interrupted: "Interrupted",
  cancelled: "Cancelled",
  timed_out: "Timed out",
};

const TERMINAL = ["merged", "failed", "cancelled"];

const CONCLUSION_LABELS: Record<string, string> = {
  success: "passed",
  failure: "failed",
  timed_out: "timed out",
  action_required: "action required",
  neutral: "neutral",
  skipped: "skipped",
  cancelled: "cancelled",
  stale: "stale",
};

const CONCLUSIONS: Record<string, string> = {
  success: styles.success,
  failure: styles.failure,
  timed_out: styles.failure,
  action_required: styles.failure,
  neutral: styles.neutral,
  skipped: styles.neutral,
  cancelled: styles.neutral,
  stale: styles.neutral,
};

function timeOf(at: string): string {
  return new Date(at).toLocaleString("en", { dateStyle: "medium", timeStyle: "short" });
}

/** The name of an event of the history: the state that a transition reaches, else what happened. */
function whatOf(event: Event): string {
  switch (event.type) {
    case "item.transitioned":
      return itemStateLabel(String(event.payload.to));
    case "item.readiness_changed":
      return "Readiness";
    case "item.cancel_requested":
      return "Cancellation asked";
    case "item.restart_requested":
      return "New test asked";
    case "merge.succeeded":
      return "Merge";
    case "merge.failed":
      return "Merge failed";
    default:
      return "Event";
  }
}

/** Submitted, queued, testing, merged: where the PR is in its life. */
function Journey({ item }: { item: Pr }) {
  const step = STEP_OF[item.state] ?? 0;

  return (
    <div className={`${styles.journey} ${TONE_OF[item.state] ?? ""}`}>
      <ol>
        {STEPS.map((label, index) => (
          <li
            key={label}
            className={index < step ? styles.done : index === step ? styles.now : ""}
            aria-current={index === step ? "step" : undefined}
          >
            <span className={styles.dot} />
            {index === step ? itemStateLabel(item.state) : label}
          </li>
        ))}
      </ol>
    </div>
  );
}

function RunSection({ org, runId }: { org: string; runId: string }) {
  const { data: run } = useSuspenseQuery(getRunOptions({ path: { org, run_id: runId } }));
  const prs = (numbers: number[]) => numbers.map((number) => `#${String(number)}`).join(", ");

  return (
    <>
      <section aria-labelledby="run-title">
        <h3 id="run-title">
          Run · attempt {run.attempt} · {RUN_STATUSES[run.status] ?? run.status}
        </h3>
        <dl className={styles.facts}>
          <dt>Test branch</dt>
          <dd>
            <code>{run.test_branch}</code>
          </dd>
          {run.draft_pr_number != null && (
            <>
              <dt>Draft PR</dt>
              <dd>#{run.draft_pr_number} · runs your CI</dd>
            </>
          )}
          <dt>Base</dt>
          <dd>
            <code>{run.base_sha.slice(0, 7)}</code>
          </dd>
          <dt>Tests</dt>
          <dd>{prs(run.tested_pr_numbers)}</dd>
          <dt>On top of</dt>
          <dd>{run.predecessor_pr_numbers.length > 0 ? prs(run.predecessor_pr_numbers) : "nothing"}</dd>
          <dt>Started</dt>
          <dd>{timeOf(run.started_at)}</dd>
        </dl>
      </section>
      <section aria-labelledby="checks-title">
        <h3 id="checks-title">Checks</h3>
        {run.checks.length === 0 ? (
          <p className={styles.note}>No check reported on the test commit yet.</p>
        ) : (
          <ul className={styles.checks}>
            {run.checks.map((check) => (
              <li key={`${check.kind}-${check.name}`}>
                <span className={`${styles.conclusion} ${CONCLUSIONS[check.conclusion ?? ""] ?? ""}`} />
                <span className={styles.checkName}>{check.name}</span>
                {!check.required && <span className={styles.note}>not required</span>}
                <span className={styles.checkState}>
                  {check.conclusion ? (CONCLUSION_LABELS[check.conclusion] ?? check.conclusion) : "running"}
                </span>
              </li>
            ))}
          </ul>
        )}
      </section>
    </>
  );
}

/** The reads of the drawer of the PR `number`: the PR and its first 100 events. */
export function prReads(path: QueuePath, number: number) {
  const pr = { org: path.org, repo: path.repo, number };
  return [getPrOptions({ path: pr }), listEventsOptions({ path: pr, query: { limit: 100 } })] as const;
}

/**
 * The drawer of a PR over the page of its queue: where it is, why it waits or how it ended, its run and its
 * history. `Escape` closes it.
 */
export function PrDrawer({ path, number }: { path: QueuePath; number: number }) {
  const [{ data: item }, { data: events }] = useSuspenseQueries({
    queries: prReads(path, number),
  });
  const timeline = events.events;
  const ahead = item.ahead.length;
  const lastWait = timeline.findLast((event) => event.type === "item.readiness_changed");
  // A request is sent, not done: the queue decides at its next reconciliation.
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(
    () =>
      onQueueChanged((queueId) => {
        if (queueId === item.queue_id) {
          setNotice(null);
        }
      }),
    [item.queue_id],
  );

  return (
    <DrawerFrame
      path={path}
      title={item.title ?? `#${String(item.number)}`}
      meta={
        <>
          #{item.number} · {item.author}
          {!TERMINAL.includes(item.state) && item.position != null && ` · position ${String(ahead + 1)}`}
        </>
      }
      head={
        <>
          <div className={styles.status}>
            <ItemStateBadge state={item.state} />
            {!TERMINAL.includes(item.state) && <PrActions path={path} item={item} onSent={setNotice} />}
          </div>
          {notice && (
            <p className={styles.notice} role="status">
              {notice}
            </p>
          )}
        </>
      }
    >
      <section aria-label="Where it is">
        <Journey item={item} />
        {!TERMINAL.includes(item.state) && item.state !== "not_ready" && (
          <p className={styles.note}>
            {ahead === 0
              ? "Nothing merges before it"
              : `${String(ahead)} pull request${ahead > 1 ? "s" : ""} merge before it`}
          </p>
        )}
      </section>
      {item.state === "not_ready" && (
        <section aria-labelledby="wait-title">
          <h3 id="wait-title">Why it waits</h3>
          <p>
            <Prose text={lastWait?.description ?? item.state_description} />
          </p>
          <p className={styles.note}>It enters the queue by itself once nothing blocks it.</p>
        </section>
      )}
      {TERMINAL.includes(item.state) && (
        <section aria-labelledby="outcome-title">
          <h3 id="outcome-title">Outcome</h3>
          <p>{item.state_description}</p>
        </section>
      )}
      {item.run_id && (
        <Suspense fallback={<p className={styles.note}>Reading the run…</p>}>
          <RunSection org={path.org} runId={item.run_id} />
        </Suspense>
      )}
      <section aria-labelledby="history-title">
        <h3 id="history-title">History</h3>
        <ol className={styles.events}>
          {timeline.map((event) => (
            <li key={event.id}>
              <div className={styles.when}>{timeOf(event.occurred_at)}</div>
              <div className={styles.what}>{whatOf(event)}</div>
              <div className={styles.why}>
                <Prose text={event.description} />
                {event.actor && <span className={styles.actor}> · by @{event.actor}</span>}
              </div>
            </li>
          ))}
        </ol>
      </section>
    </DrawerFrame>
  );
}

import { Link } from "@tanstack/react-router";
import { useRef, useState } from "react";

import type { OrgQueue } from "../client/types.gen";
import { timeAgo, useNow } from "../time";
import { QueueStateBadge } from "../ui/StateBadge";
import { useFlip } from "../ui/motion";
import ui from "../ui/ui.module.css";
import styles from "./QueueCards.module.css";

/** From this many queues, a filter helps to find one. */
const FILTER_FROM = 8;

/** A queue that someone must look at: it does not run, or a PR in it failed its tests. */
function needsAttention({ queue, counts }: OrgQueue): boolean {
  return (
    queue.access === "suspended" ||
    Boolean(queue.config_error) ||
    queue.state !== "running" ||
    counts.pending_failure > 0
  );
}

/** The cards of `queues`, the ones that need someone first, then in the order of the API. */
export function QueueCards({ org, queues }: { org: string; queues: OrgQueue[] }) {
  const [filter, setFilter] = useState("");
  const hasFilter = queues.length >= FILTER_FROM;
  const words = hasFilter ? filter.trim().toLowerCase() : "";
  const shown = queues
    .filter(({ repo, queue }) => `${repo} ${queue.name} ${queue.target_branch}`.toLowerCase().includes(words))
    .sort((a, b) => Number(needsAttention(b)) - Number(needsAttention(a)));
  const now = useNow();
  const grid = useRef<HTMLDivElement>(null);
  useFlip(grid);

  return (
    <>
      {hasFilter && (
        <input
          className={styles.filter}
          type="search"
          aria-label="Filter the queues"
          placeholder="Filter by repository or branch"
          value={filter}
          onChange={(event) => {
            setFilter(event.target.value);
          }}
        />
      )}
      {shown.length > 0 ? (
        <div ref={grid} className={styles.grid}>
          {shown.map((entry) => (
            <QueueCard key={entry.queue.id} org={org} entry={entry} now={now} />
          ))}
        </div>
      ) : (
        <p className={`${ui.card} ${styles.none}`}>No queue matches “{filter.trim()}”.</p>
      )}
    </>
  );
}

function QueueCard({ org, entry, now }: { org: string; entry: OrgQueue; now: number }) {
  const { repo, queue, counts, merged_last_24h, last_merged_at } = entry;
  const inQueue =
    counts.not_ready + counts.pending + counts.testing + counts.pending_failure + counts.tests_passed;
  const testing = counts.testing + counts.pending_failure + counts.tests_passed;

  return (
    <Link
      data-flip={queue.id}
      className={`${ui.card} ${styles.card}`}
      to="/orgs/$org/repos/$repo/queues/$queue"
      params={{ org, repo, queue: queue.name }}
    >
      <div className={styles.top}>
        <div>
          <div className={styles.repo}>
            {org}/{repo}
          </div>
          <h2 className={styles.name}>{queue.name}</h2>
        </div>
        <QueueStateBadge queue={queue} />
      </div>
      <div className={styles.numbers}>
        <div>
          <b>{inQueue}</b>
          <span>in the queue</span>
        </div>
        <div>
          <b>{testing}</b>
          <span>testing</span>
        </div>
        <div>
          <b>{merged_last_24h}</b>
          <span>merged in 24 h</span>
        </div>
      </div>
      <div className={styles.foot}>
        {counts.pending_failure > 0 && (
          <>
            <span className={styles.failing}>{counts.pending_failure} with failed tests</span> ·{" "}
          </>
        )}
        {last_merged_at ? `last merge ${timeAgo(last_merged_at, now)}` : "no merge yet"}
      </div>
    </Link>
  );
}

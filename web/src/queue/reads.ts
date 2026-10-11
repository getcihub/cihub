import type { QueryClient } from "@tanstack/react-query";

import {
  getQueueOptions,
  graphOptions,
  listOrgQueuesOptions,
  listPrsOptions,
} from "../client/@tanstack/react-query.gen";

/**
 * The read of the queues of `org`: one page of the most queues that a page holds, which the frame, the overview
 * and the stream share.
 */
export function orgQueuesOptions(org: string) {
  return listOrgQueuesOptions({ path: { org }, query: { limit: 100 } });
}

/** The tabs of the items of a queue. */
export const TABS = ["live", "merged", "ended"] as const;
export type Tab = (typeof TABS)[number];

const STATES_OF: Record<Tab, string> = {
  live: "not_ready,pending,testing,pending_failure,tests_passed",
  merged: "merged",
  ended: "failed,cancelled",
};

export interface QueuePath {
  org: string;
  repo: string;
  queue: string;
}

const HOUR = 60 * 60 * 1000;

/**
 * The start of the reads of the tabs of the last 24 hours, at `now`: a full hour, so that the read stays the same
 * for an hour; the page keeps the exact 24 hours.
 */
export function sinceOf(now: number): string {
  return new Date(Math.floor(now / HOUR) * HOUR - 24 * HOUR).toISOString();
}

function itemsOf(path: QueuePath, tab: Tab, now: number) {
  const { org, repo, queue } = path;
  const query =
    tab === "live"
      ? { repo, queue, state: STATES_OF[tab], sort: "position" as const, limit: 100 }
      : { repo, queue, state: STATES_OF[tab], state_changed_since: sinceOf(now), limit: 100 };

  return listPrsOptions({ path: { org }, query });
}

/**
 * The reads of the page of a queue at `now`: the live items give the titles of the graph, `tab` the open tab.
 */
export function queueReads(path: QueuePath, tab: Tab, now: number) {
  return {
    detail: getQueueOptions({ path }),
    graph: graphOptions({ path }),
    live: itemsOf(path, "live", now),
    tab: itemsOf(path, tab, now),
  };
}

// The reads that an action on a queue changes.
const READS_OF_A_QUEUE = new Set([
  "getQueue",
  "graph",
  "listPrs",
  "getPr",
  "listEvents",
  "getRun",
  "listOrgQueues",
]);

/** Reads again what an action on a queue changes: the queue decides at its next reconciliation. */
export function readQueueAgain(queryClient: QueryClient): Promise<void> {
  return queryClient.invalidateQueries({
    predicate: (query) => {
      const key = query.queryKey[0] as { _id?: string } | undefined;
      return READS_OF_A_QUEUE.has(key?._id ?? "");
    },
  });
}

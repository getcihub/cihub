import type { QueryClient } from "@tanstack/react-query";

import {
  configCheckOptions,
  getQueueOptions,
  graphOptions,
  listItemsOptions,
} from "../client/@tanstack/react-query.gen";
import { READ_AGAIN_EVERY } from "../live";

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
  const query =
    tab === "live"
      ? { state: STATES_OF[tab], take: 100 }
      : { state: STATES_OF[tab], state_changed_since: sinceOf(now), take: 100 };

  return { ...listItemsOptions({ path, query }), refetchInterval: READ_AGAIN_EVERY };
}

/**
 * The reads of the page of a queue at `now`: the live items give the titles of the graph, `tab` the open tab.
 */
export function queueReads(path: QueuePath, tab: Tab, now: number) {
  return {
    detail: { ...getQueueOptions({ path }), refetchInterval: READ_AGAIN_EVERY },
    graph: { ...graphOptions({ path }), refetchInterval: READ_AGAIN_EVERY },
    check: { ...configCheckOptions({ path }), refetchInterval: READ_AGAIN_EVERY },
    live: itemsOf(path, "live", now),
    tab: itemsOf(path, tab, now),
  };
}

// The reads that an action on a queue changes.
const READS_OF_A_QUEUE = new Set([
  "getQueue",
  "graph",
  "listItems",
  "getPr",
  "testRun",
  "listOrgPrs",
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

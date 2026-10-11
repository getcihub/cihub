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

function itemsOf(path: QueuePath, tab: Tab) {
  return {
    ...listItemsOptions({ path, query: { state: STATES_OF[tab], take: 100 } }),
    refetchInterval: READ_AGAIN_EVERY,
  };
}

/** The reads of the page of a queue: the live items give the titles of the graph, `tab` the open tab. */
export function queueReads(path: QueuePath, tab: Tab) {
  return {
    detail: { ...getQueueOptions({ path }), refetchInterval: READ_AGAIN_EVERY },
    graph: { ...graphOptions({ path }), refetchInterval: READ_AGAIN_EVERY },
    check: { ...configCheckOptions({ path }), refetchInterval: READ_AGAIN_EVERY },
    live: itemsOf(path, "live"),
    tab: itemsOf(path, tab),
  };
}

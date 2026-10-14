import type { QueryKey } from "@tanstack/react-query";

import {
  getHealthOptions,
  getQueueOptions,
  graphOptions,
  listOrgQueuesOptions,
  listPrsOptions,
} from "@/client/@tanstack/react-query.gen";
import type { Graph } from "@/client/types.gen";

/**
 * The read of the queues of `org`: one page of the most queues that a page holds, which the frame, the overview
 * and the stream share.
 */
export function orgQueuesOptions(org: string): ReturnType<typeof listOrgQueuesOptions> {
  return listOrgQueuesOptions({ path: { org }, query: { limit: 100 } });
}

/** The states of a PR in the queue, as a filter of the API: waiting to enter, in line, tested, or passed. */
export const LIVE_STATES = "not_ready,pending,testing,pending_failure,tests_passed";

/** The tabs of the items of a queue. */
export const TABS = ["live", "merged", "ended"] as const;
export type Tab = (typeof TABS)[number];

const STATES_OF: Record<Tab, string> = {
  live: LIVE_STATES,
  merged: "merged",
  ended: "failed,cancelled",
};

/** The panes of the page of a queue: the queue now, and what it did over a period. */
export const PANES = ["graph", "health"] as const;
export type Pane = (typeof PANES)[number];

/** The periods of the Health of a queue. */
export const RANGES = ["24h", "7d", "30d"] as const;
export type Range = (typeof RANGES)[number];

export interface QueuePath {
  org: string;
  repo: string;
  queue: string;
}

/** What the dashboard reads in a query key of the generated client: the operation and the path of the read. */
export interface ReadKey {
  _id?: string;
  path?: { org?: string; repo?: string; queue?: string };
}

/** The fields of `queryKey`, a key of the generated client: its first entry holds them. */
export function readKeyOf(queryKey: QueryKey): ReadKey {
  return queryKey[0] ?? {};
}

const HOUR = 60 * 60 * 1000;

/**
 * The start of the reads of the tabs of the last 24 hours, at `now`: a full hour, so that the read stays the same
 * for an hour; the page keeps the exact 24 hours.
 */
export function sinceOf(now: number): string {
  return new Date(Math.floor(now / HOUR) * HOUR - 24 * HOUR).toISOString();
}

function itemsOf(path: QueuePath, tab: Tab, now: number): ReturnType<typeof listPrsOptions> {
  const { org, repo, queue } = path;
  const query =
    tab === "live"
      ? { repo, queue, state: STATES_OF[tab], sort: "position" as const, limit: 100 }
      : { repo, queue, state: STATES_OF[tab], state_changed_since: sinceOf(now), limit: 100 };

  return listPrsOptions({ path: { org }, query });
}

export interface QueueReads {
  detail: ReturnType<typeof getQueueOptions>;
  graph: ReturnType<typeof graphOptions>;
  live: ReturnType<typeof listPrsOptions>;
  tab: ReturnType<typeof listPrsOptions>;
}

/**
 * The reads of the page of a queue at `now`: the live items give the titles of the graph, `tab` the open tab.
 */
export function queueReads(path: QueuePath, tab: Tab, now: number): QueueReads {
  return {
    detail: getQueueOptions({ path }),
    graph: graphOptions({ path }),
    live: itemsOf(path, "live", now),
    tab: itemsOf(path, tab, now),
  };
}

/** The read of the Health of a queue over `range`. */
export function healthOptions(path: QueuePath, range: Range): ReturnType<typeof getHealthOptions> {
  return getHealthOptions({ path, query: { range } });
}

/**
 * The rank of each item admitted in the queue: 1, 2, 3 in the order of their positions. A position is an order,
 * not a rank. The graph has every live item, where the read of the items stops at 100.
 */
export function ranksOf(graph: Graph): Map<string, number> {
  return new Map(
    graph.nodes
      .flatMap((node) => (node.position == null ? [] : [{ id: node.item_id, position: node.position }]))
      .sort((a, b) => a.position - b.position)
      .map((node, index) => [node.id, index + 1]),
  );
}

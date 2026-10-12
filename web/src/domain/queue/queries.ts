import { type QueryKey, useSuspenseQueries, useSuspenseQuery } from "@tanstack/react-query";

import {
  getQueueOptions,
  graphOptions,
  listOrgQueuesOptions,
  listPrsOptions,
} from "@/client/@tanstack/react-query.gen";
import type { Graph, Pr, PrList, Queue } from "@/client/types.gen";

/**
 * The read of the queues of `org`: one page of the most queues that a page holds, which the frame, the overview
 * and the stream share.
 */
export function orgQueuesOptions(org: string): ReturnType<typeof listOrgQueuesOptions> {
  return listOrgQueuesOptions({ path: { org }, query: { limit: 100 } });
}

/** The queues of `org`, from the read that the layout of the organization starts. */
export function useOrgQueues(org: string): Queue[] {
  return useSuspenseQuery(orgQueuesOptions(org)).data.queues;
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

export interface QueuePageData {
  queue: Queue;
  graph: Graph;
  /** The live items, in the order of the queue, as the server gives them. */
  live: Pr[];
}

/** The data of the page of a queue at `now`, from the reads that its loader awaits. */
export function useQueuePageData(path: QueuePath, now: number): QueuePageData {
  const reads = queueReads(path, "live", now);
  const [detail, graph, live] = useSuspenseQueries({
    queries: [reads.detail, reads.graph, reads.live],
  });

  return { queue: detail.data, graph: graph.data, live: live.data.prs };
}

/**
 * The items of the tab `tab` at `now`: the same read as the live items for the tab of the live ones. Its loader
 * starts it without waiting: the items of a tab load under its tabs, and a change of tab never hides the page.
 */
export function useTabItems(path: QueuePath, tab: Tab, now: number): PrList {
  return useSuspenseQuery(queueReads(path, tab, now).tab).data;
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

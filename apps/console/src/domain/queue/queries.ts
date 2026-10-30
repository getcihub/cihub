import {
  getHealthOptions,
  getQueueOptions,
  graphOptions,
  listOrgQueuesOptions,
  listRepoPrsOptions,
} from "@/client/@tanstack/react-query.gen";
import type { Graph } from "@/client/types.gen";
import type { HealthPeriod } from "./period";
import type { Tab } from "./search";

/**
 * The read of the queues of `org`: one page of the most queues that a page holds, which the frame, the overview
 * and the stream share.
 */
export function orgQueuesOptions(org: string): ReturnType<typeof listOrgQueuesOptions> {
  return listOrgQueuesOptions({ path: { org }, query: { limit: 100 } });
}

/** The states of a PR in the queue, as a filter of the API: waiting to enter, in line, tested, or passed. */
export const LIVE_STATES = "not_ready,pending,testing,pending_failure,tests_passed";

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

const HOUR = 60 * 60 * 1000;

/**
 * The start of the reads of the tabs of the last 24 hours, at `now`: a full hour, so that the read stays the same
 * for an hour; the page keeps the exact 24 hours.
 */
export function sinceOf(now: number): string {
  return new Date(Math.floor(now / HOUR) * HOUR - 24 * HOUR).toISOString();
}

/** The read of the queue at `path`: its state, counters and settings. */
export function queueOptions(path: QueuePath): ReturnType<typeof getQueueOptions> {
  return getQueueOptions({ path });
}

/** The read of the graph of the queue at `path`: its runs and the items that they test. */
export function queueGraphOptions(path: QueuePath): ReturnType<typeof graphOptions> {
  return graphOptions({ path });
}

/**
 * The read of the items of the tab `tab` at `now`: the live ones in the order of the queue, the others of the last
 * 24 hours. The live items also give the titles of the graph.
 */
export function queueItemsOptions(
  path: QueuePath,
  tab: Tab,
  now: number,
): ReturnType<typeof listRepoPrsOptions> {
  const { org, repo, queue } = path;
  const query =
    tab === "live"
      ? { queue, state: STATES_OF[tab], sort: "position" as const, limit: 100 }
      : { queue, state: STATES_OF[tab], state_changed_since: sinceOf(now), limit: 100 };

  return listRepoPrsOptions({ path: { org, repo }, query });
}

/**
 * How often an open Health reads its facts again. The notifier writes them a few seconds after a change of the
 * queue, so a read at each change of the stream would mostly read the facts before it.
 */
const HEALTH_READ_EVERY_MS = 30_000;

/** The read of the Health of a queue over `period`. */
export function healthOptions(
  path: QueuePath,
  period: HealthPeriod,
): ReturnType<typeof getHealthOptions> & { refetchInterval: number } {
  return { ...getHealthOptions({ path, query: period }), refetchInterval: HEALTH_READ_EVERY_MS };
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

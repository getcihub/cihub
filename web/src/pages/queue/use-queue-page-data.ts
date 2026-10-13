import { useSuspenseQueries } from "@tanstack/react-query";

import type { Graph, Pr, Queue } from "@/client/types.gen";
import { type QueuePath, queueReads } from "@/domain/queue/queries";

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

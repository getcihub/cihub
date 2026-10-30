import type { QueryClient } from "@tanstack/react-query";

import {
  getPrQueryKey,
  getQueueQueryKey,
  graphQueryKey,
  listEventsQueryKey,
  listOrgPrsQueryKey,
  listOrgQueuesQueryKey,
  listRepoPrsQueryKey,
} from "@/client/@tanstack/react-query.gen";
import type { QueuePath } from "./queries";

/**
 * Reads again what an action on the queue at `path` changes: the queue, its graph, its items, the list of the queues
 * and the PRs of the person. A key without its query matches every read of the operation on that path. The queue
 * decides at its next reconciliation: the stream brings what changes after.
 */
export async function invalidateQueue(queryClient: QueryClient, path: QueuePath): Promise<void> {
  const { org, repo } = path;
  const keys = [
    getQueueQueryKey({ path }),
    graphQueryKey({ path }),
    listRepoPrsQueryKey({ path: { org, repo } }),
    listOrgQueuesQueryKey({ path: { org } }),
    listOrgPrsQueryKey({ path: { org } }),
  ];

  await Promise.all(keys.map((queryKey) => queryClient.invalidateQueries({ queryKey })));
}

/** Reads again what an action on the PR `number` of the queue at `path` changes: the queue, the PR and its events. */
export async function invalidatePr(queryClient: QueryClient, path: QueuePath, number: number): Promise<void> {
  const pr = { org: path.org, repo: path.repo, number };
  const keys = [getPrQueryKey({ path: pr }), listEventsQueryKey({ path: pr })];

  await Promise.all([
    invalidateQueue(queryClient, path),
    ...keys.map((queryKey) => queryClient.invalidateQueries({ queryKey })),
  ]);
}

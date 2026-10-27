import type { QueryClient } from "@tanstack/react-query";

import { type Failure, problemOf } from "@/domain/problem";
import { readKeyOf } from "./queries";

/** What the API answered to a refused action: its problem, else that it did not answer. */
export function refusalOf(failure: Failure): string {
  const problem = problemOf(failure);
  if (!problem) {
    return "The server did not answer. Try again.";
  }
  return problem.detail || "The server refused the request.";
}

// The reads that an action on a queue changes.
const READS_OF_A_QUEUE = new Set([
  "getQueue",
  "graph",
  "listOrgPrs",
  "listRepoPrs",
  "getPr",
  "listEvents",
  "getRun",
  "listOrgQueues",
]);

/** Reads again what an action on a queue changes: the queue decides at its next reconciliation. */
export function readQueueAgain(queryClient: QueryClient): Promise<void> {
  return queryClient.invalidateQueries({
    predicate: (query) => READS_OF_A_QUEUE.has(readKeyOf(query.queryKey)._id ?? ""),
  });
}

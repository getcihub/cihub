import {
  type QueryClient,
  type UseMutationOptions,
  type UseMutationResult,
  useMutation,
  useQueryClient,
} from "@tanstack/react-query";

import { readKeyOf } from "./queries";

/** An action on a queue: once it settles, the page reads again what it changes. */
export function useQueueMutation<TData, TError, TVariables>(
  options: UseMutationOptions<TData, TError, TVariables>,
): UseMutationResult<TData, TError, TVariables> {
  const queryClient = useQueryClient();

  return useMutation({ ...options, onSettled: () => readQueueAgain(queryClient) });
}

/** What the API answered to a refused action: its problem, else that it did not answer. */
export function refusalOf(error: unknown): string {
  if (typeof error === "object" && error !== null && "detail" in error && typeof error.detail === "string") {
    return error.detail || "The server refused the request.";
  }
  return "The server did not answer. Try again.";
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
    predicate: (query) => READS_OF_A_QUEUE.has(readKeyOf(query.queryKey)._id ?? ""),
  });
}

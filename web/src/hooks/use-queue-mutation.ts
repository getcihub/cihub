import {
  type UseMutationOptions,
  type UseMutationResult,
  useMutation,
  useQueryClient,
} from "@tanstack/react-query";

import { readQueueAgain } from "@/domain/queue/actions";

/** An action on a queue: once it settles, the page reads again what it changes. */
export function useQueueMutation<TData, TError, TVariables>(
  options: UseMutationOptions<TData, TError, TVariables>,
): UseMutationResult<TData, TError, TVariables> {
  const queryClient = useQueryClient();

  return useMutation({ ...options, onSettled: () => readQueueAgain(queryClient) });
}

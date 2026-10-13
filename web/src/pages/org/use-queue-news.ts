import { useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";

import { followQueueNews } from "@/domain/queue/live";

/**
 * Follows the changes of the queues of `org` while one of its pages is open, and opens the stream again when the
 * browser closes it.
 */
export function useQueueNews(org: string): void {
  const queryClient = useQueryClient();

  useEffect(() => followQueueNews(queryClient, org), [org, queryClient]);
}

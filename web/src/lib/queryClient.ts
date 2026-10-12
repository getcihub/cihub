import { QueryClient } from "@tanstack/react-query";

/** Whether `error` is a problem that the API answered with a 4xx status: the same request gets the same answer. */
function isRefusal(error: unknown): boolean {
  return (
    typeof error === "object" &&
    error !== null &&
    "status" in error &&
    typeof error.status === "number" &&
    error.status >= 400 &&
    error.status < 500
  );
}

/** A read that failed once tries again, unless the API refused it: a 4xx shows at once. */
export function shouldRetryRead(failureCount: number, error: unknown): boolean {
  return failureCount < 1 && !isRefusal(error);
}

/** The client of the queries of the dashboard: one per page load, carried by the context of the router. */
export function createQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      // A read stays fresh for 30 seconds: a click inside an organization reads it again only after that. The
      // stream of the organization (`live.ts`) invalidates what changed.
      queries: { staleTime: 30_000, retry: shouldRetryRead },
    },
  });
}

/**
 * Starts `reads` together into the cache, for a loader. A failed read does not stop the loader: the component
 * that reads it shows the failure, or a page of its own for a 404.
 */
export async function startReads(...reads: Promise<unknown>[]): Promise<void> {
  await Promise.all(reads.map((read) => read.catch(() => undefined)));
}

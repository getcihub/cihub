import { QueryClient } from "@tanstack/react-query";

/**
 * A read that failed once tries again, unless the API refused it with a 4xx status: the same request gets the
 * same answer, so it shows at once.
 */
export function shouldRetryRead(failureCount: number, failure: Error | { status: number }): boolean {
  const isRefusal = !(failure instanceof Error) && failure.status >= 400 && failure.status < 500;

  return failureCount < 1 && !isRefusal;
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

import { QueryClient } from "@tanstack/react-query";
import { describe, expect, it } from "vitest";

import { getPrQueryKey, getRepoQueryKey } from "@/client/@tanstack/react-query.gen";
import { invalidatePr, invalidateQueue } from "./actions";
import { orgQueuesOptions, queueGraphOptions, queueItemsOptions, queueOptions } from "./queries";

const MAIN = { org: "acme", repo: "web", queue: "main" };
const RELEASE = { org: "acme", repo: "web", queue: "release" };
const OTHER_ORG = { org: "globex", repo: "web", queue: "main" };

function cacheOf(keys: readonly (readonly object[])[]): QueryClient {
  const queryClient = new QueryClient();
  for (const queryKey of keys) {
    queryClient.setQueryData(queryKey, {});
  }
  return queryClient;
}

function isStale(queryClient: QueryClient, queryKey: readonly object[]): boolean {
  return queryClient.getQueryState(queryKey)?.isInvalidated ?? false;
}

describe("the reads that an action changes", () => {
  it("are the reads of its queue, every tab of its repository and the lists of its organization", async () => {
    const changed = [
      queueOptions(MAIN).queryKey,
      queueGraphOptions(MAIN).queryKey,
      queueItemsOptions(MAIN, "live", 0).queryKey,
      queueItemsOptions(MAIN, "merged", 0).queryKey,
      orgQueuesOptions("acme").queryKey,
    ];
    const kept = [
      queueOptions(RELEASE).queryKey,
      queueOptions(OTHER_ORG).queryKey,
      getRepoQueryKey({ path: { org: "acme", repo: "web" } }),
    ];
    const queryClient = cacheOf([...changed, ...kept]);

    await invalidateQueue(queryClient, MAIN);

    expect(changed.map((key) => isStale(queryClient, key))).toEqual(changed.map(() => true));
    expect(kept.map((key) => isStale(queryClient, key))).toEqual(kept.map(() => false));
  });

  it("add the PR and its events for an action on a PR, not the other PRs", async () => {
    const acted = getPrQueryKey({ path: { org: "acme", repo: "web", number: 118 } });
    const other = getPrQueryKey({ path: { org: "acme", repo: "web", number: 119 } });
    const queryClient = cacheOf([acted, other]);

    await invalidatePr(queryClient, MAIN, 118);

    expect(isStale(queryClient, acted)).toBe(true);
    expect(isStale(queryClient, other)).toBe(false);
  });
});

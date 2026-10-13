import { useQuery } from "@tanstack/react-query";

import { listPrsOptions } from "@/client/@tanstack/react-query.gen";
import type { Pr } from "@/client/types.gen";
import { LIVE_STATES } from "@/domain/queue/queries";

/** How long a merged PR stays among the PRs of the person: they see it reach its branch. */
const MERGED_STAYS_MS = 10 * 60 * 1000;

/**
 * The reads of the PRs of the person of the session in the queues of `org`: the live ones, in the order of their
 * queues, and the last ones that merged.
 */
export function minePrsReads(org: string): Record<"live" | "merged", ReturnType<typeof listPrsOptions>> {
  return {
    live: listPrsOptions({
      path: { org },
      query: { author: "@me", state: LIVE_STATES, sort: "position", limit: 100 },
    }),
    merged: listPrsOptions({ path: { org }, query: { author: "@me", state: "merged", limit: 10 } }),
  };
}

/** The PRs of the person: the ones merged a moment ago, then the live ones, by repository and queue. */
export function minePrsOf(live: Pr[] = [], merged: Pr[] = [], now = Date.now()): Pr[] {
  const recent = merged.filter((pr) => now - Date.parse(pr.state_changed_at) < MERGED_STAYS_MS);
  const where = (pr: Pr) => `${pr.repo}/${pr.queue}`;

  return [...recent, ...live].sort((a, b) => (where(a) < where(b) ? -1 : where(a) > where(b) ? 1 : 0));
}

/**
 * The PRs of the person of the session in the queues of `org`; none while they load or when their read fails.
 * They only add to a page: their read never stops it, unlike the reads that the page needs.
 */
export function useMinePrs(org: string): Pr[] {
  const reads = minePrsReads(org);
  const live = useQuery(reads.live).data?.prs;
  const merged = useQuery(reads.merged).data?.prs;
  return minePrsOf(live, merged);
}

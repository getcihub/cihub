import { listOrgPrsOptions } from "@/client/@tanstack/react-query.gen";
import type { Pr } from "@/client/types.gen";
import { LIVE_STATES } from "@/domain/queue/queries";

/** How long a merged PR stays among the PRs of the person: they see it reach its branch. */
const MERGED_STAYS_MS = 10 * 60 * 1000;

/** The read of the live PRs of `login`, the person of the session, in the queues of `org`, in their order. */
export function mineLiveOptions(org: string, login: string): ReturnType<typeof listOrgPrsOptions> {
  return listOrgPrsOptions({
    path: { org },
    query: { author: login, state: LIVE_STATES, sort: "position", limit: 100 },
  });
}

/** The read of the last PRs of `login` that merged in the queues of `org`. */
export function mineMergedOptions(org: string, login: string): ReturnType<typeof listOrgPrsOptions> {
  return listOrgPrsOptions({ path: { org }, query: { author: login, state: "merged", limit: 10 } });
}

/** The PRs of the person: the ones merged a moment ago, then the live ones, by repository and queue. */
export function minePrsOf(live: Pr[] = [], merged: Pr[] = [], now = Date.now()): Pr[] {
  const recent = merged.filter((pr) => now - Date.parse(pr.state_changed_at) < MERGED_STAYS_MS);
  const where = (pr: Pr) => `${pr.repo}/${pr.queue}`;

  return [...recent, ...live].sort((a, b) => where(a).localeCompare(where(b)));
}

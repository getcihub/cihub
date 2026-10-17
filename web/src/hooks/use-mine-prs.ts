import { useQuery } from "@tanstack/react-query";

import type { Pr } from "@/client/types.gen";
import { minePrsOf, minePrsReads } from "@/domain/mine/queries";
import { useMe } from "@/hooks/use-me";

/**
 * The PRs of the person of the session in the queues of `org`; none while they load or when their read fails.
 * They only add to a page: their read never stops it, unlike the reads that the page needs.
 */
export function useMinePrs(org: string): Pr[] {
  const reads = minePrsReads(org, useMe().login);
  const live = useQuery(reads.live).data?.prs;
  const merged = useQuery(reads.merged).data?.prs;
  return minePrsOf(live, merged);
}

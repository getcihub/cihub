import { useQuery } from "@tanstack/react-query";

import { listOrgPrsOptions } from "../client/@tanstack/react-query.gen";
import type { OrgPr } from "../client/types.gen";

/** The read of the PRs of `login` in the queues of `org`. */
export function minePrsOptions(org: string, login: string) {
  return listOrgPrsOptions({ path: { org }, query: { author: login } });
}

/**
 * The PRs of `login` in the queues of `org`; none while they load or when their read fails. They only add to a
 * page: their read never stops it, unlike the reads that the page needs.
 */
export function useMinePrs(org: string, login: string): OrgPr[] {
  return useQuery(minePrsOptions(org, login)).data?.prs ?? [];
}

function ordinal(number: number): string {
  const tens = number % 100;
  const suffix =
    tens >= 11 && tens <= 13 ? "th" : ({ 1: "st", 2: "nd", 3: "rd" } as Record<number, string>)[number % 10];
  return `${String(number)}${suffix ?? "th"}`;
}

/** Where the PR is, in a few words: "2nd in line", "testing", "merged into main". */
export function whereOf(pr: OrgPr, isPaused: boolean, branch: string): string {
  switch (pr.item.state) {
    case "not_ready":
      return "not in line yet";
    case "pending":
      return `${ordinal(pr.ahead.length + 1)} in line${isPaused ? ", paused" : ""}`;
    case "testing":
      return "testing";
    case "pending_failure":
      return "tests failed";
    case "tests_passed":
      return "merging";
    case "merged":
      return `merged into ${branch}`;
    default:
      return pr.item.state;
  }
}

/** The estimate of the server, as "about 12 min"; `null` without one. */
export function etaOf(pr: OrgPr): string | null {
  const seconds = pr.merges_in_seconds;
  if (seconds == null) {
    return null;
  }
  return seconds < 60 ? "a few seconds" : `about ${String(Math.ceil(seconds / 60))} min`;
}

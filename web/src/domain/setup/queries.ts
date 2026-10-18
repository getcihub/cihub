import type { QueryClient } from "@tanstack/react-query";

import { getRepoOptions, listReposOptions } from "@/client/@tanstack/react-query.gen";
import type { RepoList, SetupSummary } from "@/client/types.gen";
import { readKeyOf } from "@/domain/queue/queries";

/** An organization has a few hundred repositories at most: one page of the most that a page holds. */
const REPOS_PER_PAGE = 100;

/** The read of the repositories of `org` whose name holds `q`, with the verdict of their setup. */
export function reposOptions(org: string, q: string | undefined): ReturnType<typeof listReposOptions> {
  return listReposOptions({ path: { org }, query: { q, limit: REPOS_PER_PAGE } });
}

/** The read of the repository `repo` of `org`, with its whole setup and what the person may do on it. */
export function repoOptions(org: string, repo: string): ReturnType<typeof getRepoOptions> {
  return getRepoOptions({ path: { org, repo } });
}

/** The server read the setup, and no read that an admin asked for waits. */
export function isSetupRead(setup: Pick<SetupSummary, "state" | "requested_at">): boolean {
  return setup.state !== "checking" && !setup.requested_at;
}

/** A list of repositories waits for a read: one of them is checked, or, after an install, none came yet. */
export function isListWaiting(list: RepoList | undefined, isConnecting: boolean): boolean {
  if (!list) {
    return true;
  }
  return (isConnecting && list.repos.length === 0) || list.repos.some((repo) => !isSetupRead(repo.setup));
}

/** Reads again the lists of the repositories of `org`: a setup in them changed. */
export function readReposAgain(queryClient: QueryClient, org: string): Promise<void> {
  return queryClient.invalidateQueries({
    predicate: (query) => {
      const key = readKeyOf(query.queryKey);
      return key._id === "listRepos" && key.path?.org === org;
    },
  });
}

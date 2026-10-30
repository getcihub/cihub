import type { QueryClient } from "@tanstack/react-query";

import { getRepoQueryKey, listReposQueryKey } from "@/client/@tanstack/react-query.gen";

/** Reads again the lists of the repositories of `org`, whatever their search: a setup in them changed. */
export async function invalidateRepos(queryClient: QueryClient, org: string): Promise<void> {
  await queryClient.invalidateQueries({ queryKey: listReposQueryKey({ path: { org } }) });
}

/** Reads again the repository `repo` of `org` and the lists that show it: the server reads its setup again. */
export async function invalidateRepo(queryClient: QueryClient, org: string, repo: string): Promise<void> {
  await Promise.all([
    queryClient.invalidateQueries({ queryKey: getRepoQueryKey({ path: { org, repo } }) }),
    invalidateRepos(queryClient, org),
  ]);
}

import { useMutation, useQueryClient, useSuspenseQuery } from "@tanstack/react-query";

import { syncRepoMutation } from "@/client/@tanstack/react-query.gen";
import type { RepoDetail } from "@/client/types.gen";
import { isSetupRead, readReposAgain, repoOptions } from "@/domain/setup/queries";
import { useReadAgain } from "./use-read-again";

export interface RepoSetupRead {
  repo: RepoDetail;
  /** The server reads the setup: the page waits for it. */
  isChecking: boolean;
  /** The read takes longer than the page waits: the page says so, and reads no more. */
  hasGivenUp: boolean;
  /** "Check again", for an admin of the repository only. */
  checkAgain: { run: () => void; isPending: boolean } | null;
}

/**
 * The repository `name` of `org` with its whole setup, which a read follows every 2 seconds while it waits, and
 * "Check again" for an admin. `onSync` runs once the server took the request: a list that shows the setup waits too.
 */
export function useRepoSetup(org: string, name: string, onSync?: () => void): RepoSetupRead {
  const queryClient = useQueryClient();
  const wait = useReadAgain((data: RepoDetail | undefined) => !data || !isSetupRead(data.setup));
  const read = repoOptions(org, name);
  const { data: repo } = useSuspenseQuery({ ...read, refetchInterval: wait.refetchInterval });
  const sync = useMutation({
    ...syncRepoMutation(),
    onSettled: () =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: read.queryKey }),
        readReposAgain(queryClient, org),
      ]),
    onSuccess: () => {
      wait.restart();
      onSync?.();
    },
  });
  const isChecking = !isSetupRead(repo.setup);

  return {
    repo,
    isChecking,
    hasGivenUp: isChecking && wait.isOver,
    checkAgain: repo.permission.admin
      ? {
          run: () => {
            sync.mutate({ path: { org, repo: name } });
          },
          isPending: sync.isPending,
        }
      : null,
  };
}

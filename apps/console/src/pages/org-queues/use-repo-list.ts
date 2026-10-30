import { useMutation, useQueryClient, useSuspenseQuery } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";

import { syncUserMutation } from "@/client/@tanstack/react-query.gen";
import type { RepoList, User } from "@/client/types.gen";
import { userOptions } from "@/domain/session/queries";
import { invalidateRepos } from "@/domain/setup/actions";
import { isListWaiting, reposOptions } from "@/domain/setup/queries";
import { useReadAgain } from "@/hooks/use-read-again";

export interface RepoListRead {
  /** Every repository of the organization, whatever the search. */
  all: RepoList;
  /** The repositories that the search gives. */
  list: RepoList;
  /** The first read of the repositories that the person reaches has not ended yet. */
  isFirstRead: boolean;
  /** The list waits for a read of the server: a setup, the first read, or one that the person asked for. */
  isWaiting: boolean;
  /** The wait lasted past a minute: the page stopped reading. */
  hasGivenUp: boolean;
  /** The list is empty while CIHub reads the repositories of the person. */
  isReading: boolean;
  /** "Check again": reads the repositories that the person reaches again. */
  checkAgain: { run: () => void; isPending: boolean; hasFailed: boolean };
  /** Waits again, after an action that changes a setup of the list. */
  waitAgain: () => void;
}

/**
 * The repositories of `org` that `search` gives, which a read follows while they wait for the server: after an
 * install (`isConnecting`), before the first read of the repositories of the person, and after "Check again".
 */
export function useRepoList(org: string, search: string | undefined, isConnecting: boolean): RepoListRead {
  const queryClient = useQueryClient();
  const firstRead = useReadAgain((data: User | undefined) => !data?.synced_at);
  const { data: user } = useSuspenseQuery({ ...userOptions(), refetchInterval: firstRead.refetchInterval });
  const isFirstRead = !user.synced_at;

  const [hasAskedAgain, setHasAskedAgain] = useState(false);
  const wait = useReadAgain((data: RepoList | undefined) =>
    isListWaiting(data, isConnecting || isFirstRead || hasAskedAgain),
  );
  const { data: all } = useSuspenseQuery(reposOptions(org, undefined));
  const { data: list } = useSuspenseQuery({
    ...reposOptions(org, search),
    refetchInterval: wait.refetchInterval,
  });
  const sync = useMutation({
    ...syncUserMutation(),
    onSettled: () => invalidateRepos(queryClient, org),
    onSuccess: () => {
      setHasAskedAgain(true);
      wait.restart();
    },
  });

  const isWaiting = isListWaiting(list, isConnecting || isFirstRead || (hasAskedAgain && !wait.isOver));
  const hasGivenUp = isWaiting && wait.isOver;
  const isReading = !isConnecting && isWaiting && !hasGivenUp && list.repos.length === 0;

  // The list read last may predate the end of the first read.
  const wasFirstRead = useRef(isFirstRead);
  useEffect(() => {
    if (wasFirstRead.current && !isFirstRead) {
      void invalidateRepos(queryClient, org);
    }
    wasFirstRead.current = isFirstRead;
  }, [isFirstRead, queryClient, org]);

  return {
    all,
    list,
    isFirstRead,
    isWaiting,
    hasGivenUp,
    isReading,
    checkAgain: {
      run: () => {
        sync.mutate({});
      },
      isPending: sync.isPending,
      hasFailed: sync.isError,
    },
    waitAgain: wait.restart,
  };
}

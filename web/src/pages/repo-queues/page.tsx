import { useMutation, useQueryClient, useSuspenseQuery } from "@tanstack/react-query";
import { getRouteApi } from "@tanstack/react-router";

import { syncRepoMutation } from "@/client/@tanstack/react-query.gen";
import type { RepoDetail } from "@/client/types.gen";
import { MinePrs } from "@/components/mine/mine";
import { OverviewHead } from "@/components/queue/overview-head";
import { QueueCards } from "@/components/queue/queue-cards";
import { Page } from "@/components/ui/page";
import { isSetupRead, repoOptions } from "@/domain/setup/queries";
import { useMinePrs } from "@/hooks/use-mine-prs";
import { useNow } from "@/hooks/use-now";
import { useOrgQueues } from "@/hooks/use-org-queues";
import { useReadAgain } from "@/hooks/use-read-again";
import { RepoSetup } from "./components/repo-setup";

const route = getRouteApi("/orgs/$org/repos/$repo/");

/**
 * A repository: the setup of its default branch, which a read follows every 2 seconds while it waits, then its
 * queues, after the PRs of the person in it.
 */
export function RepoQueuesPage() {
  const { org, repo: name } = route.useParams();
  const queryClient = useQueryClient();
  const now = useNow();
  const wait = useReadAgain((data: RepoDetail | undefined) => !data || !isSetupRead(data.setup));
  const read = repoOptions(org, name);
  const { data: repo } = useSuspenseQuery({ ...read, refetchInterval: wait.refetchInterval });
  const sync = useMutation({
    ...syncRepoMutation(),
    onSettled: () => queryClient.invalidateQueries({ queryKey: read.queryKey }),
    onSuccess: wait.restart,
  });
  const queues = useOrgQueues(org).filter((queue) => queue.repo === name);
  const mine = useMinePrs(org).filter((pr) => pr.repo === name);
  const checkAgain = repo.permission.admin
    ? {
        run: () => {
          sync.mutate({ path: { org, repo: name } });
        },
        isPending: sync.isPending,
      }
    : null;

  return (
    <Page>
      <OverviewHead where={org} title={name} />
      <RepoSetup
        repo={repo}
        hasQueue={queues.length > 0}
        now={now}
        hasGivenUp={!isSetupRead(repo.setup) && wait.isOver}
        checkAgain={checkAgain}
      />
      {queues.length > 0 && (
        <>
          <MinePrs org={org} prs={mine} queues={queues} />
          <QueueCards org={org} queues={queues} />
        </>
      )}
    </Page>
  );
}

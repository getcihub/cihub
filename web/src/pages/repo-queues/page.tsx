import { getRouteApi } from "@tanstack/react-router";

import { MinePrs } from "@/components/mine/mine";
import { OverviewHead } from "@/components/queue/overview-head";
import { QueueCards } from "@/components/queue/queue-cards";
import { Page } from "@/components/ui/page";
import { useMinePrs } from "@/hooks/use-mine-prs";
import { useOrgQueues } from "@/hooks/use-org-queues";

const route = getRouteApi("/orgs/$org/repos/$repo/");

/** The queues of a repository, after the PRs of the person in it. */
export function RepoQueuesPage() {
  const { org, repo } = route.useParams();
  const queues = useOrgQueues(org).filter((queue) => queue.repo === repo);
  const mine = useMinePrs(org).filter((pr) => pr.repo === repo);

  return (
    <Page>
      <OverviewHead where={`${org}/${repo}`} />
      <MinePrs org={org} prs={mine} queues={queues} />
      <QueueCards org={org} queues={queues} />
    </Page>
  );
}

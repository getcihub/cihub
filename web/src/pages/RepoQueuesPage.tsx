import { type ErrorComponentProps, getRouteApi } from "@tanstack/react-router";

import { ServerError } from "@/components/errors/ServerError";
import { MinePrs } from "@/components/mine/Mine";
import { OverviewHead } from "@/components/queue/OverviewHead";
import { QueueCards } from "@/components/queue/QueueCards";
import { useMinePrs } from "@/domain/mine/queries";
import { useOrgQueues } from "@/domain/queue/queries";
import { Card } from "@/ui/card";
import { Page } from "@/ui/page";

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

/** The queues could not be read: the head of the page stays, with what the server answered. */
export function RepoQueuesPageError({ error, reset }: ErrorComponentProps) {
  const { org, repo } = route.useParams();

  return (
    <Page>
      <OverviewHead where={`${org}/${repo}`} />
      <Card>
        <ServerError error={error} reset={reset} />
      </Card>
    </Page>
  );
}

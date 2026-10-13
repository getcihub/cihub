import { type ErrorComponentProps, getRouteApi } from "@tanstack/react-router";

import { ServerErrorPage } from "@/components/errors/server-error-page";
import { useOrgQueues } from "@/hooks/use-org-queues";
import { QueueHead } from "@/pages/queue/components/queue-head";

const route = getRouteApi("/orgs/$org/repos/$repo/queues/$queue");

/** A read of the page failed: its head stays, from the list of the queues, with what the server answered. */
export function QueuePageError(props: ErrorComponentProps) {
  const { org, repo, queue: name } = route.useParams();
  const queue = useOrgQueues(org).find((candidate) => candidate.repo === repo && candidate.name === name);

  return (
    <ServerErrorPage
      {...props}
      head={queue && <QueueHead path={{ org, repo, queue: name }} queue={queue} />}
    />
  );
}

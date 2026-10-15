import { type ErrorComponentProps, getRouteApi } from "@tanstack/react-router";

import type { Queue } from "@/client/types.gen";
import { ServerErrorPage } from "@/components/errors/server-error-page";
import type { QueuePath } from "@/domain/queue/queries";
import { useOrgQueues } from "@/hooks/use-org-queues";
import { useSentNotice } from "@/hooks/use-sent-notice";
import { QueueHead } from "@/pages/queue/components/queue-head";

const route = getRouteApi("/orgs/$org/repos/$repo/queues/$queue");

function Head({ path, queue }: { path: QueuePath; queue: Queue }) {
  return <QueueHead path={path} queue={queue} sent={useSentNotice(queue.id)} />;
}

/** A read of the page failed: its head stays, from the list of the queues, with what the server answered. */
export function QueuePageError(props: ErrorComponentProps) {
  const { org, repo, queue: name } = route.useParams();
  const queue = useOrgQueues(org).find(
    (candidate) => candidate.repo === repo && candidate.target_branch === name,
  );

  return (
    <ServerErrorPage {...props} head={queue && <Head path={{ org, repo, queue: name }} queue={queue} />} />
  );
}

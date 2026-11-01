import { type ErrorComponentProps, getRouteApi } from "@tanstack/react-router";

import type { Queue } from "@/client/types.gen";
import { ServerErrorPage } from "@/components/errors/server-error-page";
import type { QueuePath } from "@/domain/queue/queries";
import { useOrgQueues } from "@/hooks/use-org-queues";
import { useSentNotice } from "@/hooks/use-sent-notice";
import type { Pane } from "@/domain/queue/search";
import { QueueHead } from "@/pages/queue/components/head/queue-head";
import { QueuePanes } from "@/pages/queue/components/head/queue-panes";

const route = getRouteApi("/orgs/$org/repos/$repo/queues/$queue");

function Head({ path, queue, pane }: { path: QueuePath; queue: Queue; pane: Pane }) {
  return (
    <QueueHead path={path} queue={queue} sent={useSentNotice(queue.id)}>
      <QueuePanes path={path} pane={pane} />
    </QueueHead>
  );
}

/** A read of the page failed: its head stays, from the list of the queues, with what the server answered. */
export function QueuePageError(props: ErrorComponentProps) {
  const { org, repo, queue: name } = route.useParams();
  const { pane } = route.useSearch();
  const queue = useOrgQueues(org).find(
    (candidate) => candidate.repo === repo && candidate.target_branch === name,
  );

  return (
    <ServerErrorPage
      {...props}
      band={queue && <Head path={{ org, repo, queue: name }} queue={queue} pane={pane} />}
    />
  );
}

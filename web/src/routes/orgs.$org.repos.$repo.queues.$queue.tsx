import { useSuspenseQuery } from "@tanstack/react-query";
import { createFileRoute, stripSearchParams } from "@tanstack/react-router";
import { z } from "zod";

import { listOrgQueuesOptions } from "../client/@tanstack/react-query.gen";
import { NotFoundPage } from "../pages/NotFoundPage";
import { ServerErrorPage } from "../pages/ServerErrorPage";
import { QueuePage } from "../queue/QueuePage";
import { TABS, queueReads } from "../queue/reads";

const search = z.object({ tab: z.enum(TABS).default("live").catch("live") });

export const Route = createFileRoute("/orgs/$org/repos/$repo/queues/$queue")({
  validateSearch: search,
  // The tab of the live items is the page itself: its address carries no tab.
  search: { middlewares: [stripSearchParams({ tab: "live" })] },
  loaderDeps: ({ search: { tab } }) => ({ tab }),
  // The reads leave together. A queue that does not exist answers 404: the page says "not found" in the frame,
  // so a failed read does not stop the loader.
  loader: async ({ context: { queryClient }, params, deps }) => {
    const reads = queueReads(params, deps.tab, Date.now());
    const ignore = () => undefined;

    await Promise.all([
      queryClient.query(reads.detail).catch(ignore),
      queryClient.query(reads.graph).catch(ignore),
      queryClient.query(reads.check).catch(ignore),
      queryClient.query(reads.live).catch(ignore),
      queryClient.query(reads.tab).catch(ignore),
    ]);
  },
  component: QueueRoute,
  // A failed read shows under the frame, which stays.
  errorComponent: ServerErrorPage,
});

function QueueRoute() {
  const { org, repo, queue } = Route.useParams();
  const { tab } = Route.useSearch();
  const { data } = useSuspenseQuery(listOrgQueuesOptions({ path: { org } }));

  if (!data.queues.some((candidate) => candidate.repo === repo && candidate.queue.name === queue)) {
    return <NotFoundPage />;
  }

  return <QueuePage path={{ org, repo, queue }} tab={tab} />;
}

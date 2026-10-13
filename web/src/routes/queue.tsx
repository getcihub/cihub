import { createFileRoute, notFound, stripSearchParams } from "@tanstack/react-router";
import { z } from "zod";

import { NotFoundPage } from "@/components/errors/NotFoundPage";
import { TABS, orgQueuesOptions, queueReads } from "@/domain/queue/queries";
import { startReads } from "@/lib/queryClient";
import { QueuePage, QueuePageError } from "@/pages/QueuePage";
import { QueuePageSkeleton } from "@/pages/skeletons";

const search = z.object({ tab: z.enum(TABS).default("live").catch("live") });

export const Route = createFileRoute("/orgs/$org/repos/$repo/queues/$queue")({
  validateSearch: search,
  // The tab of the live items is the page itself: its address carries no tab.
  search: { middlewares: [stripSearchParams({ tab: "live" })] },
  loaderDeps: ({ search: { tab } }) => ({ tab }),
  // A queue that the organization does not have is not found, in the frame. The reads of the page leave together;
  // a failed one does not stop the loader: the page says that it could not read it.
  loader: async ({ context: { queryClient }, params, deps }) => {
    // The params of a page of a PR under this one also hold its number.
    const { org, repo, queue } = params;
    const list = await queryClient.query(orgQueuesOptions(org)).catch(() => null);
    if (list && !list.queues.some((candidate) => candidate.repo === repo && candidate.name === queue)) {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- the router stops on a thrown notFound
      throw notFound();
    }

    const reads = queueReads({ org, repo, queue }, deps.tab, Date.now());
    // The items of a tab load under its tabs (`useTabItems`): a change of tab never shows the skeleton of the page.
    void startReads(queryClient.query(reads.tab));
    await startReads(
      queryClient.query(reads.detail),
      queryClient.query(reads.graph),
      queryClient.query(reads.live),
    );
  },
  component: QueuePage,
  pendingComponent: QueuePageSkeleton,
  errorComponent: QueuePageError,
  notFoundComponent: NotFoundPage,
});

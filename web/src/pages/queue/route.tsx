import { noop } from "@tanstack/react-query";
import { createFileRoute, notFound, stripSearchParams } from "@tanstack/react-router";
import { z } from "zod";

import { NotFoundPage } from "@/components/errors/not-found-page";
import { QueueSkeleton } from "@/components/queue/queue-skeleton";
import { periodOfSearch } from "@/domain/queue/period";
import { PANES, RANGES, TABS, healthOptions, orgQueuesOptions, queueReads } from "@/domain/queue/queries";
import { QueuePageError } from "./error";
import { QueuePage } from "./page";

const search = z.object({
  tab: z.enum(TABS).default("live").catch("live"),
  pane: z.enum(PANES).default("graph").catch("graph"),
  range: z.enum(RANGES).default("24h").catch("24h"),
  // A period that the person chose, two RFC 3339 times; the server checks it.
  from: z.string().optional().catch(undefined),
  to: z.string().optional().catch(undefined),
});

export const Route = createFileRoute("/orgs/$org/repos/$repo/queues/$queue")({
  validateSearch: search,
  // The tab of the live items, the graph and the last 24 hours are the page itself: its address carries none of them.
  search: { middlewares: [stripSearchParams({ tab: "live", pane: "graph", range: "24h" })] },
  loaderDeps: ({ search: { tab, pane, range, from, to } }) => ({ tab, pane, range, from, to }),
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
    void queryClient.query(reads.tab).catch(noop);
    // The Health loads under the tabs of the panes, as the items of a tab do.
    if (deps.pane === "health") {
      void queryClient.query(healthOptions({ org, repo, queue }, periodOfSearch(deps))).catch(noop);
    }
    await Promise.allSettled([
      queryClient.query(reads.detail),
      queryClient.query(reads.graph),
      queryClient.query(reads.live),
    ]);
  },
  component: QueuePage,
  pendingComponent: QueueSkeleton,
  errorComponent: QueuePageError,
  notFoundComponent: NotFoundPage,
});

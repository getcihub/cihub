import { noop } from "@tanstack/react-query";
import { createFileRoute, notFound, stripSearchParams } from "@tanstack/react-router";

import { NotFoundPage } from "@/components/errors/not-found-page";
import { QueueSkeleton } from "@/components/queue/queue-skeleton";
import { periodOfSearch } from "@/domain/queue/period";
import {
  healthOptions,
  orgQueuesOptions,
  queueGraphOptions,
  queueItemsOptions,
  queueOptions,
} from "@/domain/queue/queries";
import { QUEUE_SEARCH_DEFAULTS, queueSearch } from "@/domain/queue/search";
import { repoOptions } from "@/domain/setup/queries";
import { QueuePageError } from "./error";
import { QueuePage } from "./page";

export const Route = createFileRoute("/orgs/$org/repos/$repo/queues/$queue")({
  validateSearch: queueSearch,
  search: { middlewares: [stripSearchParams(QUEUE_SEARCH_DEFAULTS)] },
  loaderDeps: ({ search: { tab, pane, range, from, to } }) => ({ tab, pane, range, from, to }),
  loader: async ({ context: { queryClient }, params, deps }) => {
    // The params of a page of a PR under this one also hold its number.
    const path = { org: params.org, repo: params.repo, queue: params.queue };
    const now = Date.now();

    // A queue that the organization does not have is not found, in the frame.
    const list = await queryClient.query(orgQueuesOptions(path.org)).catch(() => null);
    const isKnown = list?.queues.some(
      (candidate) => candidate.repo === path.repo && candidate.target_branch === path.queue,
    );
    if (list && !isKnown) {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- the router stops on a thrown notFound
      throw notFound();
    }

    // The items of a tab and the Health load under their own tabs: a change of tab never shows the skeleton of the
    // page.
    void queryClient.query(queueItemsOptions(path, deps.tab, now)).catch(noop);
    if (deps.pane === "health") {
      void queryClient.query(healthOptions(path, periodOfSearch(deps))).catch(noop);
    }

    // The reads that the whole page shows leave together; a failed one does not stop the loader: the page reads it
    // again and shows what failed.
    await Promise.allSettled([
      queryClient.query(repoOptions(path.org, path.repo)),
      queryClient.query(queueOptions(path)),
      queryClient.query(queueGraphOptions(path)),
      queryClient.query(queueItemsOptions(path, "live", now)),
    ]);
  },
  component: QueuePage,
  pendingComponent: QueueSkeleton,
  errorComponent: QueuePageError,
  notFoundComponent: NotFoundPage,
});

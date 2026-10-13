import { createFileRoute, notFound } from "@tanstack/react-router";

import { NotFoundPage } from "@/components/errors/not-found-page";
import { orgQueuesOptions } from "@/domain/queue/queries";
import { RepoQueuesPageError } from "./error";
import { RepoQueuesPageLoading } from "./loading";
import { RepoQueuesPage } from "./page";

export const Route = createFileRoute("/orgs/$org/repos/$repo/")({
  // A repository without a queue is not found; the layout of the organization says why it cannot read them.
  loader: async ({ context: { queryClient }, params: { org, repo } }) => {
    const list = await queryClient.query(orgQueuesOptions(org)).catch(() => null);
    if (list && !list.queues.some((queue) => queue.repo === repo)) {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- the router stops on a thrown notFound
      throw notFound();
    }
  },
  component: RepoQueuesPage,
  pendingComponent: RepoQueuesPageLoading,
  errorComponent: RepoQueuesPageError,
  notFoundComponent: NotFoundPage,
});

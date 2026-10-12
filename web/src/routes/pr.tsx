import { createFileRoute, notFound } from "@tanstack/react-router";

import { prReads } from "@/domain/pr/queries";
import { isProblem } from "@/domain/session/session";
import { startReads } from "@/lib/queryClient";
import { PrDrawer, PrDrawerError, PrNotFound } from "@/pages/PrDrawer";
import { PrDrawerSkeleton } from "@/pages/skeletons";

/** The page of a PR: the page of its queue, with the drawer of the PR open. */
export const Route = createFileRoute("/orgs/$org/repos/$repo/queues/$queue/prs/$number")({
  params: {
    parse: ({ number }) => {
      const parsed = Number(number);
      if (!Number.isSafeInteger(parsed) || parsed <= 0) {
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- the router stops on a thrown notFound
        throw notFound();
      }
      return { number: parsed };
    },
    stringify: ({ number }) => ({ number: String(number) }),
  },
  // A PR that the queue never had answers 404: the drawer says so. Another failure shows in the drawer.
  loader: async ({ context: { queryClient }, params: { org, repo, queue, number } }) => {
    const [pr, events] = prReads({ org, repo, queue }, number);
    const readEvents = startReads(queryClient.query(events));
    const isMissing = await queryClient.query(pr).then(
      () => false,
      (error: unknown) => isProblem(error, "pr_not_found"),
    );

    await readEvents;
    if (isMissing) {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- the router stops on a thrown notFound
      throw notFound();
    }
  },
  component: PrDrawer,
  pendingComponent: PrDrawerSkeleton,
  errorComponent: PrDrawerError,
  notFoundComponent: PrNotFound,
});

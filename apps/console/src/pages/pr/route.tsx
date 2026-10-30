import { noop } from "@tanstack/react-query";
import { createFileRoute, notFound } from "@tanstack/react-router";

import { prEventsOptions, prOptions, runOptions } from "@/domain/pr/queries";
import { failureOf, isProblem } from "@/domain/problem";
import { PrDrawerError } from "./error";
import { PrDrawerLoading } from "./loading";
import { PrNotFound } from "./not-found";
import { PrDrawer } from "./page";

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
  loader: async ({ context: { queryClient }, params: { org, repo, queue, number } }) => {
    const path = { org, repo, queue };

    // The drawer shows the PR and its history together; a failure other than a 404 shows in the drawer.
    const [readPr] = await Promise.allSettled([
      queryClient.query(prOptions(path, number)),
      queryClient.query(prEventsOptions(path, number)),
    ]);

    // A PR that the queue never had answers 404: the drawer says so.
    if (readPr.status === "rejected" && isProblem(failureOf(readPr.reason), "pr_not_found")) {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- the router stops on a thrown notFound
      throw notFound();
    }

    // The run loads under its own section, as soon as the PR names it.
    if (readPr.status === "fulfilled" && readPr.value.run_id) {
      void queryClient.query(runOptions(path, readPr.value.run_id)).catch(noop);
    }
  },
  component: PrDrawer,
  pendingComponent: PrDrawerLoading,
  errorComponent: PrDrawerError,
  notFoundComponent: PrNotFound,
});

import { useQueryErrorResetBoundary } from "@tanstack/react-query";
import { type ErrorComponentProps, createFileRoute, notFound } from "@tanstack/react-router";
import { useEffect } from "react";

import { getPrOptions } from "../client/@tanstack/react-query.gen";
import { ServerErrorPage } from "../pages/ServerErrorPage";
import { DrawerFrame } from "../queue/DrawerFrame";
import { PrDrawer } from "../queue/PrDrawer";
import { isProblem } from "../session";

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
    // A PR that is not in the queue answers 404: the drawer says so.
    await queryClient
      .query(getPrOptions({ path: { org, repo, queue, number }, query: { include: "timeline" } }))
      .catch(() => undefined);
  },
  component: PrRoute,
  errorComponent: PrError,
});

function PrRoute() {
  const { org, repo, queue, number } = Route.useParams();

  return <PrDrawer path={{ org, repo, queue }} number={number} />;
}

/** A PR that the queue never had answers 404: the drawer says so over the page of the queue. */
function PrError(props: ErrorComponentProps) {
  const { org, repo, queue, number } = Route.useParams();
  const queries = useQueryErrorResetBoundary();

  // The next visit of the PR reads it again.
  useEffect(() => queries.reset, [queries]);

  if (!isProblem(props.error, "not_found")) {
    return <ServerErrorPage {...props} />;
  }

  return <DrawerFrame path={{ org, repo, queue }} title={`#${String(number)} is not in this queue`} />;
}

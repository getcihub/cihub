import { type ErrorComponentProps, Link, createFileRoute, notFound } from "@tanstack/react-router";

import { getPrOptions } from "../client/@tanstack/react-query.gen";
import { ServerErrorPage } from "../pages/ServerErrorPage";
import { PrDrawer } from "../queue/PrDrawer";
import styles from "../queue/PrDrawer.module.css";
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

  if (!isProblem(props.error, "not_found")) {
    return <ServerErrorPage {...props} />;
  }

  return (
    <aside className={styles.drawer} role="dialog" aria-labelledby="pr-title">
      <header>
        <Link
          className={styles.close}
          to="/orgs/$org/repos/$repo/queues/$queue"
          params={{ org, repo, queue }}
          search={(kept) => kept}
        >
          Close
        </Link>
        <h2 id="pr-title">#{number} is not in this queue</h2>
      </header>
    </aside>
  );
}

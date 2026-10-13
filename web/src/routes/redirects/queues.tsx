import { createFileRoute, redirect } from "@tanstack/react-router";

/** `…/queues` lists nothing of its own: the page of the repository lists its queues. */
export const Route = createFileRoute("/orgs/$org/repos/$repo/queues/")({
  beforeLoad: ({ params: { org, repo } }) => {
    // eslint-disable-next-line @typescript-eslint/only-throw-error -- the router stops on a thrown redirect
    throw redirect({ to: "/orgs/$org/repos/$repo", params: { org, repo } });
  },
});

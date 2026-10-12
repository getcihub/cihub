import { createFileRoute, redirect } from "@tanstack/react-router";

/** `/orgs/$org/repos` lists nothing of its own: the page of the organization lists its queues by repository. */
export const Route = createFileRoute("/orgs/$org/repos/")({
  beforeLoad: ({ params: { org } }) => {
    // eslint-disable-next-line @typescript-eslint/only-throw-error -- the router stops on a thrown redirect
    throw redirect({ to: "/orgs/$org", params: { org } });
  },
});

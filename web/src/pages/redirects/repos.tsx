import { createFileRoute, redirect } from "@tanstack/react-router";

import { orgSearch } from "@/domain/setup/search";

/** `/orgs/$org/repos` lists nothing of its own: the page of the organization lists its repositories. */
export const Route = createFileRoute("/orgs/$org/repos/")({
  validateSearch: orgSearch,
  beforeLoad: ({ params: { org }, search }) => {
    // eslint-disable-next-line @typescript-eslint/only-throw-error -- the router stops on a thrown redirect
    throw redirect({ to: "/orgs/$org", params: { org }, search });
  },
});

import { createFileRoute } from "@tanstack/react-router";

import { reposOptions } from "@/domain/setup/queries";
import { orgSearch } from "@/domain/setup/search";
import { OrgQueuesPageError } from "./error";
import { OrgQueuesPageLoading } from "./loading";
import { OrgQueuesPage } from "./page";

export const Route = createFileRoute("/orgs/$org/")({
  validateSearch: orgSearch,
  // The queues come with the layout of the organization; the repositories with the page. A search reads its own
  // list in the page, which keeps the field while it loads.
  loader: async ({ context: { queryClient }, params: { org } }) => {
    await queryClient.query(reposOptions(org, undefined));
  },
  // Another organization waits for its own reads.
  remountDeps: ({ params }) => params,
  component: OrgQueuesPage,
  pendingComponent: OrgQueuesPageLoading,
  errorComponent: OrgQueuesPageError,
});

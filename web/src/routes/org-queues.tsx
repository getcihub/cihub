import { createFileRoute } from "@tanstack/react-router";

import { OrgQueuesPage, OrgQueuesPageError } from "@/pages/OrgQueuesPage";
import { OrgQueuesPageSkeleton } from "@/pages/skeletons";

export const Route = createFileRoute("/orgs/$org/")({
  component: OrgQueuesPage,
  pendingComponent: OrgQueuesPageSkeleton,
  errorComponent: OrgQueuesPageError,
});

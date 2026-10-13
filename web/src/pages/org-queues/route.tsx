import { createFileRoute } from "@tanstack/react-router";

import { OrgQueuesPageError } from "./error";
import { OrgQueuesPageLoading } from "./loading";
import { OrgQueuesPage } from "./page";

export const Route = createFileRoute("/orgs/$org/")({
  component: OrgQueuesPage,
  pendingComponent: OrgQueuesPageLoading,
  errorComponent: OrgQueuesPageError,
});

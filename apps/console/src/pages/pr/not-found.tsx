import { getRouteApi } from "@tanstack/react-router";

import { DrawerFrame } from "@/pages/pr/components/drawer-frame";

const route = getRouteApi("/orgs/$org/repos/$repo/queues/$queue/prs/$number");

/** A PR that the queue never had: the drawer says so over the page of the queue. */
export function PrNotFound() {
  const { org, repo, queue, number } = route.useParams();

  return <DrawerFrame path={{ org, repo, queue }} title={`#${String(number)} is not in this queue`} />;
}

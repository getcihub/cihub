import { getRouteApi } from "@tanstack/react-router";

import { QueuesSkeleton } from "@/components/queue/queues-skeleton";

const route = getRouteApi("/orgs/$org/");

export function OrgQueuesPageLoading() {
  const { org } = route.useParams();

  return <QueuesSkeleton where={org} title="Repositories" />;
}

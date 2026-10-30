import { getRouteApi } from "@tanstack/react-router";

import { QueuesSkeleton } from "@/pages/repo-queues/components/queues-skeleton";

const route = getRouteApi("/orgs/$org/repos/$repo/");

export function RepoQueuesPageLoading() {
  const { org, repo } = route.useParams();

  return <QueuesSkeleton where={org} title={repo} />;
}

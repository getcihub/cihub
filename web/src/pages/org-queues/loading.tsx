import { getRouteApi } from "@tanstack/react-router";

import { ReposSkeleton } from "@/components/setup/repos-skeleton";

const route = getRouteApi("/orgs/$org/");

export function OrgQueuesPageLoading() {
  const { org } = route.useParams();

  return <ReposSkeleton org={org} />;
}

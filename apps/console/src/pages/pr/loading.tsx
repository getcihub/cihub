import { getRouteApi } from "@tanstack/react-router";

import { LoadingStatus, Skeleton } from "@cihub/ui/components/skeleton";
import { DrawerFrame, DrawerSection } from "@/pages/pr/components/drawer-frame";
import { RunSectionSkeleton } from "@/pages/pr/components/run-section";

const route = getRouteApi("/orgs/$org/repos/$repo/queues/$queue/prs/$number");

/** The drawer while the PR loads, over the page of its queue. */
export function PrDrawerLoading() {
  const { org, repo, queue } = route.useParams();

  return (
    <DrawerFrame
      path={{ org, repo, queue }}
      meta={<Skeleton className="mt-1 w-30" />}
      title={<Skeleton className="h-5 w-60" />}
      head={<Skeleton className="h-5 w-20 rounded-full" />}
    >
      <LoadingStatus />
      <DrawerSection>
        <Skeleton className="h-10" />
      </DrawerSection>
      <RunSectionSkeleton />
    </DrawerFrame>
  );
}

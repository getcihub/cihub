// The pages while they load. A route keeps its pending component in the entry chunk of the dashboard, which
// `autoCodeSplitting` does not split: a skeleton never imports its page, so that the page stays in its own chunk.
import { getRouteApi, useMatchRoute } from "@tanstack/react-router";

import { TopBar, TopBarLogo } from "@/components/frame/Frame";
import { DrawerFrame, DrawerSection } from "@/components/pr/DrawerFrame";
import { RunSectionSkeleton } from "@/components/pr/RunSection";
import { OverviewHead } from "@/components/queue/OverviewHead";
import { QueueCardsSkeleton } from "@/components/queue/QueueCardsSkeleton";
import { QueueSkeleton } from "@/components/queue/QueueSkeleton";
import { Page } from "@/ui/page";
import { LoadingStatus, Skeleton } from "@/ui/skeleton";

const orgRoute = getRouteApi("/orgs/$org");
const orgQueuesRoute = getRouteApi("/orgs/$org/");
const repoQueuesRoute = getRouteApi("/orgs/$org/repos/$repo/");
const prRoute = getRouteApi("/orgs/$org/repos/$repo/queues/$queue/prs/$number");

/**
 * The organization loads: the top bar, then the skeleton of the page that the address opens, as the frame and
 * the page will show it.
 */
export function OrgLayoutPending() {
  const { org } = orgRoute.useParams();
  const matchRoute = useMatchRoute();
  // The page of a queue, or a PR over it.
  const isQueue = matchRoute({ to: "/orgs/$org/repos/$repo/queues/$queue", fuzzy: true }) !== false;

  return (
    <>
      <TopBar>
        <TopBarLogo />
      </TopBar>
      <Page>
        {isQueue ? (
          <QueueSkeleton />
        ) : (
          <>
            <OverviewHead where={org} />
            <QueueCardsSkeleton />
          </>
        )}
      </Page>
    </>
  );
}

/** The queues while they load: the head of the page, then the shapes of the cards. */
export function OrgQueuesPageSkeleton() {
  const { org } = orgQueuesRoute.useParams();

  return (
    <Page>
      <OverviewHead where={org} />
      <QueueCardsSkeleton />
    </Page>
  );
}

/** The queues while they load: the head of the page, then the shapes of the cards. */
export function RepoQueuesPageSkeleton() {
  const { org, repo } = repoQueuesRoute.useParams();

  return (
    <Page>
      <OverviewHead where={`${org}/${repo}`} />
      <QueueCardsSkeleton />
    </Page>
  );
}

/** The page of a queue while it loads. */
export function QueuePageSkeleton() {
  return (
    <Page>
      <QueueSkeleton />
    </Page>
  );
}

/** The drawer while the PR loads, over the page of its queue. */
export function PrDrawerSkeleton() {
  const { org, repo, queue } = prRoute.useParams();

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

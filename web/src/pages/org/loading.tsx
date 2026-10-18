// A route keeps its loading component in the entry chunk of the dashboard, which `autoCodeSplitting` does not
// split: a loading file never imports a page, so that the page stays in its own chunk.
import { getRouteApi, useMatchRoute } from "@tanstack/react-router";

import { TopBar, TopBarLogo } from "@/components/frame/frame";
import { QueueSkeleton } from "@/components/queue/queue-skeleton";
import { QueuesSkeleton } from "@/components/queue/queues-skeleton";

const route = getRouteApi("/orgs/$org");

/**
 * The organization loads: the top bar, then the skeleton of the page that the address opens, as the frame and
 * the page will show it.
 */
export function OrgLayoutLoading() {
  const { org } = route.useParams();
  const matchRoute = useMatchRoute();
  // The page of a queue, or a PR over it.
  const isQueue = matchRoute({ to: "/orgs/$org/repos/$repo/queues/$queue", fuzzy: true }) !== false;

  return (
    <>
      <TopBar>
        <TopBarLogo />
      </TopBar>
      {isQueue ? <QueueSkeleton /> : <QueuesSkeleton where={org} title="Repositories" />}
    </>
  );
}

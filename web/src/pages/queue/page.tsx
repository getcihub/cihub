import { Outlet, getRouteApi } from "@tanstack/react-router";

import { Spotlight } from "@/components/mine/mine";
import { Page } from "@/components/ui/page";
import { ranksOf } from "@/domain/queue/queries";
import { useMinePrs } from "@/hooks/use-mine-prs";
import { useNow } from "@/hooks/use-now";
import { Banners } from "@/pages/queue/components/banners";
import { ItemTabs } from "@/pages/queue/components/item-tabs";
import { QueueGraph } from "@/pages/queue/components/queue-graph";
import { QueueHead } from "@/pages/queue/components/queue-head";
import { QuietQueue } from "@/pages/queue/components/quiet-queue";
import { Settings, Stats } from "@/pages/queue/components/summary";
import { useQueuePageData } from "./use-queue-page-data";

const route = getRouteApi("/orgs/$org/repos/$repo/queues/$queue");

/** The page of a queue: what stops it, its counters and settings, its graph and its items; a PR opens over it. */
export function QueuePage() {
  const { org, repo, queue: name } = route.useParams();
  const { tab } = route.useSearch();
  const path = { org, repo, queue: name };
  const now = useNow();
  const { queue, graph, live } = useQueuePageData(path, now);
  const spotlit = useMinePrs(org).filter((pr) => pr.repo === repo && pr.queue === name);
  const ranks = ranksOf(graph);
  // A tab that the address opens shows its items, even in a queue with nothing live.
  const isQuiet =
    tab === "live" && live.length === 0 && queue.merged_last_24h === 0 && queue.failed_last_24h === 0;

  return (
    <>
      <Page>
        <QueueHead path={path} queue={queue} />
        <Spotlight org={org} prs={spotlit} queue={queue} />
        <Banners queue={queue} org={org} repo={repo} now={now} />
        {!isQuiet && <Stats queue={queue} now={now} />}
        <Settings queue={queue} />
        {isQuiet ? (
          <QuietQueue queue={queue} />
        ) : (
          <>
            <QueueGraph queue={queue} graph={graph} items={live} ranks={ranks} />
            <ItemTabs path={path} tab={tab} queue={queue} live={live} ranks={ranks} now={now} />
          </>
        )}
      </Page>
      <Outlet />
    </>
  );
}

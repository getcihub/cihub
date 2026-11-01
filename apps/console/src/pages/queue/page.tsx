import { Outlet, getRouteApi } from "@tanstack/react-router";
import { Suspense } from "react";

import { Card } from "@cihub/ui/components/card";
import { Page } from "@cihub/ui/components/page";
import { periodOfSearch } from "@/domain/queue/period";
import { ranksOf } from "@/domain/queue/queries";
import { useMinePrs } from "@/hooks/use-mine-prs";
import { useNow } from "@/hooks/use-now";
import { useSentNotice } from "@/hooks/use-sent-notice";
import { EmptyQueue } from "@/pages/queue/components/graph-pane/empty-queue";
import { ItemTabs } from "@/pages/queue/components/graph-pane/item-tabs";
import { QueueGraph } from "@/pages/queue/components/graph-pane/queue-graph";
import { Spotlight } from "@/pages/queue/components/graph-pane/spotlight";
import { Settings, Stats } from "@/pages/queue/components/graph-pane/summary";
import { Banners } from "@/pages/queue/components/head/banners";
import { QueueHead } from "@/pages/queue/components/head/queue-head";
import { QueuePanes } from "@/pages/queue/components/head/queue-panes";
import { HealthPane } from "@/pages/queue/components/health-pane/health-pane";
import { HealthSkeleton } from "@/pages/queue/components/health-pane/health-skeleton";
import { useQueuePageData } from "./use-queue-page-data";

const route = getRouteApi("/orgs/$org/repos/$repo/queues/$queue");

/**
 * The page of a queue: what stops it, then its graph pane (the PRs of the person, its counters and settings, its
 * graph and its items) or its Health; a PR opens over it.
 */
export function QueuePage() {
  const { org, repo, queue: name } = route.useParams();
  const { tab, pane, ...search } = route.useSearch();
  const path = { org, repo, queue: name };
  const now = useNow();

  const { queue, graph, live } = useQueuePageData(path, now);
  const spotlit = useMinePrs(org).filter((pr) => pr.repo === repo && pr.queue === name);
  const sent = useSentNotice(queue.id);

  const ranks = ranksOf(graph);
  // A tab that the address opens shows its items, even in a queue with nothing live.
  const isQuiet =
    tab === "live" &&
    live.length === 0 &&
    queue.merged_last_24h === 0 &&
    queue.failed_or_cancelled_last_24h === 0;

  return (
    <>
      <QueueHead path={path} queue={queue} sent={sent}>
        <QueuePanes path={path} pane={pane} live={live.length} />
      </QueueHead>
      <Page>
        <Banners queue={queue} org={org} repo={repo} now={now} />
        {pane === "health" ? (
          <Suspense fallback={<HealthSkeleton />}>
            <HealthPane path={path} period={periodOfSearch(search)} branch={queue.target_branch} />
          </Suspense>
        ) : (
          <>
            <Spotlight org={org} prs={spotlit} queue={queue} />
            {!isQuiet && <Stats queue={queue} now={now} />}
            <Settings queue={queue} />
            {isQuiet ? (
              <Card>
                <EmptyQueue path={path} queue={queue} now={now} onSent={sent.showNotice} />
              </Card>
            ) : (
              <>
                <QueueGraph queue={queue} graph={graph} items={live} ranks={ranks} />
                <ItemTabs
                  path={path}
                  tab={tab}
                  queue={queue}
                  live={live}
                  ranks={ranks}
                  now={now}
                  onSent={sent.showNotice}
                />
              </>
            )}
          </>
        )}
      </Page>
      <Outlet />
    </>
  );
}

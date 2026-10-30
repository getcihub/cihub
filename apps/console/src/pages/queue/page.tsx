import { Outlet, getRouteApi } from "@tanstack/react-router";
import { Suspense } from "react";

import { Spotlight } from "@/components/mine/mine";
import { Card } from "@cihub/ui/components/card";
import { Page } from "@cihub/ui/components/page";
import { ranksOf } from "@/domain/queue/queries";
import { useMinePrs } from "@/hooks/use-mine-prs";
import { periodOfSearch } from "@/domain/queue/period";
import { useNow } from "@/hooks/use-now";
import { useSentNotice } from "@/hooks/use-sent-notice";
import { Banners } from "@/pages/queue/components/banners";
import { EmptyQueue } from "@/pages/queue/components/empty-queue";
import { HealthPane, HealthSkeleton } from "@/pages/queue/components/health-pane";
import { ItemTabs } from "@/pages/queue/components/item-tabs";
import { QueueGraph } from "@/pages/queue/components/queue-graph";
import { QueueHead } from "@/pages/queue/components/queue-head";
import { QueuePanes } from "@/pages/queue/components/queue-panes";
import { Settings, Stats } from "@/pages/queue/components/summary";
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
  const sent = useSentNotice(queue.id);
  const spotlit = useMinePrs(org).filter((pr) => pr.repo === repo && pr.queue === name);
  const ranks = ranksOf(graph);
  // A tab that the address opens shows its items, even in a queue with nothing live.
  const isQuiet =
    tab === "live" &&
    live.length === 0 &&
    queue.merged_last_24h === 0 &&
    queue.failed_or_cancelled_last_24h === 0;

  return (
    <>
      <Page>
        <QueueHead path={path} queue={queue} sent={sent} />
        <QueuePanes path={path} pane={pane} live={live.length} />
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

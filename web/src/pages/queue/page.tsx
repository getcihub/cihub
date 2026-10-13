import { Outlet, getRouteApi } from "@tanstack/react-router";

import { Spotlight } from "@/components/mine/mine";
import { Card } from "@/components/ui/card";
import { Page } from "@/components/ui/page";
import { ranksOf } from "@/domain/queue/queries";
import { useMinePrs } from "@/hooks/use-mine-prs";
import { useNow } from "@/hooks/use-now";
import { useSentNotice } from "@/hooks/use-sent-notice";
import { Banners } from "@/pages/queue/components/banners";
import { EmptyQueue } from "@/pages/queue/components/empty-queue";
import { ItemTabs } from "@/pages/queue/components/item-tabs";
import { QueueGraph } from "@/pages/queue/components/queue-graph";
import { QueueHead } from "@/pages/queue/components/queue-head";
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
  const sent = useSentNotice(queue.id);
  const spotlit = useMinePrs(org).filter((pr) => pr.repo === repo && pr.queue === name);
  const ranks = ranksOf(graph);
  // A tab that the address opens shows its items, even in a queue with nothing live.
  const isQuiet =
    tab === "live" && live.length === 0 && queue.merged_last_24h === 0 && queue.failed_last_24h === 0;

  return (
    <>
      <Page>
        <QueueHead path={path} queue={queue} sent={sent} />
        <Spotlight org={org} prs={spotlit} queue={queue} />
        <Banners queue={queue} org={org} repo={repo} now={now} />
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
      </Page>
      <Outlet />
    </>
  );
}

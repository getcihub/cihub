import { type ErrorComponentProps, Outlet, getRouteApi } from "@tanstack/react-router";

import { ServerError } from "@/components/errors/ServerError";
import { Spotlight } from "@/components/mine/Mine";
import { Banners } from "@/components/queue/Banners";
import { ItemTabs } from "@/components/queue/ItemTabs";
import { QueueGraph } from "@/components/queue/QueueGraph";
import { QueueHead } from "@/components/queue/QueueHead";
import { QuietQueue } from "@/components/queue/QuietQueue";
import { Settings, Stats } from "@/components/queue/Summary";
import { useMinePrs } from "@/domain/mine/queries";
import { ranksOf, useOrgQueues, useQueuePageData } from "@/domain/queue/queries";
import { useNow } from "@/lib/time";
import { Card } from "@/ui/card";
import { Page } from "@/ui/page";

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

/** A read of the page failed: its head stays, from the list of the queues, with what the server answered. */
export function QueuePageError({ error, reset }: ErrorComponentProps) {
  const { org, repo, queue: name } = route.useParams();
  const queue = useOrgQueues(org).find((candidate) => candidate.repo === repo && candidate.name === name);

  return (
    <Page>
      {queue && <QueueHead path={{ org, repo, queue: name }} queue={queue} />}
      <Card>
        <ServerError error={error} reset={reset} />
      </Card>
    </Page>
  );
}

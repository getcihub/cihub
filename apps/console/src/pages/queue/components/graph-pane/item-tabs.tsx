import { Link } from "@tanstack/react-router";
import { Suspense } from "react";

import type { Pr, Queue } from "@/client/types.gen";
import { ItemRowsSkeleton } from "@/components/queue/queue-skeleton";
import { CardHeader, CardHint, cardVariants } from "@cihub/ui/components/card";
import { cn } from "@cihub/ui/lib/cn";
import type { QueuePath } from "@/domain/queue/queries";
import { TABS, type Tab } from "@/domain/queue/search";
import { useTabItems } from "@/pages/queue/use-tab-items";
import { TabRows } from "./item-rows";

const TAB_LABELS: Record<Tab, string> = {
  live: "In the queue",
  merged: "Merged",
  ended: "Failed and cancelled",
};

/** The items of the open tab: the live ones in their order, the others of the last 24 hours. */
export function ItemTabs({
  path,
  tab,
  queue,
  live,
  ranks,
  now,
  onSent,
}: {
  path: QueuePath;
  tab: Tab;
  queue: Queue;
  live: Pr[];
  ranks: Map<string, number>;
  now: number;
  /** What the page shows once the empty queue submitted a pull request. */
  onSent: (notice: string) => void;
}) {
  const counts: Record<Tab, number> = {
    live: live.length,
    merged: queue.merged_last_24h,
    ended: queue.failed_or_cancelled_last_24h,
  };

  return (
    <section className={cn(cardVariants(), "min-w-0")} aria-labelledby="items-title">
      <CardHeader>
        <h2 id="items-title" className="sr-only">
          Pull requests
        </h2>
        <nav className="flex gap-1" aria-label="Tabs of the pull requests">
          {TABS.map((candidate) => (
            <Link
              key={candidate}
              className={cn(
                "rounded-sm px-2.5 py-1 text-base text-secondary-foreground hover:text-foreground",
                candidate === tab && "bg-muted text-primary",
              )}
              to="/orgs/$org/repos/$repo/queues/$queue"
              params={path}
              search={{ tab: candidate }}
              resetScroll={false}
              aria-current={candidate === tab ? "page" : undefined}
            >
              {TAB_LABELS[candidate]}
              <span className="ml-1 text-muted-foreground">{counts[candidate]}</span>
            </Link>
          ))}
        </nav>
        <Suspense fallback={null}>
          <TabHint path={path} tab={tab} now={now} />
        </Suspense>
      </CardHeader>
      <Suspense fallback={<ItemRowsSkeleton />}>
        <TabRows path={path} tab={tab} queue={queue} ranks={ranks} now={now} onSent={onSent} />
      </Suspense>
    </section>
  );
}

/** How the open tab orders its items, and whether its read stopped at 100. */
function TabHint({ path, tab, now }: { path: QueuePath; tab: Tab; now: number }) {
  const { has_more: isCut } = useTabItems(path, tab, now);

  return (
    <CardHint className="max-md:hidden">
      {tab === "live" &&
        (isCut ? "By position, the first 100 only" : "By position: priority first, then arrival")}
      {tab !== "live" && (isCut ? "Last 24 hours, the 100 latest submissions only" : "Last 24 hours")}
    </CardHint>
  );
}

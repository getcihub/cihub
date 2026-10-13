import { Link } from "@tanstack/react-router";
import { Suspense, useRef } from "react";

import type { Pr, Queue } from "@/client/types.gen";
import { ItemStateBadge } from "@/components/pr/item-state-badge";
import { ItemRowsSkeleton } from "@/components/queue/queue-skeleton";
import { Avatar } from "@/components/ui/avatar";
import { CardHeader, CardHint, cardVariants } from "@/components/ui/card";
import { useFlip } from "@/components/ui/hooks/use-flip";
import { Prose } from "@/components/ui/prose";
import { type QueuePath, TABS, type Tab } from "@/domain/queue/queries";
import { cn } from "@/lib/cn";
import { shortSha } from "@/lib/text";
import { durationBetween, timeAgo } from "@/lib/time";
import { useTabItems } from "@/pages/queue/use-tab-items";
import { PrLink } from "./pr-link";

const DAY = 24 * 60 * 60 * 1000;

const TAB_LABELS: Record<Tab, string> = {
  live: "In the queue",
  merged: "Merged",
  ended: "Failed and cancelled",
};

const NONE: Record<Tab, string> = {
  live: "No pull request in the queue.",
  merged: "Nothing merged in the last 24 hours.",
  ended: "Nothing failed or cancelled in the last 24 hours.",
};

const PRIORITIES: Record<number, string> = { 0: "urgent", 10: "high", 100: "medium", 200: "low" };

const PRIORITY_TONES: Record<string, string> = {
  urgent: "font-medium text-destructive",
  high: "font-medium text-warning",
};

const HEAD_CELL = "border-b border-divider px-4 py-2 text-left text-md font-medium text-secondary-foreground";

const CELL = "px-4 py-2.25";

/** The priority of `item` by its name, "urgent" and "high" in color; a value that has no name, as a number. */
function Priority({ item }: { item: Pr }) {
  const name = PRIORITIES[item.priority];

  return (
    <span className={cn("text-base text-secondary-foreground", name && PRIORITY_TONES[name])}>
      {name ?? item.priority}
    </span>
  );
}

/** The sentence of the API, and for a merge where and after how long it landed. */
function Detail({ item }: { item: Pr }) {
  const landedSha = item.state === "merged" ? item.merged_sha : null;

  return (
    <>
      <Prose text={item.state_description} />
      {landedSha && (
        <span className="text-muted-foreground">
          {" "}
          · <code>{shortSha(landedSha)}</code> · after{" "}
          {durationBetween(item.submitted_at, item.state_changed_at)}
        </span>
      )}
    </>
  );
}

/** The items of the open tab: the live ones in their order, the others of the last 24 hours. */
export function ItemTabs({
  path,
  tab,
  queue,
  live,
  ranks,
  now,
}: {
  path: QueuePath;
  tab: Tab;
  queue: Queue;
  live: Pr[];
  ranks: Map<string, number>;
  now: number;
}) {
  const counts: Partial<Record<Tab, number>> = { live: live.length, merged: queue.merged_last_24h };

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
                "rounded-sm px-2.5 py-1 text-base font-medium text-secondary-foreground hover:text-foreground",
                candidate === tab && "bg-muted text-primary",
              )}
              to="/orgs/$org/repos/$repo/queues/$queue"
              params={path}
              search={{ tab: candidate }}
              aria-current={candidate === tab ? "page" : undefined}
            >
              {TAB_LABELS[candidate]}
              {counts[candidate] !== undefined && (
                <span className="ml-1 text-muted-foreground">{counts[candidate]}</span>
              )}
            </Link>
          ))}
        </nav>
        <Suspense fallback={null}>
          <TabHint path={path} tab={tab} now={now} />
        </Suspense>
      </CardHeader>
      <Suspense fallback={<ItemRowsSkeleton />}>
        <TabRows path={path} tab={tab} ranks={ranks} now={now} />
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

/** The rows of the open tab: its read is the one of the live items for the tab of the live ones. */
function TabRows({
  path,
  tab,
  ranks,
  now,
}: {
  path: QueuePath;
  tab: Tab;
  ranks: Map<string, number>;
  now: number;
}) {
  const items = useTabItems(path, tab, now).prs;
  const rowsRef = useRef<HTMLTableSectionElement>(null);
  useFlip(rowsRef);
  const rows = tab === "live" ? items : items.filter((item) => now - Date.parse(item.state_changed_at) < DAY);

  return rows.length === 0 ? (
    <p className="p-7 text-center text-secondary-foreground">{NONE[tab]}</p>
  ) : (
    <table className="w-full">
      <thead>
        <tr>
          <th className={HEAD_CELL}>#</th>
          <th className={HEAD_CELL}>Pull request</th>
          <th className={HEAD_CELL}>State</th>
          <th className={cn(HEAD_CELL, "max-md:hidden")}>Details</th>
          <th className={HEAD_CELL}>Priority</th>
          <th className={HEAD_CELL}>{tab === "live" ? "Since" : "When"}</th>
        </tr>
      </thead>
      <tbody ref={rowsRef}>
        {rows.map((item) => (
          <tr key={item.id} className="border-b border-divider last:border-b-0" data-flip={item.id}>
            <td
              className={cn(CELL, "w-8 text-md font-semibold text-muted-foreground tabular-nums")}
              data-testid="queue-position"
            >
              {tab === "live" ? (ranks.get(item.id) ?? "–") : ""}
            </td>
            <td className={CELL}>
              <div className="flex items-center gap-2.25">
                <Avatar name={item.author} />
                <div>
                  <PrLink className="font-medium text-inherit hover:underline" number={item.number}>
                    {item.title ?? `#${String(item.number)}`}
                  </PrLink>
                  <div className="text-sm text-muted-foreground">
                    #{item.number} · {item.author}
                  </div>
                </div>
              </div>
            </td>
            <td className={CELL}>
              <ItemStateBadge state={item.state} />
            </td>
            <td className={cn(CELL, "max-w-95 text-md text-secondary-foreground max-md:hidden")}>
              <Detail item={item} />
            </td>
            <td className={CELL}>
              <Priority item={item} />
            </td>
            <td className={cn(CELL, "text-md whitespace-nowrap text-muted-foreground")}>
              {timeAgo(tab === "live" ? item.submitted_at : item.state_changed_at, now)}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

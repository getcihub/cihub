import { useRef } from "react";

import type { Pr, Queue } from "@/client/types.gen";
import { ItemStateBadge } from "@/components/pr/item-state-badge";
import { Avatar } from "@cihub/ui/components/avatar";
import { Prose } from "@cihub/ui/components/prose";
import { useFlip } from "@cihub/ui/hooks/use-flip";
import { cn } from "@cihub/ui/lib/cn";
import type { QueuePath } from "@/domain/queue/queries";
import { type Tab } from "@/domain/queue/search";
import { shortSha } from "@/lib/text";
import { durationBetween, timeAgo } from "@/lib/time";
import { useTabItems } from "@/pages/queue/use-tab-items";
import { EmptyQueue } from "./empty-queue";
import { PrLink } from "./pr-link";

const DAY = 24 * 60 * 60 * 1000;

const NONE: Record<Exclude<Tab, "live">, string> = {
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

/**
 * The priority of `item` by its name, "urgent" and "high" in color; a value that has no name, as a number. Then
 * its profile, whose settings it takes.
 */
function Priority({ item }: { item: Pr }) {
  const name = PRIORITIES[item.priority];

  return (
    <>
      <span className={cn("text-base text-secondary-foreground", name && PRIORITY_TONES[name])}>
        {name ?? item.priority}
      </span>
      {item.profile && <span className="text-base text-muted-foreground"> · {item.profile}</span>}
    </>
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

/**
 * The rows of the open tab: its read is the one of the live items for the tab of the live ones. With no live item,
 * the tab says how a pull request enters the queue.
 */
export function TabRows({
  path,
  tab,
  queue,
  ranks,
  now,
  onSent,
}: {
  path: QueuePath;
  tab: Tab;
  queue: Queue;
  ranks: Map<string, number>;
  now: number;
  onSent: (notice: string) => void;
}) {
  const items = useTabItems(path, tab, now).prs;

  const rowsRef = useRef<HTMLTableSectionElement>(null);
  useFlip(rowsRef);

  const rows = tab === "live" ? items : items.filter((item) => now - Date.parse(item.state_changed_at) < DAY);

  if (rows.length === 0) {
    return tab === "live" ? (
      <EmptyQueue path={path} queue={queue} now={now} onSent={onSent} />
    ) : (
      <p className="p-7 text-center text-secondary-foreground">{NONE[tab]}</p>
    );
  }

  return (
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
                <Avatar name={item.author.login} src={item.author.avatar_url} />
                <div>
                  <PrLink className="font-medium text-inherit hover:underline" number={item.number}>
                    {item.title ?? `#${String(item.number)}`}
                  </PrLink>
                  <div className="text-sm text-muted-foreground">
                    #{item.number} · {item.author.login}
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

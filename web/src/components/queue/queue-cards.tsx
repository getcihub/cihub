import { Link } from "@tanstack/react-router";
import { useRef, useState } from "react";

import type { Queue } from "@/client/types.gen";
import { cardVariants } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { useFlip } from "@/components/ui/motion";
import { inQueueCount, testingCount } from "@/domain/queue/counts";
import { useNow } from "@/hooks/use-now";
import { cn } from "@/lib/cn";
import { timeAgo } from "@/lib/time";
import { QueueStateBadge } from "./queue-state-badge";

/** From this many queues, a filter helps to find one. */
const FILTER_FROM = 8;

/** A queue that someone must look at: it does not run, or a PR in it failed its tests. */
function needsAttention(queue: Queue): boolean {
  return (
    queue.access === "suspended" ||
    Boolean(queue.config_error) ||
    queue.state !== "running" ||
    queue.counts.pending_failure > 0
  );
}

/** The cards of `queues`, the ones that need someone first, then in the order of the API. */
export function QueueCards({ org, queues }: { org: string; queues: Queue[] }) {
  const [filter, setFilter] = useState("");
  const hasFilter = queues.length >= FILTER_FROM;
  const words = hasFilter ? filter.trim().toLowerCase() : "";
  const shown = queues
    .filter((queue) => `${queue.repo} ${queue.name} ${queue.target_branch}`.toLowerCase().includes(words))
    .sort((a, b) => Number(needsAttention(b)) - Number(needsAttention(a)));
  const now = useNow();
  const grid = useRef<HTMLDivElement>(null);
  useFlip(grid);

  return (
    <>
      {hasFilter && (
        <Input
          className="h-7 w-65 justify-self-end px-2.5 text-base"
          type="search"
          aria-label="Filter the queues"
          placeholder="Filter by repository or branch"
          value={filter}
          onChange={(event) => {
            setFilter(event.target.value);
          }}
        />
      )}
      {shown.length > 0 ? (
        <div ref={grid} className="grid grid-cols-[repeat(auto-fill,minmax(300px,1fr))] gap-3">
          {shown.map((queue) => (
            <QueueCard key={queue.id} org={org} queue={queue} now={now} />
          ))}
        </div>
      ) : (
        <p className={cn(cardVariants(), "p-7 text-center text-secondary-foreground")}>
          No queue matches “{filter.trim()}”.
        </p>
      )}
    </>
  );
}

function QueueCard({ org, queue, now }: { org: string; queue: Queue; now: number }) {
  const { repo, counts, merged_last_24h, last_merged_at } = queue;

  return (
    <Link
      data-flip={queue.id}
      className={cn(
        cardVariants(),
        "grid content-start gap-3 px-4 py-3.5 text-inherit transition-[border-color,translate] duration-150 hover:-translate-y-px hover:border-primary-border focus-visible:border-primary-border motion-reduce:hover:translate-y-0",
      )}
      to="/orgs/$org/repos/$repo/queues/$queue"
      params={{ org, repo, queue: queue.name }}
    >
      <div className="flex items-center justify-between gap-2.5">
        <div>
          <div className="text-sm text-muted-foreground">
            {org}/{repo}
          </div>
          <h2 className="text-2xl font-medium">{queue.name}</h2>
        </div>
        <QueueStateBadge queue={queue} />
      </div>
      <div className="flex gap-5.5">
        <Counter value={inQueueCount(counts)} label="in the queue" />
        <Counter value={testingCount(counts)} label="testing" />
        <Counter value={merged_last_24h} label="merged in 24 h" />
      </div>
      <div className="text-md text-muted-foreground">
        {counts.pending_failure > 0 && (
          <>
            <span className="text-warning">{counts.pending_failure} with failed tests</span> ·{" "}
          </>
        )}
        {last_merged_at ? `last merge ${timeAgo(last_merged_at, now)}` : "no merge yet"}
      </div>
    </Link>
  );
}

function Counter({ value, label }: { value: number; label: string }) {
  return (
    <div className="grid">
      <b className="text-4xl font-medium tabular-nums">{value}</b>
      <span className="text-xs text-muted-foreground">{label}</span>
    </div>
  );
}

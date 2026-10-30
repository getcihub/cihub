import { Link } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { GraphIcon } from "@cihub/icons/graph";
import { PulseIcon } from "@cihub/icons/pulse";
import { cn } from "@cihub/ui/lib/cn";
import { PANES, type Pane, type QueuePath } from "@/domain/queue/queries";

const PANE_LABELS: Record<Pane, { label: string; icon: ReactNode }> = {
  graph: { label: "Graph", icon: <GraphIcon /> },
  health: { label: "Health", icon: <PulseIcon /> },
};

/** The panes of a queue: the queue now, with its live items, and what it did over a period. */
export function QueuePanes({ path, pane, live }: { path: QueuePath; pane: Pane; live: number }) {
  return (
    <nav className="flex gap-5 border-b border-border" aria-label="Queue">
      {PANES.map((candidate) => (
        <Link
          key={candidate}
          className={cn(
            "-mb-px inline-flex items-center gap-1.75 border-b-2 border-transparent px-0.5 pt-1.5 pb-2.25 font-medium whitespace-nowrap text-secondary-foreground hover:text-foreground [&>svg]:size-3.5 [&>svg]:flex-none",
            candidate === pane && "border-primary text-foreground",
          )}
          to="/orgs/$org/repos/$repo/queues/$queue"
          params={path}
          search={{ pane: candidate }}
          resetScroll={false}
          aria-current={candidate === pane ? "page" : undefined}
        >
          {PANE_LABELS[candidate].icon}
          {PANE_LABELS[candidate].label}
          {candidate === "graph" && (
            <span className="text-sm font-normal text-muted-foreground tabular-nums">{live}</span>
          )}
        </Link>
      ))}
    </nav>
  );
}

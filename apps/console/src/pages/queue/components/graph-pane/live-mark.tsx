import { useSyncExternalStore } from "react";

import type { Queue } from "@/client/types.gen";
import { cn } from "@cihub/ui/lib/cn";
import { isStreamOpen, onStreamOpenChanged } from "@/domain/queue/live";

/**
 * Whether the graph moves: the stream brings the changes, and the queue tests and merges. A draining queue still
 * merges the PRs in flight; a paused one, or one without access, merges nothing.
 */
export function LiveMark({ queue }: { queue: Queue }) {
  const isStreamLive = useSyncExternalStore(onStreamOpenChanged, isStreamOpen);

  const isLive = isStreamLive && queue.state !== "paused" && !queue.is_suspended;

  return (
    <span
      className={cn(
        "flex items-center gap-1.5 text-xs font-semibold uppercase",
        isLive ? "text-success" : "text-muted-foreground",
      )}
    >
      <span
        className={cn(
          "size-1.5 rounded-full",
          isLive ? "bg-success shadow-live motion-safe:animate-live" : "bg-muted-foreground",
        )}
      />
      {isLive ? "Live" : "Not live"}
    </span>
  );
}

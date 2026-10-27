import type { Queue } from "@/client/types.gen";

/** A queue that someone must look at: it does not run, or a PR in it failed its tests. */
export function needsAttention(queue: Queue): boolean {
  return (
    queue.is_suspended ||
    Boolean(queue.config_error) ||
    queue.state !== "running" ||
    queue.counts.pending_failure > 0
  );
}

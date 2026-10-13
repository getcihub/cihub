import type { Queue } from "@/client/types.gen";
import { Card } from "@/ui/card";
import { EmptyState } from "@/ui/empty-state";

/** A queue with nothing live, and nothing merged or failed in the last 24 hours: how a PR enters it. */
export function QuietQueue({ queue }: { queue: Queue }) {
  return (
    <Card>
      <EmptyState illustration={null} title="No pull request in the queue">
        Nothing waits in the queue of <code>{queue.target_branch}</code>, and nothing merged or failed in the
        last 24 hours. A pull request enters when someone comments <code>/cihub merge</code> on it.
      </EmptyState>
    </Card>
  );
}

import type { Queue } from "@/client/types.gen";
import { COMMAND, CopyCommand } from "@/components/queue/copy-command";
import { EmptyState } from "@/components/ui/empty-state";
import { QueueIllustration } from "@/components/ui/illustrations";
import type { QueuePath } from "@/domain/queue/queries";
import { timeAgo } from "@/lib/time";
import { SubmitPr } from "./submit-pr";

/** A queue with no pull request: when it last merged, and how a pull request enters it: a command, else a label. */
export function EmptyQueue({
  path,
  queue,
  now,
  onSent,
}: {
  path: QueuePath;
  queue: Queue;
  now: number;
  onSent: (notice: string) => void;
}) {
  const label = queue.settings.enqueue_label;
  const lastMerge = queue.last_merged_at
    ? `; the last merge was ${timeAgo(queue.last_merged_at, now)}`
    : ", and nothing merged here yet";

  return (
    <EmptyState
      illustration={<QueueIllustration />}
      title="No pull request in the queue"
      actions={
        <>
          <SubmitPr path={path} isDisabled={queue.is_suspended} onSent={onSent} />
          <CopyCommand />
        </>
      }
      foot={
        label && (
          <>
            Or add the label <code>{label}</code> to the pull request.
          </>
        )
      }
    >
      Nothing waits in the queue of <code>{queue.target_branch}</code>
      {lastMerge}. A pull request enters when someone comments <code>{COMMAND}</code> on it.
    </EmptyState>
  );
}

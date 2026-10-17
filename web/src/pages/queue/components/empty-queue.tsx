import { useEffect, useState } from "react";

import type { Queue } from "@/client/types.gen";
import { CopyIcon } from "@/components/icons/copy-icon";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { QueueIllustration } from "@/components/ui/illustrations";
import { Toast } from "@/components/ui/toast";
import type { QueuePath } from "@/domain/queue/queries";
import { timeAgo } from "@/lib/time";
import { SubmitPr } from "./submit-pr";

const COMMAND = "/cihub merge";

/** How long the button says what the copy did, and the toast says that it copied. */
const RESULT_SHOWN_MS = 2_000;

/** The command that submits a PR, and a button that copies it. */
function CopyCommand() {
  const [result, setResult] = useState<"copied" | "failed" | null>(null);

  useEffect(() => {
    if (!result) {
      return;
    }

    const timer = setTimeout(() => {
      setResult(null);
    }, RESULT_SHOWN_MS);

    return () => {
      clearTimeout(timer);
    };
  }, [result]);

  return (
    <span className="inline-flex items-center gap-2.5 rounded-sm border border-border bg-muted py-1 pr-1 pl-3 font-mono text-base">
      <span className="select-all">{COMMAND}</span>
      <Button
        className="h-6 px-2 font-sans text-md"
        onClick={() => {
          // A page served over plain HTTP has no clipboard: the command stays selectable.
          void Promise.resolve()
            .then(() => navigator.clipboard.writeText(COMMAND))
            .then(
              () => {
                setResult("copied");
              },
              () => {
                setResult("failed");
              },
            );
        }}
      >
        <CopyIcon />
        {result === "copied" ? "Copied" : result === "failed" ? "Copy failed" : "Copy"}
      </Button>
      <Toast className="font-sans">{result === "copied" && <>Copied {COMMAND} to the clipboard.</>}</Toast>
    </span>
  );
}

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

import { useState } from "react";

import { cancelMutation, restartMutation } from "@/client/@tanstack/react-query.gen";
import type { Pr } from "@/client/types.gen";
import { Button, Modal, ModalDescription, ModalFooter } from "@cihub/ui";
import { refusalOf } from "@/domain/queue/actions";
import type { QueuePath } from "@/domain/queue/queries";
import { useQueueMutation } from "@/hooks/use-queue-mutation";

// The small buttons of the head of the drawer, a size that no other page has.
const SMALL_BUTTON = "h-6 px-2 text-md";

/** Restarts the tests of a PR, or takes it out of the queue: the queue decides at its next reconciliation. */
export function PrActions({
  path,
  item,
  onSent,
}: {
  path: QueuePath;
  item: Pr;
  onSent: (notice: string) => void;
}) {
  const [isCancelling, setIsCancelling] = useState(false);
  const restart = useQueueMutation(restartMutation());
  const cancel = useQueueMutation(cancelMutation());
  const target = { path: { org: path.org, repo: path.repo, number: item.number } };
  const canRestart = item.state === "testing" || item.state === "pending_failure";
  const isTested = canRestart || item.state === "tests_passed";
  const error = restart.error ?? cancel.error;

  return (
    <div className="flex justify-end gap-2">
      {canRestart && (
        <Button
          className={SMALL_BUTTON}
          isPending={restart.isPending}
          pendingText="Restarting"
          onClick={() => {
            restart.mutate(target, {
              onSuccess: () => {
                onSent("Restart sent: the queue tests the pull request again at its next reconciliation.");
              },
            });
          }}
        >
          Restart tests
        </Button>
      )}
      <Button
        className={SMALL_BUTTON}
        variant="destructive-outline"
        onClick={() => {
          cancel.reset();
          setIsCancelling(true);
        }}
      >
        Cancel
      </Button>
      {error && (
        <p className="text-md text-destructive" role="alert">
          {refusalOf(error)}
        </p>
      )}
      {isCancelling && (
        <Modal
          title={`Take #${String(item.number)} out of the queue?`}
          onClose={() => {
            setIsCancelling(false);
          }}
        >
          {isTested && (
            <ModalDescription>The pull requests behind it are tested again without it.</ModalDescription>
          )}
          <ModalFooter>
            <Button
              onClick={() => {
                setIsCancelling(false);
              }}
            >
              Keep it
            </Button>
            <Button
              variant="destructive"
              isPending={cancel.isPending}
              pendingText="Cancelling"
              onClick={() => {
                cancel.mutate(target, {
                  onSuccess: () => {
                    setIsCancelling(false);
                    onSent(
                      "Cancellation sent: the queue takes the pull request out at its next reconciliation.",
                    );
                  },
                  onError: () => {
                    setIsCancelling(false);
                  },
                });
              }}
            >
              Cancel the pull request
            </Button>
          </ModalFooter>
        </Modal>
      )}
    </div>
  );
}

import { type ReactNode, useState } from "react";

import { updateQueueMutation } from "@/client/@tanstack/react-query.gen";
import type { Queue, RequestedQueueState } from "@/client/types.gen";
import { DrainIcon } from "@/components/icons/drain-icon";
import { PauseIcon } from "@/components/icons/pause-icon";
import { PlayIcon } from "@/components/icons/play-icon";
import { Button } from "@/components/ui/button";
import { Modal, ModalDescription, ModalFooter } from "@/components/ui/modal";
import { refusalOf } from "@/domain/queue/actions";
import type { QueuePath } from "@/domain/queue/queries";
import { useQueueMutation } from "@/hooks/use-queue-mutation";

interface StateRequest {
  /** The button, and the confirmation in the modal: "Pause". */
  verb: string;
  icon: ReactNode;
  /** The confirmation while the request leaves: "Pausing". */
  pending: string;
  question: string;
  consequence: string;
}

const STATE_REQUESTS: Record<RequestedQueueState, StateRequest> = {
  paused: {
    verb: "Pause",
    icon: <PauseIcon />,
    pending: "Pausing",
    question: "Pause the queue?",
    consequence: "It starts no test and merges nothing, and keeps its order.",
  },
  draining: {
    verb: "Drain",
    icon: <DrainIcon />,
    pending: "Draining",
    question: "Drain the queue?",
    consequence: "It accepts no new pull request and lets the ones in it merge.",
  },
  running: {
    verb: "Resume",
    icon: <PlayIcon />,
    pending: "Resuming",
    question: "Resume the queue?",
    consequence: "It starts where it stopped.",
  },
};

/** Pause, drain or resume the queue: an admin only, as the server checks. */
export function QueueActions({
  path,
  queue,
  isAdmin,
  onSent,
}: {
  path: QueuePath;
  queue: Queue;
  isAdmin: boolean;
  onSent: (notice: string) => void;
}) {
  const [asked, setAsked] = useState<RequestedQueueState | null>(null);
  const update = useQueueMutation(updateQueueMutation());
  const choices: RequestedQueueState[] = queue.state === "running" ? ["paused", "draining"] : ["running"];
  const isLost = queue.access === "suspended";
  const request = asked && STATE_REQUESTS[asked];

  return (
    <>
      {choices.map((state) => (
        <Button
          key={state}
          disabled={!isAdmin || isLost}
          title={isAdmin ? undefined : "Needs the admin role"}
          onClick={() => {
            update.reset();
            setAsked(state);
          }}
        >
          {STATE_REQUESTS[state].icon}
          {STATE_REQUESTS[state].verb}
        </Button>
      ))}
      {asked && request && (
        <Modal
          title={request.question}
          onClose={() => {
            setAsked(null);
          }}
        >
          <ModalDescription>{request.consequence}</ModalDescription>
          {update.isError && (
            <p className="text-md text-destructive" role="alert">
              {refusalOf(update.error)}
            </p>
          )}
          <ModalFooter>
            <Button
              onClick={() => {
                setAsked(null);
              }}
            >
              Keep it
            </Button>
            <Button
              variant="primary"
              isPending={update.isPending}
              pendingText={request.pending}
              onClick={() => {
                update.mutate(
                  { path, body: { state: asked } },
                  {
                    onSuccess: () => {
                      setAsked(null);
                      onSent(`${request.verb} sent: the queue changes at its next reconciliation.`);
                    },
                  },
                );
              }}
            >
              {request.verb}
            </Button>
          </ModalFooter>
        </Modal>
      )}
    </>
  );
}

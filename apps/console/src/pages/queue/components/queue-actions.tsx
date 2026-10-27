import { type ReactNode, useEffect, useState } from "react";

import { updateQueueMutation } from "@/client/@tanstack/react-query.gen";
import type { Queue, RequestedQueueState } from "@/client/types.gen";
import {
  Button,
  CheckIcon,
  DrainIcon,
  Modal,
  ModalDescription,
  ModalFooter,
  PauseIcon,
  PlayIcon,
  Spinner,
  Toast,
} from "@cihub/ui";
import { refusalOf } from "@/domain/queue/actions";
import type { QueuePath } from "@/domain/queue/queries";
import { useQueueMutation } from "@/hooks/use-queue-mutation";

interface StateRequest {
  /** The button, and the confirmation in the modal: "Pause". */
  verb: string;
  icon: ReactNode;
  /** The confirmation while the request leaves, and the toast until the queue takes it: "Pausing". */
  pending: string;
  question: string;
  consequence: string;
  /** What the toast says once the queue is in the requested state. */
  done: string;
}

/** How long the toast says that the queue is in the requested state. */
const DONE_SHOWN_MS = 3_000;

const STATE_REQUESTS: Record<RequestedQueueState, StateRequest> = {
  paused: {
    verb: "Pause",
    icon: <PauseIcon />,
    pending: "Pausing",
    question: "Pause the queue?",
    consequence: "It starts no test and merges nothing, and keeps its order.",
    done: "The queue is paused.",
  },
  draining: {
    verb: "Drain",
    icon: <DrainIcon />,
    pending: "Draining",
    question: "Drain the queue?",
    consequence: "It accepts no new pull request and lets the ones in it merge.",
    done: "The queue is draining.",
  },
  running: {
    verb: "Resume",
    icon: <PlayIcon />,
    pending: "Resuming",
    question: "Resume the queue?",
    consequence: "It starts where it stopped.",
    done: "The queue is running.",
  },
};

/** Pause, drain or resume the queue: an admin only, as the server checks. */
export function QueueActions({ path, queue, isAdmin }: { path: QueuePath; queue: Queue; isAdmin: boolean }) {
  const [asked, setAsked] = useState<RequestedQueueState | null>(null);
  const [sent, setSent] = useState<RequestedQueueState | null>(null);
  const [done, setDone] = useState<RequestedQueueState | null>(null);
  const update = useQueueMutation(updateQueueMutation());
  const choices: RequestedQueueState[] = queue.state === "running" ? ["paused", "draining"] : ["running"];
  const isLost = queue.is_suspended;
  const request = asked && STATE_REQUESTS[asked];

  // The queue takes the request at its next reconciliation; the stream brings its new state.
  if (sent && queue.state === sent) {
    setSent(null);
    setDone(sent);
  }

  useEffect(() => {
    if (!done) {
      return;
    }

    const timer = setTimeout(() => {
      setDone(null);
    }, DONE_SHOWN_MS);

    return () => {
      clearTimeout(timer);
    };
  }, [done]);

  return (
    <>
      {choices.map((state) => (
        <Button
          key={state}
          disabled={!isAdmin || isLost}
          title={isAdmin ? undefined : "Needs the admin permission on the repository"}
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
                      setSent(asked);
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
      <Toast>
        {sent ? (
          <span className="flex items-center gap-2">
            <Spinner className="text-primary" />
            {STATE_REQUESTS[sent].pending} the queue…
          </span>
        ) : (
          done && (
            <span className="flex items-center gap-2">
              <CheckIcon className="size-4 text-success" />
              {STATE_REQUESTS[done].done}
            </span>
          )
        )}
      </Toast>
    </>
  );
}

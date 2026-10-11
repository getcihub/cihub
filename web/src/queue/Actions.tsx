import { useForm } from "@tanstack/react-form";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { z } from "zod";

import {
  cancelItemMutation,
  restartItemMutation,
  submitMutation,
  updateQueueMutation,
} from "../client/@tanstack/react-query.gen";
import type { Item, Queue, RequestedQueueState } from "../client/types.gen";
import { Modal } from "../ui/Modal";
import ui from "../ui/ui.module.css";
import { type QueuePath, readQueueAgain } from "./reads";

/** What the API answered to a refused action: its problem, else that it did not answer. */
function refusalOf(error: unknown): string {
  if (typeof error === "object" && error !== null && "detail" in error && typeof error.detail === "string") {
    return error.detail || "The server refused the request.";
  }
  return "The server did not answer. Try again.";
}

const ASKS: Record<RequestedQueueState, [string, string, string]> = {
  paused: ["Pause", "Pause the queue?", "It starts no test and merges nothing, and keeps its order."],
  draining: ["Drain", "Drain the queue?", "It accepts no new pull request and lets the ones in it merge."],
  running: ["Resume", "Resume the queue?", "It starts where it stopped."],
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
  const queryClient = useQueryClient();
  const [asked, setAsked] = useState<RequestedQueueState | null>(null);
  const update = useMutation({
    ...updateQueueMutation(),
    onSettled: () => readQueueAgain(queryClient),
  });
  const choices: RequestedQueueState[] = queue.state === "running" ? ["paused", "draining"] : ["running"];
  const isLost = queue.access === "suspended";

  return (
    <>
      {choices.map((state) => (
        <button
          key={state}
          type="button"
          className={ui.button}
          disabled={!isAdmin || isLost}
          title={isAdmin ? undefined : "Needs the admin role"}
          onClick={() => {
            update.reset();
            setAsked(state);
          }}
        >
          {ASKS[state][0]}
        </button>
      ))}
      {asked && (
        <Modal
          title={ASKS[asked][1]}
          onClose={() => {
            setAsked(null);
          }}
        >
          <p>{ASKS[asked][2]}</p>
          {update.isError && (
            <p className={ui.invalid} role="alert">
              {refusalOf(update.error)}
            </p>
          )}
          <div className={ui.buttons}>
            <button
              type="button"
              className={ui.button}
              onClick={() => {
                setAsked(null);
              }}
            >
              Keep it
            </button>
            <button
              type="button"
              className={`${ui.button} ${ui.primary}`}
              disabled={update.isPending}
              onClick={() => {
                update.mutate(
                  { path, body: { state: asked } },
                  {
                    onSuccess: () => {
                      setAsked(null);
                      onSent(`${ASKS[asked][0]} sent: the queue changes at its next reconciliation.`);
                    },
                  },
                );
              }}
            >
              {ASKS[asked][0]}
            </button>
          </div>
        </Modal>
      )}
    </>
  );
}

const PRIORITIES = ["urgent", "high", "medium", "low"] as const;

const submission = z.object({
  number: z.string().regex(/^[1-9][0-9]*$/, "The number of a pull request: 1 or more."),
  priority: z.enum(PRIORITIES),
  isAlone: z.boolean(),
});

/** Submits a PR to the queue: the queue reads it, then takes it or says why not. */
export function SubmitPr({
  path,
  isDisabled,
  onSent,
}: {
  path: QueuePath;
  isDisabled: boolean;
  onSent: (notice: string) => void;
}) {
  const queryClient = useQueryClient();
  const [isOpen, setIsOpen] = useState(false);
  const submit = useMutation({ ...submitMutation(), onSettled: () => readQueueAgain(queryClient) });
  const empty: z.input<typeof submission> = { number: "", priority: "medium", isAlone: false };
  const form = useForm({
    defaultValues: empty,
    validators: { onSubmit: submission },
    onSubmit: async ({ value }) => {
      const number = Number(value.number);
      const body = { pr_number: number, priority: value.priority, no_batch: value.isAlone };
      try {
        await submit.mutateAsync({ path, body });
      } catch {
        // The form shows the refusal of the server.
        return;
      }
      setIsOpen(false);
      form.reset();
      onSent(`#${String(number)} sent: the queue reads it, then takes it or says why on the pull request.`);
    },
  });

  return (
    <>
      <button
        type="button"
        className={`${ui.button} ${ui.primary}`}
        disabled={isDisabled}
        onClick={() => {
          submit.reset();
          setIsOpen(true);
        }}
      >
        Submit a pull request
      </button>
      {isOpen && (
        <Modal
          title="Submit a pull request"
          onClose={() => {
            setIsOpen(false);
          }}
        >
          <form
            onSubmit={(event) => {
              event.preventDefault();
              void form.handleSubmit();
            }}
            noValidate
          >
            <form.Field name="number">
              {(field) => (
                <label className={ui.field}>
                  Pull request number
                  <input
                    type="text"
                    inputMode="numeric"
                    name={field.name}
                    value={field.state.value}
                    onChange={(event) => {
                      field.handleChange(event.target.value);
                    }}
                    aria-invalid={field.state.meta.errors.length > 0}
                  />
                  {field.state.meta.errors.map((error) => (
                    <span key={error?.message} className={ui.invalid}>
                      {error?.message}
                    </span>
                  ))}
                </label>
              )}
            </form.Field>
            <form.Field name="priority">
              {(field) => (
                <label className={ui.field}>
                  Priority
                  <select
                    name={field.name}
                    value={field.state.value}
                    onChange={(event) => {
                      field.handleChange(event.target.value as (typeof PRIORITIES)[number]);
                    }}
                  >
                    {PRIORITIES.map((priority) => (
                      <option key={priority} value={priority}>
                        {priority}
                      </option>
                    ))}
                  </select>
                </label>
              )}
            </form.Field>
            <form.Field name="isAlone">
              {(field) => (
                <label className={ui.check}>
                  <input
                    type="checkbox"
                    name={field.name}
                    checked={field.state.value}
                    onChange={(event) => {
                      field.handleChange(event.target.checked);
                    }}
                  />
                  Test it alone, never in a batch
                </label>
              )}
            </form.Field>
            {submit.isError && (
              <p className={ui.invalid} role="alert">
                {refusalOf(submit.error)}
              </p>
            )}
            <div className={ui.buttons}>
              <button
                type="button"
                className={ui.button}
                onClick={() => {
                  setIsOpen(false);
                }}
              >
                Close
              </button>
              <button type="submit" className={`${ui.button} ${ui.primary}`} disabled={submit.isPending}>
                Submit
              </button>
            </div>
          </form>
        </Modal>
      )}
    </>
  );
}

/** Restarts the tests of a PR, or takes it out of the queue: the queue decides at its next reconciliation. */
export function PrActions({
  path,
  item,
  onSent,
}: {
  path: QueuePath;
  item: Item;
  onSent: (notice: string) => void;
}) {
  const queryClient = useQueryClient();
  const [isCancelling, setIsCancelling] = useState(false);
  const options = { onSettled: () => readQueueAgain(queryClient) };
  const restart = useMutation({ ...restartItemMutation(), ...options });
  const cancel = useMutation({ ...cancelItemMutation(), ...options });
  const target = { path: { ...path, number: item.pr_number } };
  const canRestart = item.state === "testing" || item.state === "pending_failure";
  const isTested = canRestart || item.state === "tests_passed";
  const error = restart.error ?? cancel.error;

  return (
    <div className={ui.buttons}>
      {canRestart && (
        <button
          type="button"
          className={ui.button}
          disabled={restart.isPending}
          onClick={() => {
            restart.mutate(target, {
              onSuccess: () => {
                onSent("Restart sent: the queue tests the pull request again at its next reconciliation.");
              },
            });
          }}
        >
          Restart tests
        </button>
      )}
      <button
        type="button"
        className={`${ui.button} ${ui.danger}`}
        onClick={() => {
          cancel.reset();
          setIsCancelling(true);
        }}
      >
        Cancel
      </button>
      {error && (
        <p className={ui.invalid} role="alert">
          {refusalOf(error)}
        </p>
      )}
      {isCancelling && (
        <Modal
          title={`Take #${String(item.pr_number)} out of the queue?`}
          onClose={() => {
            setIsCancelling(false);
          }}
        >
          {isTested && <p>The pull requests behind it are tested again without it.</p>}
          <div className={ui.buttons}>
            <button
              type="button"
              className={ui.button}
              onClick={() => {
                setIsCancelling(false);
              }}
            >
              Keep it
            </button>
            <button
              type="button"
              className={`${ui.button} ${ui.danger} ${ui.primary}`}
              disabled={cancel.isPending}
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
            </button>
          </div>
        </Modal>
      )}
    </div>
  );
}

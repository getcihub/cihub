import { useForm } from "@tanstack/react-form";
import { useState } from "react";
import { z } from "zod";

import { submitMutation } from "@/client/@tanstack/react-query.gen";
import { Button, Input, Modal, ModalFooter, PlusIcon, Select } from "@cihub/ui";
import { refusalOf } from "@/domain/queue/actions";
import type { QueuePath } from "@/domain/queue/queries";
import { useQueueMutation } from "@/hooks/use-queue-mutation";

const PRIORITIES = ["urgent", "high", "medium", "low"] as const;

const priority = z.enum(PRIORITIES);

const submission = z.object({
  number: z.string().regex(/^[1-9][0-9]*$/, "The number of a pull request: 1 or more."),
  priority,
  isAlone: z.boolean(),
});

const EMPTY: z.input<typeof submission> = { number: "", priority: "medium", isAlone: false };

/** Submits a PR to the queue of its base branch: the queue reads it, then takes it or says why not. */
export function SubmitPr({
  path,
  isDisabled,
  onSent,
}: {
  path: QueuePath;
  isDisabled: boolean;
  onSent: (notice: string) => void;
}) {
  const [isOpen, setIsOpen] = useState(false);
  const submit = useQueueMutation(submitMutation());
  const form = useForm({
    defaultValues: EMPTY,
    validators: { onSubmit: submission },
    onSubmit: async ({ value }) => {
      const number = Number(value.number);
      const body = { priority: value.priority, no_batch: value.isAlone };
      try {
        await submit.mutateAsync({ path: { org: path.org, repo: path.repo, number }, body });
      } catch {
        // The form shows the refusal of the server.
        return;
      }
      setIsOpen(false);
      form.reset();
      onSent(
        `#${String(number)} sent: the queue of its base branch reads it, then takes it or says why on the pull request.`,
      );
    },
  });

  return (
    <>
      <Button
        variant="primary"
        disabled={isDisabled}
        onClick={() => {
          submit.reset();
          setIsOpen(true);
        }}
      >
        <PlusIcon />
        Submit a pull request
      </Button>

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
                <label className="grid gap-1.5 text-lg">
                  <span className="text-md font-medium">Pull request number</span>
                  <Input
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
                    <span key={error?.message} className="text-sm font-medium text-destructive">
                      {error?.message}
                    </span>
                  ))}
                </label>
              )}
            </form.Field>
            <form.Field name="priority">
              {(field) => (
                <label className="grid gap-1.5 text-lg">
                  <span className="text-md font-medium">Priority</span>
                  <Select
                    name={field.name}
                    value={field.state.value}
                    onChange={(event) => {
                      field.handleChange(priority.parse(event.target.value));
                    }}
                  >
                    {PRIORITIES.map((priority) => (
                      <option key={priority} value={priority}>
                        {priority}
                      </option>
                    ))}
                  </Select>
                </label>
              )}
            </form.Field>
            <form.Field name="isAlone">
              {(field) => (
                <label className="flex items-center gap-2 text-lg">
                  <input
                    // The margin of a checkbox in the browser, which the reset of the styles removes.
                    className="my-0.75 mr-0.75 ml-1"
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
              <p className="text-md text-destructive" role="alert">
                {refusalOf(submit.error)}
              </p>
            )}
            <ModalFooter>
              <Button
                onClick={() => {
                  setIsOpen(false);
                }}
              >
                Close
              </Button>
              <Button type="submit" variant="primary" isPending={submit.isPending} pendingText="Submitting">
                Submit
              </Button>
            </ModalFooter>
          </form>
        </Modal>
      )}
    </>
  );
}

import { useForm } from "@tanstack/react-form";
import { useMutation } from "@tanstack/react-query";
import { useState } from "react";
import { z } from "zod";

import { approveDeviceCodeMutation, denyDeviceCodeMutation } from "@/client/@tanstack/react-query.gen";
import { ErrorAlert } from "@/components/errors/error-alert";
import { Lead, Note, Title } from "@/components/session/parts";
import { Alert } from "@cihub/ui/components/alert";
import { Button } from "@cihub/ui/components/button";
import { type Failure, isProblem, refusalOf } from "@/domain/problem";
import { userCodeOf } from "@/domain/session/device";
import { CodeBoxes } from "./code-boxes";

type Answer = "approve" | "deny";

const typedCode = z.object({
  code: z.string().refine((typed) => userCodeOf(typed) !== null, "8 letters, as the terminal shows them."),
});

/** What the server answered to a code that it refused, for the person. */
function codeRefusalOf(failure: Failure): string {
  if (isProblem(failure, "device_code_not_found")) {
    return "No command line waits for this code: check it, or run cihub auth login again.";
  }

  return refusalOf(failure);
}

/**
 * The code of the command line, which the person types, then approves or denies. No link carries the code: a link
 * sent by someone else would approve their code with one click.
 */
export function DeviceForm({ login }: { login: string }) {
  const approve = useMutation(approveDeviceCodeMutation());
  const deny = useMutation(denyDeviceCodeMutation());

  const [answered, setAnswered] = useState<Answer | null>(null);
  const form = useForm({
    defaultValues: { code: "" },
    validators: { onSubmit: typedCode },
    onSubmitMeta: { answer: "approve" as Answer },
    onSubmit: async ({ value, meta }) => {
      const path = { user_code: userCodeOf(value.code) ?? value.code };
      const mutation = meta.answer === "approve" ? approve : deny;
      try {
        await mutation.mutateAsync({ path });
      } catch {
        // The form shows the refusal of the server.
        return;
      }
      setAnswered(meta.answer);
    },
  });

  const failed = approve.error ?? deny.error;

  if (answered) {
    return (
      <>
        <Title>{answered === "approve" ? "Command line signed in" : "Sign-in denied"}</Title>
        <Alert role="status">
          {answered === "approve"
            ? `The command line acts as ${login} now. You can close this tab and go back to the terminal.`
            : "No command line signs in with this code."}
        </Alert>
      </>
    );
  }

  return (
    <>
      <Title>Sign in the command line</Title>
      <Lead>
        Type the code that <code>cihub auth login</code> shows in your terminal.
      </Lead>
      <form
        className="grid gap-4"
        onSubmit={(event) => {
          event.preventDefault();
          void form.handleSubmit({ answer: "approve" });
        }}
        noValidate
      >
        <form.Field name="code">
          {(field) => (
            <div className="grid gap-1.5">
              <CodeBoxes
                letters={field.state.value}
                isInvalid={field.state.meta.errors.length > 0}
                onChange={(letters) => {
                  field.handleChange(letters);
                }}
              />
              {field.state.meta.errors.map((error) => (
                <span key={error?.message} className="text-sm font-medium text-destructive">
                  {error?.message}
                </span>
              ))}
            </div>
          )}
        </form.Field>
        {failed && <ErrorAlert>{codeRefusalOf(failed)}</ErrorAlert>}
        <Button
          type="submit"
          variant="primary"
          size="lg"
          isPending={approve.isPending}
          pendingText="Approving"
        >
          Approve as {login}
        </Button>
        <Button
          size="lg"
          isPending={deny.isPending}
          pendingText="Denying"
          onClick={() => {
            void form.handleSubmit({ answer: "deny" });
          }}
        >
          Deny
        </Button>
      </form>
      <Note>
        If you did not run <code>cihub auth login</code>, deny the code: someone else asks to act as you.
      </Note>
    </>
  );
}

import { useForm } from "@tanstack/react-form";
import { useMutation } from "@tanstack/react-query";
import { useState } from "react";
import { z } from "zod";

import { approveDeviceCodeMutation, denyDeviceCodeMutation } from "@/client/@tanstack/react-query.gen";
import { Failure, Lead, Note, Title } from "@/components/session/parts";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { failureOf, isProblem, problemOf } from "@/domain/problem";
import { userCodeOf } from "@/domain/session/device";

type Answer = "approve" | "deny";

const typedCode = z.object({
  code: z.string().refine((typed) => userCodeOf(typed) !== null, "8 letters, as the terminal shows them."),
});

/** What the server answered to a code that it refused, for the person. */
function refusalOf(thrown: unknown): string {
  const failure = failureOf(thrown);
  if (isProblem(failure, "device_code_not_found")) {
    return "No command line waits for this code: check it, or run cihub auth login again.";
  }
  return problemOf(failure)?.detail ?? "The server did not answer. Try again.";
}

/**
 * The code of the command line, which the person checks, then approves or denies. Nothing approves a code by
 * itself: a link can carry the code of someone else. A code of the link shows as it is, to compare with the
 * terminal, never to edit; without one, the person types it.
 */
export function DeviceForm({ login, code }: { login: string; code: string }) {
  const [answered, setAnswered] = useState<Answer | null>(null);
  const approve = useMutation(approveDeviceCodeMutation());
  const deny = useMutation(denyDeviceCodeMutation());
  const linkedCode = userCodeOf(code);
  const form = useForm({
    defaultValues: { code: linkedCode ?? code },
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
        {linkedCode ? "Check that this code is the one that" : "Type the code that"}{" "}
        <code>cihub auth login</code> shows in your terminal.
      </Lead>
      <form
        className="grid gap-4"
        onSubmit={(event) => {
          event.preventDefault();
          void form.handleSubmit({ answer: "approve" });
        }}
        noValidate
      >
        {linkedCode ? (
          <div className="grid gap-1.5">
            <span id="device-code" className="text-md font-medium">
              Code
            </span>
            <p aria-labelledby="device-code" className="font-mono text-5xl font-semibold tracking-[0.08em]">
              {linkedCode}
            </p>
          </div>
        ) : (
          <form.Field name="code">
            {(field) => (
              <label className="grid gap-1.5 text-lg">
                <span className="text-md font-medium">Code</span>
                <Input
                  type="text"
                  name={field.name}
                  value={field.state.value}
                  placeholder="WDJB-MJHT"
                  autoComplete="off"
                  spellCheck={false}
                  className="font-mono uppercase"
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
        )}
        {failed && <Failure className="">{refusalOf(failed)}</Failure>}
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

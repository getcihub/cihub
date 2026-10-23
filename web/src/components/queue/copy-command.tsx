import { useEffect, useState } from "react";

import { CopyIcon } from "@/components/icons/copy-icon";
import { Button } from "@/components/ui/button";
import { Toast } from "@/components/ui/toast";
import { mergeCommandOf } from "@/domain/session/session";
import { useMe } from "@/hooks/use-me";

/** The command that submits a PR in a comment. */
export function MergeCommand() {
  return <code>{mergeCommandOf(useMe())}</code>;
}

/** How long the button says what the copy did, and the toast says that it copied. */
const RESULT_SHOWN_MS = 2_000;

/** The command that submits a PR, and a button that copies it. */
export function CopyCommand() {
  const command = mergeCommandOf(useMe());
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
      <span className="select-all">{command}</span>
      <Button
        className="h-6 px-2 font-sans text-md"
        onClick={() => {
          // A page served over plain HTTP has no clipboard: the command stays selectable.
          void Promise.resolve()
            .then(() => navigator.clipboard.writeText(command))
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
      <Toast className="font-sans">{result === "copied" && <>Copied {command} to the clipboard.</>}</Toast>
    </span>
  );
}

import type { BranchConfig, DiffLine } from "@/client/types.gen";
import { ConfigText } from "@/components/setup/config-text";
import { Card, CardHeader, CardHint, CardTitle } from "@/components/ui/card";
import { Prose } from "@/components/ui/prose";
import { cn } from "@/lib/cn";
import type { ConfigDraft } from "@/pages/repo-config/use-config-draft";

/** The file as CIHub read it, with the line of its error; with the changes of a draft, the diff of its YAML. */
export function YamlView({ config, draft }: { config: BranchConfig; draft: ConfigDraft }) {
  const preview = draft.preview;

  return (
    <Card className="p-0">
      <CardHeader>
        <CardTitle>{config.path ?? ".cihub.yaml"}</CardTitle>
        <CardHint>
          {preview ? (
            "with your pending changes"
          ) : (
            <>
              the file on <code>{config.branch}</code>
            </>
          )}
        </CardHint>
      </CardHeader>
      <div className="grid gap-2 p-4">
        {preview ? (
          <DiffText lines={preview.diff} errorLine={preview.line ?? null} />
        ) : config.text ? (
          <ConfigText text={config.text} errorLine={config.line ?? null} />
        ) : (
          <p className="text-md text-muted-foreground">
            {config.state === "none" ? (
              <>
                No <code>.cihub.yaml</code> on <code>{config.branch}</code>: the queues take the values of
                CIHub.
              </>
            ) : config.state === "invalid" ? (
              <Prose text={config.error ?? ""} />
            ) : (
              "The file is larger than 64 KiB: open it on GitHub."
            )}
          </p>
        )}
        <p className="text-md text-muted-foreground">
          CIHub reads the file at the head of <code>{config.branch}</code>, so a pull request never changes
          its own gate.
        </p>
      </div>
    </Card>
  );
}

const MARKS = { same: " ", added: "+", removed: "−" };

/**
 * The lines of a diff: those that stay, go and come, with the numbers of the new file. `errorLine`, in the new
 * file, is marked. Only the changes, for the modal of a proposal.
 */
export function DiffText({
  lines,
  errorLine,
  isChangesOnly = false,
}: {
  lines: DiffLine[];
  errorLine: number | null;
  isChangesOnly?: boolean;
}) {
  // The number of each line in the new file: a line that goes has none.
  const numbers = lines.map(
    (_, index) => lines.slice(0, index + 1).filter((line) => line.change !== "removed").length,
  );

  return (
    <pre className="max-h-96 overflow-auto rounded-sm border border-border bg-muted py-1.5 font-mono text-md">
      {lines.map((line, index) => {
        const number = numbers[index];
        return (
          <span
            key={index}
            className={cn(
              "block px-3",
              line.change === "added" && "bg-success-muted",
              line.change === "removed" && "bg-destructive-muted text-muted-foreground",
              line.change !== "removed" && number === errorLine && "bg-destructive-muted text-destructive",
            )}
          >
            {!isChangesOnly && (
              <i className="mr-3 inline-block w-6 text-right text-muted-foreground not-italic select-none">
                {line.change === "removed" ? "" : number}
              </i>
            )}
            <b className="mr-2 font-normal select-none">{MARKS[line.change]}</b>
            {line.text}
          </span>
        );
      })}
    </pre>
  );
}

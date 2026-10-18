import { cn } from "@/lib/cn";

/** The text of a configuration file that is not valid, with the line of its error marked. */
export function ConfigText({ text, errorLine }: { text: string; errorLine: number | null }) {
  return (
    <pre className="overflow-x-auto rounded-sm border border-border bg-muted py-1.5 font-mono text-md">
      {text.split("\n").map((line, index) => {
        const number = index + 1;
        return (
          <span
            key={number}
            className={cn("block px-3", number === errorLine && "bg-destructive-muted text-destructive")}
          >
            <i className="mr-3 inline-block w-6 text-right text-muted-foreground not-italic select-none">
              {number}
            </i>
            {line}
          </span>
        );
      })}
    </pre>
  );
}

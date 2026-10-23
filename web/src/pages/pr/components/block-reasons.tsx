import type { Pr } from "@/client/types.gen";
import { Prose } from "@/components/ui/prose";
import { WrittenConditions } from "@/components/conditions/condition-list";
import { blockSentence } from "@/domain/pr/words";
import { cn } from "@/lib/cn";

/** What keeps a PR out of the queue, one reason a line; the unmet queue conditions as the condition builder. */
export function BlockReasons({ reasons, className }: { reasons: Pr["block_reasons"]; className?: string }) {
  return (
    <ul className={cn("grid gap-2.5", className)}>
      {reasons.map((reason, index) => {
        const sentence = blockSentence(reason);
        const conditions = Array.isArray(reason.conditions) ? reason.conditions.map(String) : [];

        return (
          <li key={index}>
            {sentence === null ? (
              <>
                It does not meet {conditions.length > 1 ? "these queue conditions" : "this queue condition"}:
                <div className="mt-2" role="group" aria-label="Unmet queue conditions">
                  <WrittenConditions conditions={conditions} />
                </div>
              </>
            ) : (
              <Prose text={sentence} />
            )}
          </li>
        );
      })}
    </ul>
  );
}

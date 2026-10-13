import type { Pr } from "@/client/types.gen";
import { itemStateLabel } from "@/domain/pr/words";
import { cn } from "@/lib/cn";

const STEPS = ["Submitted", "Queued", "Testing", "Merged"];

const STEP_OF: Record<string, number> = {
  not_ready: 0,
  pending: 1,
  testing: 2,
  pending_failure: 2,
  tests_passed: 2,
  merged: 3,
  failed: 2,
  cancelled: 1,
};

/** The dot of the current step, in the color of how the PR stands. */
const CURRENT_DOTS: Record<string, string> = {
  pending_failure: "border-warning bg-warning",
  failed: "border-destructive bg-destructive",
  cancelled: "border-waiting",
};

/** Submitted, queued, testing, merged: where the PR is in its life. */
export function Journey({ item }: { item: Pr }) {
  const step = STEP_OF[item.state] ?? 0;

  return (
    <div className="relative before:absolute before:top-1.25 before:right-[12.5%] before:left-[12.5%] before:h-0.5 before:bg-divider">
      <ol className="relative grid grid-cols-4 text-sm text-muted-foreground">
        {STEPS.map((label, index) => (
          <li
            key={label}
            className={cn(
              "grid justify-items-center gap-1.5 text-center",
              index === step && "font-medium text-foreground",
            )}
            aria-current={index === step ? "step" : undefined}
          >
            <span
              className={cn(
                "size-3 rounded-full border-2 border-waiting bg-card",
                index < step && "border-primary bg-primary",
                index === step && (CURRENT_DOTS[item.state] ?? "border-primary"),
              )}
            />
            {index === step ? itemStateLabel(item.state) : label}
          </li>
        ))}
      </ol>
    </div>
  );
}

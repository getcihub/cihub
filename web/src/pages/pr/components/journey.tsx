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
  merged: "border-primary bg-primary",
  pending_failure: "border-warning bg-warning",
  failed: "border-destructive bg-destructive",
  cancelled: "border-waiting",
};

/** The states whose current step glows: the PR still moves toward its merge. */
const MOVING = new Set(["not_ready", "pending", "testing", "pending_failure", "tests_passed"]);

/** Submitted, queued, testing, merged: where the PR is in its life. */
export function Journey({ item }: { item: Pr }) {
  const step = STEP_OF[item.state] ?? 0;

  return (
    <div className="relative before:absolute before:top-1.25 before:right-[12.5%] before:left-[12.5%] before:h-0.5 before:bg-divider">
      {/* The steps are a quarter of the width each: the line goes from the middle of the first to the current one. */}
      <span
        className="absolute top-1.25 left-[12.5%] h-0.5 bg-primary"
        style={{ width: `${String(25 * step)}%` }}
      />
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
                index === step && MOVING.has(item.state) && "motion-safe:animate-beacon",
              )}
            />
            {index === step ? itemStateLabel(item.state) : label}
          </li>
        ))}
      </ol>
    </div>
  );
}

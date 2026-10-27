import type { ItemState, Pr } from "@/client/types.gen";
import { CheckIcon, cn } from "@cihub/ui";
import { itemStateLabel } from "@/domain/pr/words";
import { plural } from "@/lib/text";

/** The most PRs ahead that the train shows; the others are a number. */
const SHOWN = 10;

const CAR = "relative size-3.5 flex-none rounded-xs border-[1.5px] border-waiting bg-card";

const CARS: Record<string, string> = {
  testing: "border-primary bg-primary-muted",
  pending_failure: "border-warning bg-warning-muted",
  tests_passed: "border-success bg-success-muted",
};

/**
 * The car of the PR itself: a round one, amber when its tests failed, an outline before it enters the line; it
 * glows while its run tests it and until it merges.
 */
function ownCarOf(state: ItemState): string {
  const own = "mr-1.5 size-5 rounded-full border-primary bg-primary ring-3 ring-primary-muted";

  if (state === "pending_failure") {
    return cn(own, "border-warning bg-warning ring-warning-muted");
  }
  if (state === "not_ready") {
    return cn(own, "border-dashed bg-card ring-0");
  }
  if (state === "testing" || state === "tests_passed") {
    return cn(own, "motion-safe:animate-beacon");
  }
  return own;
}

/**
 * The line in front of a PR, toward the target branch: the PR, then the PRs that merge before it, the nearest
 * first, then the branch.
 */
export function Train({ pr, branch }: { pr: Pr; branch: string }) {
  const nearestFirst = [...pr.ahead].reverse();
  const shown = nearestFirst.slice(-SHOWN);
  const isMerged = pr.state === "merged";

  return (
    <div
      className="relative flex h-6.5 min-w-0 items-center justify-end gap-1.5 before:absolute before:top-1/2 before:right-2.75 before:left-0 before:border-t-[1.5px] before:border-dashed before:border-waiting"
      role="img"
      aria-label={
        isMerged
          ? `Merged into ${branch}`
          : `${plural(pr.ahead.length, "pull request", "pull requests")} before it, then ${branch}`
      }
    >
      {!isMerged && <span className={cn(CAR, ownCarOf(pr.state))} />}
      {nearestFirst.length > shown.length && (
        <span className="relative bg-card px-1 text-2xs text-muted-foreground">
          +{nearestFirst.length - shown.length}
        </span>
      )}
      {shown.map((other) => (
        <span
          key={other.pr_number}
          className={cn(CAR, CARS[other.state])}
          title={`#${String(other.pr_number)} · ${itemStateLabel(other.state)}`}
        />
      ))}
      <span
        className={cn(
          "relative ml-2 grid size-5.5 flex-none place-items-center rounded-full bg-primary text-primary-foreground [&_svg]:size-3",
          isMerged && "ring-2 ring-primary ring-offset-3 ring-offset-primary-muted",
        )}
        title={branch}
      >
        <CheckIcon />
      </span>
    </div>
  );
}

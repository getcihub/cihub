import type { QueueHealth } from "@/client/types.gen";
import { Card, CardHeader, CardHint, CardTitle } from "@cihub/ui/components/card";
import { cn } from "@cihub/ui/lib/cn";
import { plural } from "@/lib/text";

/** Why the PRs that did not merge left, in the words of the API. */
export function LeftWithoutMerge({ health }: { health: QueueHealth }) {
  const reasons = health.left_without_merge;
  const total = reasons.reduce((sum, reason) => sum + reason.count, 0);
  const most = Math.max(1, ...reasons.map((reason) => reason.count));

  return (
    <Card className="min-w-0">
      <CardHeader>
        <CardTitle>Left without a merge</CardTitle>
        <CardHint>{plural(total, "pull request", "pull requests")}</CardHint>
      </CardHeader>
      {total === 0 ? (
        <p className="p-7 text-center text-secondary-foreground">Every pull request merged.</p>
      ) : (
        <ul className="grid gap-2.5 p-4 text-base">
          {reasons.map((reason) => (
            <li
              key={`${reason.conclusion}-${reason.reason}`}
              className="grid grid-cols-[minmax(0,1fr)_110px_24px] items-center gap-3 max-md:grid-cols-[minmax(0,1fr)_70px_24px]"
            >
              <span>
                {reason.description.charAt(0).toUpperCase()}
                {reason.description.slice(1)}
              </span>
              <span className="h-1.5 rounded-full bg-muted">
                <span
                  className={cn(
                    "block h-full rounded-full",
                    reason.conclusion === "failed" ? "bg-destructive" : "bg-waiting",
                  )}
                  style={{ width: `${String((reason.count / most) * 100)}%` }}
                />
              </span>
              <b className="text-right font-medium tabular-nums">{reason.count}</b>
            </li>
          ))}
        </ul>
      )}
    </Card>
  );
}

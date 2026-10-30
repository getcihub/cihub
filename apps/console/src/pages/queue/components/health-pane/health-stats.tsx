import type { ReactNode } from "react";

import type { QueueHealth } from "@/client/types.gen";
import { Card } from "@cihub/ui/components/card";
import { cn } from "@cihub/ui/lib/cn";
import { type PeriodWords, hasPeriodBefore } from "@/domain/queue/period";
import { plural } from "@/lib/text";
import { durationOf, failureRateOf } from "./health-format";

/** How a value moved since the period before, in green when it got better. */
function Delta({
  now,
  before,
  isLowerBetter = false,
  text,
}: {
  now: number;
  before: number;
  isLowerBetter?: boolean;
  text: string;
}) {
  if (now === before) {
    return null;
  }

  const isUp = now > before;

  return (
    <span className={cn("text-sm font-medium", isUp === isLowerBetter ? "text-destructive" : "text-success")}>
      {isUp ? "+" : "−"}
      {text}
    </span>
  );
}

function Stat({
  label,
  value,
  delta,
  hint,
}: {
  label: string;
  value: string;
  delta: ReactNode;
  hint: string;
}) {
  return (
    <Card className="px-3.5 py-3">
      <div className="text-sm text-secondary-foreground">{label}</div>
      <div className="mt-0.5 flex items-baseline gap-2 text-5xl font-medium tracking-[-0.3px] tabular-nums">
        {value}
        {delta}
      </div>
      <div className="text-md text-muted-foreground">{hint}</div>
    </Card>
  );
}

export function HealthStats({
  health,
  words,
  now,
}: {
  health: QueueHealth;
  words: PeriodWords;
  now: number;
}) {
  // A period before that starts out of the 13 months that CIHub keeps has nothing to compare with.
  const isCompared = hasPeriodBefore(Date.parse(health.from), Date.parse(health.to), now);
  const { current } = health;
  const previous = isCompared ? health.previous : current;
  const rate = Math.round(failureRateOf(current) * 100);
  const rateBefore = Math.round(failureRateOf(previous) * 100);
  const median = current.time_to_merge?.median_seconds;
  const medianBefore = previous.time_to_merge?.median_seconds;

  return (
    <div className="grid grid-cols-3 gap-3 max-md:grid-cols-1">
      <Stat
        label="Merged"
        value={String(current.merged)}
        delta={
          <Delta
            now={current.merged}
            before={previous.merged}
            text={String(Math.abs(current.merged - previous.merged))}
          />
        }
        hint={
          isCompared
            ? `${String(previous.merged)} ${words.previous}`
            : "Nothing to compare with: older than 13 months"
        }
      />
      <Stat
        label="Time to merge"
        value={median == null ? "–" : durationOf(median)}
        delta={
          median != null &&
          medianBefore != null && (
            <Delta
              now={median}
              before={medianBefore}
              isLowerBetter
              text={durationOf(Math.abs(median - medianBefore))}
            />
          )
        }
        hint={
          current.time_to_merge
            ? `median; 90% within ${durationOf(current.time_to_merge.p90_seconds)}`
            : "no merge"
        }
      />
      <Stat
        label="Failure rate"
        value={`${String(rate)}%`}
        delta={
          <Delta
            now={rate}
            before={rateBefore}
            isLowerBetter
            text={`${String(Math.abs(rate - rateBefore))} pts`}
          />
        }
        hint={`${String(current.failed)} of ${plural(current.merged + current.failed, "pull request", "pull requests")} failed`}
      />
    </div>
  );
}

import { Link } from "@tanstack/react-router";

import { cn } from "@cihub/ui/lib/cn";
import { type HealthPeriod, RANGE_LABELS } from "@/domain/queue/period";
import type { QueuePath } from "@/domain/queue/queries";
import { RANGES } from "@/domain/queue/search";
import { PeriodPicker } from "./period-picker";

/** The periods of the Health: the last 24 hours, 7 days or 30 days, or Custom, a period that the person chooses. */
export function RangePicker({ path, period, now }: { path: QueuePath; period: HealthPeriod; now: number }) {
  return (
    <div className="relative max-md:w-full">
      <nav
        className="inline-flex rounded-md border border-border bg-card p-0.5 max-md:flex max-md:w-full"
        aria-label="Period"
      >
        {RANGES.map((candidate) => {
          const isCurrent = "range" in period && candidate === period.range;
          return (
            <Link
              key={candidate}
              className={cn(
                "flex h-6 items-center justify-center rounded-xs px-2.5 text-md whitespace-nowrap text-secondary-foreground hover:text-foreground max-md:flex-1",
                isCurrent && "bg-muted font-medium text-foreground",
              )}
              to="/orgs/$org/repos/$repo/queues/$queue"
              params={path}
              // A period that the person chose holds the default range too: the link is not its own.
              search={{ pane: "health", range: candidate, from: undefined, to: undefined }}
              activeOptions={{ explicitUndefined: true }}
              aria-current={isCurrent ? "page" : undefined}
            >
              {RANGE_LABELS[candidate]}
            </Link>
          );
        })}
        <PeriodPicker path={path} period={period} now={now} />
      </nav>
    </div>
  );
}

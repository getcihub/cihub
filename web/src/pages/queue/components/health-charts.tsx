import type { CSSProperties, ReactNode } from "react";

import type { HealthBucket } from "@/client/types.gen";
import type { Range } from "@/domain/queue/queries";
import { cn } from "@/lib/cn";
import { minutesOf } from "./health-format";

/** The rounded tops of the axis: the largest value of a chart reaches one of them. */
const NICE_TOPS = [
  2, 4, 6, 8, 10, 12, 16, 20, 30, 40, 50, 60, 80, 100, 120, 160, 200, 300, 400, 600, 800, 1000,
];

function niceTop(value: number): number {
  return NICE_TOPS.find((top) => top >= value) ?? Math.ceil(value / 500) * 500;
}

const utc = (options: Intl.DateTimeFormatOptions) =>
  new Intl.DateTimeFormat("en", { ...options, timeZone: "UTC" });
const HOUR = utc({ hour: "2-digit", minute: "2-digit", hourCycle: "h23" });
const WEEKDAY = utc({ weekday: "short" });
const DAY = utc({ month: "short", day: "numeric" });
const LONG_DAY = utc({ weekday: "short", month: "short", day: "numeric" });

/** The name of a bucket in its tip: an hour of the day, or a day, in UTC as the server cuts them. */
function labelOf(bucket: HealthBucket, range: Range): string {
  const start = new Date(bucket.start);
  return range === "24h" ? HOUR.format(start) : LONG_DAY.format(start);
}

/** The name of a bucket under the axis: every 6 hours, each day of a week, or each week of a month. */
function tickOf(bucket: HealthBucket, index: number, range: Range): string {
  const start = new Date(bucket.start);
  if (range === "24h") {
    return start.getUTCHours() % 6 === 0 ? HOUR.format(start) : "";
  }
  if (range === "7d") {
    return WEEKDAY.format(start);
  }
  return index % 7 === 1 ? DAY.format(start) : "";
}

/** A chart: an axis, a column per bucket that shows its tip on hover, and the names of the buckets. */
function Chart({
  buckets,
  range,
  top,
  plot,
  column,
  tip,
}: {
  buckets: HealthBucket[];
  range: Range;
  top: number;
  plot?: ReactNode;
  column?: (bucket: HealthBucket) => ReactNode;
  tip: (bucket: HealthBucket) => ReactNode;
}) {
  const columns = { "--n": String(buckets.length) } as CSSProperties;
  const grid = "grid grid-cols-[repeat(var(--n),minmax(0,1fr))] gap-x-[clamp(1px,calc(120px/var(--n)),6px)]";

  return (
    <div
      className="grid grid-cols-[24px_minmax(0,1fr)] grid-rows-[150px_auto] gap-x-2 gap-y-1.5 px-4 pt-4.5 pb-3"
      style={columns}
    >
      <div className="-my-1.5 flex flex-col items-end justify-between text-2xs text-muted-foreground tabular-nums">
        <span>{top}</span>
        <span>{top / 2}</span>
        <span>0</span>
      </div>
      <div className="relative border-b border-border">
        <span className="absolute inset-x-0 top-0 border-t border-divider" />
        <span className="absolute inset-x-0 top-1/2 border-t border-divider" />
        {plot}
        <div className={cn(grid, "absolute inset-0")}>
          {buckets.map((bucket, index) => (
            <div
              key={bucket.start}
              className="group relative flex h-full flex-col-reverse rounded-t-xs hover:bg-muted/80"
            >
              {column?.(bucket)}
              <span
                className={cn(
                  "pointer-events-none absolute -top-1.5 z-5 hidden gap-px rounded-sm bg-foreground px-2 py-1.5 text-sm whitespace-nowrap text-background group-hover:grid",
                  index >= buckets.length * 0.7
                    ? "right-0 -translate-y-full"
                    : "left-1/2 -translate-x-1/2 -translate-y-full",
                )}
              >
                <b className="font-medium">{labelOf(bucket, range)}</b>
                {tip(bucket)}
              </span>
            </div>
          ))}
        </div>
      </div>
      <div className={cn(grid, "col-start-2 text-2xs text-muted-foreground")}>
        {buckets.map((bucket, index) => (
          <span key={bucket.start} className="whitespace-nowrap">
            {tickOf(bucket, index, range)}
          </span>
        ))}
      </div>
    </div>
  );
}

/** The PRs that merged and failed in each bucket, stacked. */
export function MergedChart({ buckets, range }: { buckets: HealthBucket[]; range: Range }) {
  const top = niceTop(Math.max(1, ...buckets.map((bucket) => bucket.merged + bucket.failed)));
  const height = (count: number) => ({ height: `${String((count / top) * 100)}%` });

  return (
    <Chart
      buckets={buckets}
      range={range}
      top={top}
      column={(bucket) => (
        <>
          <span className="block flex-none rounded-t-xs bg-primary" style={height(bucket.merged)} />
          {bucket.failed > 0 && (
            <span
              className="mb-px block flex-none rounded-t-xs bg-destructive"
              style={height(bucket.failed)}
            />
          )}
        </>
      )}
      tip={(bucket) => (
        <span>
          {bucket.merged} merged · {bucket.failed} failed
        </span>
      )}
    />
  );
}

/** The median and the 90 % of the time to merge, in minutes; a bucket without a merge has no point. */
export function TimeToMergeChart({ buckets, range }: { buckets: HealthBucket[]; range: Range }) {
  const top = niceTop(
    Math.max(10, ...buckets.map((bucket) => minutesOf(bucket.time_to_merge?.p90_seconds ?? 0))),
  );
  const points = (field: "median_seconds" | "p90_seconds") =>
    buckets.flatMap((bucket, index) => {
      const seconds = bucket.time_to_merge?.[field];
      return seconds == null
        ? []
        : [{ x: ((index + 0.5) / buckets.length) * 100, y: 100 - (minutesOf(seconds) / top) * 100 }];
    });
  const path = (field: "median_seconds" | "p90_seconds") =>
    points(field)
      .map(({ x, y }, index) => `${index === 0 ? "M" : "L"}${x.toFixed(2)} ${y.toFixed(2)}`)
      .join(" ");

  return (
    <Chart
      buckets={buckets}
      range={range}
      top={top}
      plot={
        <>
          <svg
            className="absolute inset-0 size-full overflow-visible"
            viewBox="0 0 100 100"
            preserveAspectRatio="none"
            aria-hidden="true"
          >
            <path
              d={path("p90_seconds")}
              className="fill-none stroke-primary-border"
              strokeWidth="1.5"
              strokeDasharray="4 3"
              vectorEffect="non-scaling-stroke"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
            <path
              d={path("median_seconds")}
              className="fill-none stroke-primary"
              strokeWidth="2"
              vectorEffect="non-scaling-stroke"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
          {points("median_seconds").map(({ x, y }) => (
            <span
              key={x}
              className="pointer-events-none absolute -mt-0.75 -ml-0.75 size-1.5 rounded-full border-[1.5px] border-primary bg-card"
              style={{ left: `${String(x)}%`, top: `${String(y)}%` }}
            />
          ))}
        </>
      }
      tip={(bucket) => (
        <span>
          {bucket.time_to_merge
            ? `Median ${String(minutesOf(bucket.time_to_merge.median_seconds))} min · 90% within ${String(minutesOf(bucket.time_to_merge.p90_seconds))} min`
            : "No merge"}
        </span>
      )}
    />
  );
}

import { useSuspenseQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import type { ReactNode } from "react";

import type { CheckHealth, HealthTotals, QueueHealth, StateTime } from "@/client/types.gen";
import { Card, CardHeader, CardHint, CardTitle } from "@/components/ui/card";
import { LoadingStatus, Skeleton } from "@/components/ui/skeleton";
import {
  type HealthPeriod,
  type PeriodWords,
  RANGE_LABELS,
  grainOf,
  hasPeriodBefore,
  periodWords,
} from "@/domain/queue/period";
import { type QueuePath, RANGES, healthOptions } from "@/domain/queue/queries";
import { useNow } from "@/hooks/use-now";
import { cn } from "@/lib/cn";
import { plural } from "@/lib/text";
import { MergedChart, TimeToMergeChart } from "./health-charts";
import { durationOf, percentOf } from "./health-format";
import { PeriodPicker } from "./period-picker";

/** The share of the PRs that merged or failed that failed. */
function failureRateOf({ merged, failed }: HealthTotals): number {
  return merged + failed === 0 ? 0 : failed / (merged + failed);
}

/** What the queue did over `period`, against the period before. */
export function HealthPane({
  path,
  period,
  branch,
}: {
  path: QueuePath;
  period: HealthPeriod;
  branch: string;
}) {
  const { data: health } = useSuspenseQuery(healthOptions(path, period));
  const now = useNow();
  const { current } = health;
  const isQuiet = current.merged + current.failed === 0;
  const words = periodWords(period, health, now);
  const grain = grainOf(Date.parse(health.to) - Date.parse(health.from));
  const longer = "range" in period && period.range === "24h" ? "7d" : "30d";

  return (
    <>
      <div className="flex flex-wrap items-center gap-4">
        <Verdict health={health} words={words} />
        <RangePicker path={path} period={period} now={now} />
      </div>
      {isQuiet ? (
        <Card>
          <p className="p-7 text-center text-secondary-foreground">
            Nothing to measure in this period.{" "}
            {!("range" in period && period.range === "30d") && (
              <Link
                className="text-primary hover:underline"
                to="/orgs/$org/repos/$repo/queues/$queue"
                params={path}
                search={{ pane: "health", range: longer }}
              >
                Show the last {RANGE_LABELS[longer]}
              </Link>
            )}
          </p>
        </Card>
      ) : (
        <>
          <Stats health={health} words={words} now={now} />
          <div className="grid grid-cols-2 items-start gap-4 max-md:grid-cols-1">
            <ChartCard
              title="Merged and failed"
              keys={[
                <Key key="m" swatch="bg-primary" label="Merged" />,
                <Key key="f" swatch="bg-destructive" label="Failed" />,
              ]}
            >
              <MergedChart buckets={health.buckets} grain={grain} />
            </ChartCard>
            <ChartCard
              title="Time to merge"
              keys={[
                <Key key="m" swatch="h-0.5 w-3 bg-primary" label="Median" />,
                <Key
                  key="p"
                  swatch="h-0 w-3 rounded-none border-t-[1.5px] border-dashed border-primary-border"
                  label="90%"
                />,
              ]}
            >
              <TimeToMergeChart buckets={health.buckets} grain={grain} />
            </ChartCard>
          </div>
          <div className="grid grid-cols-2 items-start gap-4 max-md:grid-cols-1">
            <TimeShare states={health.time_in_states} branch={branch} />
            <LeftWithoutMerge health={health} />
          </div>
          <Checks health={health} words={words} />
        </>
      )}
    </>
  );
}

const VERDICT_TONES = {
  quiet: "bg-waiting",
  healthy: "bg-success",
  many_failures: "bg-warning",
  check_passes_on_retry: "bg-warning",
};

/** The first thing to know about the queue, in the words of its verdict. */
function Verdict({ health, words }: { health: QueueHealth; words: PeriodWords }) {
  const { verdict, current } = health;
  const { during } = words;
  const [title, text]: [ReactNode, string] =
    verdict.kind === "quiet"
      ? ["Quiet", `Nothing merged or failed ${during}.`]
      : verdict.kind === "healthy"
        ? [
            "Healthy",
            `${String(current.merged)} merged ${during}, and ${percentOf(failureRateOf(current))} failed.`,
          ]
        : verdict.kind === "many_failures"
          ? [
              "Many pull requests fail",
              `${percentOf(failureRateOf(current))} of the pull requests failed ${during}.`,
            ]
          : [
              <>
                <code>{verdict.check}</code> looks flaky
              </>,
              `It failed, then passed on a retry, ${plural(verdict.times, "time", "times")} ${during}. Each retry costs a run and makes the pull requests behind wait.`,
            ];

  return (
    <div className="flex min-w-0 flex-[1_1_360px] items-baseline gap-2 text-base text-secondary-foreground">
      <span className={cn("size-2 flex-none self-center rounded-full", VERDICT_TONES[verdict.kind])} />
      <b className="font-medium whitespace-nowrap text-foreground">{title}</b>
      <span>{text}</span>
    </div>
  );
}

/** The periods of the Health: the last 24 hours, 7 days or 30 days, or Custom, a period that the person chooses. */
function RangePicker({ path, period, now }: { path: QueuePath; period: HealthPeriod; now: number }) {
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

function Stats({ health, words, now }: { health: QueueHealth; words: PeriodWords; now: number }) {
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

function Key({ swatch, label }: { swatch: string; label: string }) {
  return (
    <span className="inline-flex items-center gap-1.25">
      <i className={cn("inline-block size-2 flex-none rounded-xs", swatch)} />
      {label}
    </span>
  );
}

function ChartCard({ title, keys, children }: { title: string; keys: ReactNode[]; children: ReactNode }) {
  return (
    <Card className="min-w-0">
      <CardHeader>
        <CardTitle>{title}</CardTitle>
        <CardHint className="flex gap-3">{keys}</CardHint>
      </CardHeader>
      {children}
    </Card>
  );
}

/** The parts of the life of a merged PR, and the states of each part. */
const SHARES = [
  { label: "Waiting to enter", states: ["not_ready"], swatch: "border-[1.5px] border-dashed border-waiting" },
  { label: "Queued", states: ["pending"], swatch: "bg-waiting" },
  { label: "Testing", states: ["testing", "pending_failure"], swatch: "bg-primary" },
  { label: "Merging", states: ["tests_passed"], swatch: "bg-success" },
] as const;

function noteOf(label: (typeof SHARES)[number]["label"], branch: string): ReactNode {
  switch (label) {
    case "Waiting to enter":
      return "its approvals and the required checks of the PR";
    case "Queued":
      return "it waits for room in the window";
    case "Testing":
      return "the run of its tests";
    case "Merging":
      return (
        <>
          CIHub merges it into <code>{branch}</code>
        </>
      );
  }
}

/** Where the median merged PR spent its time, from its submission to its merge. */
function TimeShare({ states, branch }: { states: StateTime[]; branch: string }) {
  const shares = SHARES.map((share) => ({
    ...share,
    seconds: states
      .filter((state) => (share.states as readonly string[]).includes(state.state))
      .reduce((sum, state) => sum + state.median_seconds, 0),
  }));

  return (
    <Card className="min-w-0">
      <CardHeader>
        <CardTitle>Where the time goes</CardTitle>
        <CardHint>The median pull request, from its submission to its merge</CardHint>
      </CardHeader>
      <div className="grid gap-3.5 p-4">
        <div className="flex h-2.5 gap-0.5">
          {shares.map((share) =>
            share.seconds > 0 ? (
              <span
                key={share.label}
                className={cn("min-w-0.75 rounded-xs", share.swatch)}
                style={{ flex: share.seconds }}
                title={`${share.label}: ${durationOf(share.seconds)}`}
              />
            ) : null,
          )}
        </div>
        <ul className="grid gap-2.5">
          {shares.map((share) => (
            <li
              key={share.label}
              className="grid grid-cols-[8px_minmax(0,1fr)_auto] items-baseline gap-2.5 text-base"
            >
              <i className={cn("inline-block size-2 rounded-xs", share.swatch)} />
              <div>
                <b className="mr-1.5 font-medium">{share.label}</b>
                <span className="text-md text-muted-foreground">{noteOf(share.label, branch)}</span>
              </div>
              <span className="tabular-nums">{durationOf(share.seconds)}</span>
            </li>
          ))}
        </ul>
      </div>
    </Card>
  );
}

/** Why the PRs that did not merge left, in the words of the API. */
function LeftWithoutMerge({ health }: { health: QueueHealth }) {
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

const HEAD_CELL = "border-b border-divider px-4 py-2 text-left text-md font-medium text-secondary-foreground";

const CELL = "px-4 py-2.25";

/** The checks of the runs of the period, and the one that the verdict finds flaky. */
function Checks({ health, words }: { health: QueueHealth; words: PeriodWords }) {
  const flaky = health.verdict.kind === "check_passes_on_retry" ? health.verdict.check : null;

  return (
    <Card className="min-w-0">
      <CardHeader>
        <CardTitle>Checks</CardTitle>
        <CardHint>On the test runs of the queue, {words.over}</CardHint>
      </CardHeader>
      <div className="overflow-x-auto">
        <table className="w-full min-w-130">
          <thead>
            <tr>
              <th className={HEAD_CELL}>Check</th>
              <th className={cn(HEAD_CELL, "text-right")}>Runs</th>
              <th className={cn(HEAD_CELL, "text-right")}>Failed</th>
              <th className={cn(HEAD_CELL, "text-right")}>Passed on a retry</th>
            </tr>
          </thead>
          <tbody>
            {health.checks.map((check: CheckHealth) => (
              <tr key={`${check.kind}-${check.name}`} className="border-b border-divider last:border-b-0">
                <td className={CELL}>
                  <code>{check.name}</code>
                  {!check.required && <Tag>not required</Tag>}
                  {check.name === flaky && <Tag isWarning>flaky</Tag>}
                </td>
                <td className={cn(CELL, "text-right tabular-nums")}>{check.runs}</td>
                <td className={cn(CELL, "text-right tabular-nums")}>
                  {check.failed}
                  <span className="ml-1.5 text-sm text-muted-foreground">
                    {percentOf(check.failed / check.runs)}
                  </span>
                </td>
                <td className={cn(CELL, "text-right tabular-nums")}>{check.passed_on_retry}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </Card>
  );
}

function Tag({ isWarning = false, children }: { isWarning?: boolean; children: ReactNode }) {
  return (
    <span
      className={cn(
        "ml-2 rounded-xs px-1.25 text-2xs",
        isWarning ? "bg-warning-muted font-medium text-warning" : "bg-muted text-muted-foreground",
      )}
    >
      {children}
    </span>
  );
}

/** The Health while it loads: its verdict, its counters and its charts. */
export function HealthSkeleton() {
  return (
    <>
      <LoadingStatus />
      <Skeleton className="h-4 w-80" />
      <div className="grid grid-cols-3 gap-3 max-md:grid-cols-1">
        {[1, 2, 3].map((stat) => (
          <Card key={stat} className="grid gap-2.5 px-3.5 py-3">
            <Skeleton className="h-2.25 w-20" />
            <Skeleton className="h-6 w-15" />
            <Skeleton className="h-2.25 w-32.5" />
          </Card>
        ))}
      </div>
      <div className="grid grid-cols-2 gap-4 max-md:grid-cols-1">
        {[1, 2].map((chart) => (
          <Card key={chart}>
            <CardHeader>
              <Skeleton className="w-27.5" />
            </CardHeader>
            <div className="p-4">
              <Skeleton className="h-37.5" />
            </div>
          </Card>
        ))}
      </div>
    </>
  );
}

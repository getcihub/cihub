import { useSuspenseQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";

import { Card } from "@cihub/ui/components/card";
import { type HealthPeriod, RANGE_LABELS, grainOf, periodWords } from "@/domain/queue/period";
import { type QueuePath, healthOptions } from "@/domain/queue/queries";
import { useNow } from "@/hooks/use-now";
import { ChartCard, Key } from "./chart-card";
import { Checks } from "./checks";
import { MergedChart, TimeToMergeChart } from "./health-charts";
import { HealthStats } from "./health-stats";
import { LeftWithoutMerge } from "./left-without-merge";
import { RangePicker } from "./range-picker";
import { TimeShare } from "./time-share";
import { Verdict } from "./verdict";

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
          <HealthStats health={health} words={words} now={now} />
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

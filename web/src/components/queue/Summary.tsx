import type { ReactNode } from "react";

import type { Queue } from "@/client/types.gen";
import { cn } from "@/lib/cn";
import { plural, shortSha } from "@/lib/text";
import { timeAgo } from "@/lib/time";
import { inQueueCount, testingCount } from "@/domain/queue/counts";
import { Card, cardVariants } from "@/ui/card";

const MERGE_METHODS: Record<string, string> = {
  squash: "Squash",
  merge_commit: "Merge commit",
  rebase: "Rebase",
  fast_forward: "Fast-forward",
};

function Stat({ label, value, hint }: { label: string; value: number; hint: string }) {
  return (
    <Card className="px-3.5 py-3">
      <div className="text-md text-secondary-foreground">{label}</div>
      <div className="mt-0.5 text-5xl font-medium tracking-[-0.3px] tabular-nums">{value}</div>
      <div className="text-md text-muted-foreground">{hint}</div>
    </Card>
  );
}

/** The counters of the queue: what waits, what is tested, and the last 24 hours. */
export function Stats({ queue, now }: { queue: Queue; now: number }) {
  const { counts, live_runs, merged_last_24h, failed_last_24h, last_merged_at } = queue;

  return (
    <div className="grid grid-cols-4 gap-3 max-md:grid-cols-2">
      <Stat
        label="In the queue"
        value={inQueueCount(counts)}
        hint={
          counts.not_ready > 0
            ? `${String(counts.not_ready)} of them waiting to enter`
            : "all of them in line"
        }
      />
      <Stat
        label="Testing"
        value={testingCount(counts)}
        hint={live_runs > 0 ? `pull requests in ${plural(live_runs, "run", "runs")}` : "no run in progress"}
      />
      <Stat
        label="Merged in 24 h"
        value={merged_last_24h}
        hint={last_merged_at ? `last one ${timeAgo(last_merged_at, now)}` : "no merge yet"}
      />
      <Stat
        label="Failed in 24 h"
        value={failed_last_24h}
        hint={failed_last_24h > 0 ? "see the tab Failed and cancelled" : "nothing failed"}
      />
    </div>
  );
}

/** The settings in force, from the last valid file of the queue, or the defaults. */
export function Settings({ queue }: { queue: Queue }) {
  const { window, settings } = queue;
  const batch = settings.batching_enabled
    ? `Up to ${String(settings.batching_size)}, wait ${String(settings.batching_max_wait_minutes)} min`
    : "Off";
  const strategy = queue.strategy.charAt(0).toUpperCase() + queue.strategy.slice(1);

  return (
    <dl className={cn(cardVariants(), "flex flex-wrap")}>
      <Setting label="Strategy">{strategy}</Setting>
      <Setting label="Batch">{batch}</Setting>
      <Setting label="Window">
        {window.current} now · {window.floor} to {window.ceiling}
      </Setting>
      <Setting label="Failure budget">{plural(settings.failure_budget_retries, "retry", "retries")}</Setting>
      <Setting label="Merge method">{MERGE_METHODS[settings.merge_method] ?? settings.merge_method}</Setting>
      <Setting className="ml-auto" label="Configuration">
        {queue.has_config_file ? (
          <code>.cihub.yaml{queue.config_sha && ` @ ${shortSha(queue.config_sha)}`}</code>
        ) : (
          <>
            Defaults · no <code>.cihub.yaml</code>
          </>
        )}
      </Setting>
    </dl>
  );
}

function Setting({ label, className, children }: { label: string; className?: string; children: ReactNode }) {
  return (
    <div className={cn("border-r border-divider px-3.5 py-2.5 last:border-r-0", className)}>
      <dt className="text-md text-secondary-foreground">{label}</dt>
      <dd className="mt-px text-base font-medium">{children}</dd>
    </div>
  );
}

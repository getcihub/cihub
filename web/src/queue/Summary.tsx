import type { QueueDetail } from "../client/types.gen";
import { timeAgo } from "../time";
import styles from "./queue.module.css";

const MERGE_METHODS: Record<string, string> = {
  squash: "Squash",
  merge_commit: "Merge commit",
  rebase: "Rebase",
  fast_forward: "Fast-forward",
};

function plural(count: number, one: string, many: string): string {
  return `${String(count)} ${count === 1 ? one : many}`;
}

function Stat({ label, value, hint }: { label: string; value: number; hint: string }) {
  return (
    <div className={styles.stat}>
      <div className={styles.label}>{label}</div>
      <div className={styles.value}>{value}</div>
      <div className={styles.hint}>{hint}</div>
    </div>
  );
}

/** The counters of the queue: what waits, what is tested, and the last 24 hours. */
export function Stats({ detail, now }: { detail: QueueDetail; now: number }) {
  const { counts, live_runs, merged_last_24h, failed_last_24h, last_merged_at } = detail;
  const inQueue =
    counts.not_ready + counts.pending + counts.testing + counts.pending_failure + counts.tests_passed;
  const testing = counts.testing + counts.pending_failure + counts.tests_passed;

  return (
    <div className={styles.stats}>
      <Stat
        label="In the queue"
        value={inQueue}
        hint={
          counts.not_ready > 0
            ? `${String(counts.not_ready)} of them waiting to enter`
            : "all of them in line"
        }
      />
      <Stat
        label="Testing"
        value={testing}
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
export function Settings({ detail }: { detail: QueueDetail }) {
  const { queue, window, settings } = detail;
  const batch = settings.batching_enabled
    ? `Up to ${String(settings.batching_size)}, wait ${String(settings.batching_max_wait_minutes)} min`
    : "Off";
  const strategy = queue.strategy.charAt(0).toUpperCase() + queue.strategy.slice(1);

  return (
    <dl className={styles.settings}>
      <div>
        <dt>Strategy</dt>
        <dd>{strategy}</dd>
      </div>
      <div>
        <dt>Batch</dt>
        <dd>{batch}</dd>
      </div>
      <div>
        <dt>Window</dt>
        <dd>
          {window.current} now · {window.floor} to {window.ceiling}
        </dd>
      </div>
      <div>
        <dt>Failure budget</dt>
        <dd>{plural(settings.failure_budget_retries, "retry", "retries")}</dd>
      </div>
      <div>
        <dt>Merge method</dt>
        <dd>{MERGE_METHODS[settings.merge_method] ?? settings.merge_method}</dd>
      </div>
      <div className={styles.configuration}>
        <dt>Configuration</dt>
        <dd>
          {queue.has_config_file ? (
            <code>.cihub.yaml{queue.config_sha && ` @ ${queue.config_sha.slice(0, 7)}`}</code>
          ) : (
            <>
              Defaults · no <code>.cihub.yaml</code>
            </>
          )}
        </dd>
      </div>
    </dl>
  );
}

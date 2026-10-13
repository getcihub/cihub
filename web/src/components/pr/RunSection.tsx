import { useSuspenseQuery } from "@tanstack/react-query";
import type { ReactNode } from "react";

import { getRunOptions } from "@/client/@tanstack/react-query.gen";
import { cn } from "@/lib/cn";
import { shortSha } from "@/lib/text";
import { dateAndTime } from "@/lib/time";
import { Skeleton } from "@/ui/skeleton";
import { DrawerNote, DrawerSection, DrawerSectionTitle } from "./DrawerFrame";

const RUN_STATUSES: Record<string, string> = {
  assembling: "Assembling",
  in_progress: "Testing",
  succeeded: "Passed",
  failed: "Failed",
  interrupted: "Interrupted",
  cancelled: "Cancelled",
  timed_out: "Timed out",
};

const CONCLUSION_LABELS: Record<string, string> = {
  success: "passed",
  failure: "failed",
  timed_out: "timed out",
  action_required: "action required",
  neutral: "neutral",
  skipped: "skipped",
  cancelled: "cancelled",
  stale: "stale",
};

const CONCLUSIONS: Record<string, string> = {
  success: "border-success bg-success",
  failure: "border-destructive bg-destructive",
  timed_out: "border-destructive bg-destructive",
  action_required: "border-destructive bg-destructive",
  neutral: "border-waiting bg-waiting",
  skipped: "border-waiting bg-waiting",
  cancelled: "border-waiting bg-waiting",
  stale: "border-waiting bg-waiting",
};

/** The run of a PR in its drawer: what it tests, on top of what, and the checks of its test commit. */
export function RunSection({ org, runId }: { org: string; runId: string }) {
  const { data: run } = useSuspenseQuery(getRunOptions({ path: { org, run_id: runId } }));
  const prs = (numbers: number[]) => numbers.map((number) => `#${String(number)}`).join(", ");

  return (
    <>
      <DrawerSection aria-labelledby="run-title">
        <DrawerSectionTitle id="run-title">
          Run · attempt {run.attempt} · {RUN_STATUSES[run.status] ?? run.status}
        </DrawerSectionTitle>
        <dl className="grid grid-cols-[96px_minmax(0,1fr)] gap-x-3 gap-y-2 text-md">
          <Fact label="Test branch">
            <code>{run.test_branch}</code>
          </Fact>
          {run.draft_pr_number != null && <Fact label="Draft PR">#{run.draft_pr_number} · runs your CI</Fact>}
          <Fact label="Base">
            <code>{shortSha(run.base_sha)}</code>
          </Fact>
          <Fact label="Tests">{prs(run.tested_pr_numbers)}</Fact>
          <Fact label="On top of">
            {run.predecessor_pr_numbers.length > 0 ? prs(run.predecessor_pr_numbers) : "nothing"}
          </Fact>
          <Fact label="Started">{dateAndTime(run.started_at)}</Fact>
        </dl>
      </DrawerSection>
      <DrawerSection aria-labelledby="checks-title">
        <DrawerSectionTitle id="checks-title">Checks</DrawerSectionTitle>
        {run.checks.length === 0 ? (
          <DrawerNote>No check reported on the test commit yet.</DrawerNote>
        ) : (
          <ul className="grid gap-0.5">
            {run.checks.map((check) => (
              <li key={`${check.kind}-${check.name}`} className="flex items-center gap-2.5 py-1.25 text-md">
                <span
                  className={cn(
                    "size-2 flex-none rounded-full border-2 border-primary",
                    CONCLUSIONS[check.conclusion ?? ""],
                  )}
                />
                <span className="font-medium">{check.name}</span>
                {!check.required && <span className="text-muted-foreground">not required</span>}
                <span className="ml-auto text-muted-foreground">
                  {check.conclusion ? (CONCLUSION_LABELS[check.conclusion] ?? check.conclusion) : "running"}
                </span>
              </li>
            ))}
          </ul>
        )}
      </DrawerSection>
    </>
  );
}

function Fact({ label, children }: { label: string; children: ReactNode }) {
  return (
    <>
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="wrap-anywhere">{children}</dd>
    </>
  );
}

/** The run while it loads: its title and its facts. */
export function RunSectionSkeleton() {
  return (
    <DrawerSection>
      <Skeleton className="mb-3 w-40" />
      <div className="grid gap-2.5">
        {[1, 2, 3, 4].map((fact) => (
          <Skeleton key={fact} className="w-[70%]" />
        ))}
      </div>
    </DrawerSection>
  );
}

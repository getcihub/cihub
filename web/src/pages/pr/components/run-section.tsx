import { useSuspenseQuery } from "@tanstack/react-query";
import type { ReactNode } from "react";

import { getRunOptions } from "@/client/@tanstack/react-query.gen";
import type { CheckConclusion } from "@/client/types.gen";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/cn";
import { shortSha } from "@/lib/text";
import { dateAndTime } from "@/lib/time";
import { DrawerNote, DrawerSection, DrawerSectionTitle } from "./drawer-frame";

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

const FAILURES = new Set<CheckConclusion>(["failure", "error", "timed_out", "action_required"]);

/** The mark of a check: passed, failed, running (it turns), or ended without a verdict. */
function CheckMark({ conclusion }: { conclusion: CheckConclusion | null | undefined }) {
  const mark = "size-4 flex-none";

  if (!conclusion) {
    return (
      <svg
        className={cn(mark, "motion-safe:animate-spin")}
        viewBox="0 0 16 16"
        fill="none"
        aria-hidden="true"
      >
        <circle cx="8" cy="8" r="6" className="stroke-warning-border" strokeWidth="2" />
        <path d="M8 2 A6 6 0 0 1 14 8" className="stroke-warning" strokeWidth="2" strokeLinecap="round" />
      </svg>
    );
  }
  if (conclusion === "success") {
    return (
      <svg className={mark} viewBox="0 0 16 16" aria-hidden="true">
        <circle cx="8" cy="8" r="7" className="fill-success" />
        <path
          d="M5 8.3 L7 10.3 L11 6"
          className="stroke-primary-foreground"
          strokeWidth="1.8"
          fill="none"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </svg>
    );
  }
  if (FAILURES.has(conclusion)) {
    return (
      <svg className={mark} viewBox="0 0 16 16" aria-hidden="true">
        <circle cx="8" cy="8" r="7" className="fill-destructive" />
        <path
          d="M5.5 5.5 L10.5 10.5 M10.5 5.5 L5.5 10.5"
          className="stroke-primary-foreground"
          strokeWidth="1.8"
          strokeLinecap="round"
        />
      </svg>
    );
  }
  return (
    <svg className={mark} viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <circle cx="8" cy="8" r="6" className="stroke-waiting" strokeWidth="2" strokeDasharray="3 3" />
    </svg>
  );
}

/** The run of a PR in its drawer: what it tests, on top of what, and the checks of its test commit. */
export function RunSection({ org, repo, runId }: { org: string; repo: string; runId: string }) {
  const { data: run } = useSuspenseQuery(getRunOptions({ path: { org, repo, run_id: runId } }));
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
                <CheckMark conclusion={check.conclusion} />
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

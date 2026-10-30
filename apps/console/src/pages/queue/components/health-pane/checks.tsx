import type { ReactNode } from "react";

import type { CheckHealth, QueueHealth } from "@/client/types.gen";
import { Card, CardHeader, CardHint, CardTitle } from "@cihub/ui/components/card";
import { cn } from "@cihub/ui/lib/cn";
import type { PeriodWords } from "@/domain/queue/period";
import { percentOf } from "./health-format";

const HEAD_CELL = "border-b border-divider px-4 py-2 text-left text-md font-medium text-secondary-foreground";

const CELL = "px-4 py-2.25";

/** The checks of the runs of the period, and the one that the verdict finds flaky. */
export function Checks({ health, words }: { health: QueueHealth; words: PeriodWords }) {
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

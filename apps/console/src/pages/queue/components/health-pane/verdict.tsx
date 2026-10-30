import type { ReactNode } from "react";

import type { QueueHealth } from "@/client/types.gen";
import { cn } from "@cihub/ui/lib/cn";
import type { PeriodWords } from "@/domain/queue/period";
import { plural } from "@/lib/text";
import { failureRateOf, percentOf } from "./health-format";

const VERDICT_TONES = {
  quiet: "bg-waiting",
  healthy: "bg-success",
  many_failures: "bg-warning",
  check_passes_on_retry: "bg-warning",
};

/** The first thing to know about the queue, in the words of its verdict. */
export function Verdict({ health, words }: { health: QueueHealth; words: PeriodWords }) {
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

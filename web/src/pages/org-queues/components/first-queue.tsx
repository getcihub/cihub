import type { ReactNode } from "react";

import type { Repo } from "@/client/types.gen";
import { CheckIcon } from "@/components/icons/check-icon";
import { COMMAND } from "@/components/queue/copy-command";
import { Button, buttonVariants } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { QueueIllustration } from "@/components/ui/illustrations";
import { cn } from "@/lib/cn";
import { plural } from "@/lib/text";
import { type Verdict, isBlockedVerdict } from "@/pages/org-queues/use-setup-verdicts";

const STEPS: { title: ReactNode; text: string }[] = [
  { title: "Pick a repository", text: "Start with one where pull requests merge often." },
  {
    title: "Check its setup",
    text: "CIHub reads the branch rules and the configuration, and says what to fix.",
  },
  {
    title: (
      <>
        Comment <code>{COMMAND}</code>
      </>
    ),
    text: "On an approved pull request. The queue starts there.",
  },
];

/**
 * The organization has no queue yet: what a merge queue does, the three steps to the first one, and where to
 * start. The steps follow the open row: a repository that needs a fix, or that CIHub still checks, is at step 2;
 * one that is ready, at step 3.
 */
export function FirstQueue({
  org,
  count,
  repos,
  verdictOf,
  open,
  installUrl,
  onStart,
}: {
  org: string;
  /** How many repositories the App reaches. */
  count: number;
  repos: Repo[];
  verdictOf: (repo: Repo) => Verdict;
  open: string | undefined;
  /** Where an owner adds repositories to the App; the instance may give none. */
  installUrl: string | null | undefined;
  onStart: (repo: Repo) => void;
}) {
  const ready = repos.filter((repo) => isReady(verdictOf(repo)));
  const opened = repos.find((repo) => repo.name === open);
  const step = !opened ? 1 : needsWork(verdictOf(opened)) ? 2 : 3;
  const pick = opened ? undefined : ready[0];

  return (
    <Card className="overflow-hidden p-0">
      <div className="flex items-center gap-8 px-6 py-5.5 max-md:flex-col-reverse max-md:items-start max-md:gap-4">
        <div className="min-w-0 flex-1">
          <h2 className="mb-1.5 text-4xl font-semibold tracking-[-0.2px]">Start your first merge queue</h2>
          <p className="max-w-145 text-lg text-secondary-foreground">
            CIHub tests each pull request on top of the ones ahead of it, and merges only what still passes.
            The App reaches {plural(count, "repository", "repositories")} of {org}:{" "}
            {readyText(ready.length, count)}.
          </p>
          {(pick ?? installUrl) && (
            <div className="mt-3.5 flex flex-wrap gap-2">
              {pick && (
                <Button
                  variant="primary"
                  onClick={() => {
                    onStart(pick);
                  }}
                >
                  Start with {pick.owner}/{pick.name}
                </Button>
              )}
              {installUrl && (
                <a className={buttonVariants()} href={installUrl}>
                  Add repositories
                </a>
              )}
            </div>
          )}
        </div>
        <QueueIllustration className="m-0 w-55 max-md:w-55" />
      </div>
      <ol className="grid grid-cols-3 border-t border-divider bg-muted max-md:grid-cols-1">
        {STEPS.map(({ title, text }, index) => {
          const number = index + 1;
          const isDone = number < step;
          const isNow = number === step;
          return (
            <li
              key={number}
              aria-current={isNow ? "step" : undefined}
              className={cn(
                "relative flex gap-3 border-l border-divider px-5 py-3.5 text-muted-foreground first:border-l-0 max-md:border-t max-md:border-l-0 max-md:first:border-t-0",
                isNow && "bg-card text-foreground",
              )}
            >
              {isNow && <span className="absolute inset-x-0 top-0 h-0.5 bg-primary" />}
              <span
                className={cn(
                  "grid size-5.5 flex-none place-items-center rounded-full border border-border bg-card text-xs font-semibold [&>svg]:size-3",
                  isNow && "border-primary bg-primary text-primary-foreground",
                  isDone && "border-success-muted bg-success-muted text-success",
                )}
              >
                {isDone ? <CheckIcon /> : number}
              </span>
              <div>
                <b className="block text-base font-medium">{title}</b>
                <p className={cn("mt-0.5 text-md", isNow && "text-secondary-foreground")}>{text}</p>
              </div>
            </li>
          );
        })}
      </ol>
    </Card>
  );
}

/** The setup is not read yet, or a point of it blocks the merges. */
function needsWork(verdict: Verdict): boolean {
  return verdict === null || isBlockedVerdict(verdict);
}

/** The repository can start its first queue: its setup was read, and nothing in it blocks the merges. */
function isReady(verdict: Verdict): boolean {
  return !needsWork(verdict) && verdict !== "started";
}

function readyText(ready: number, count: number): string {
  if (ready === 0) {
    return "none is ready yet: open one below to see what to fix";
  }
  if (ready === count) {
    return count > 1 ? "all are ready to start" : "it is ready to start";
  }
  return `${String(ready)} ${ready > 1 ? "are" : "is"} ready to start`;
}

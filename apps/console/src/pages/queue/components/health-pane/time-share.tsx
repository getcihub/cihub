import type { ReactNode } from "react";

import type { StateTime } from "@/client/types.gen";
import { Card, CardHeader, CardHint, CardTitle } from "@cihub/ui/components/card";
import { cn } from "@cihub/ui/lib/cn";
import { durationOf } from "./health-format";

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
export function TimeShare({ states, branch }: { states: StateTime[]; branch: string }) {
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

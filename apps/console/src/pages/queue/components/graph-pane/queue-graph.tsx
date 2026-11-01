import { Fragment, useRef } from "react";

import type { Graph, GraphRun, Pr, Queue } from "@/client/types.gen";
import { CheckIcon } from "@cihub/icons/check";
import { CardHeader, CardHint, CardTitle, cardVariants } from "@cihub/ui/components/card";
import { useFlip } from "@cihub/ui/hooks/use-flip";
import { cn } from "@cihub/ui/lib/cn";
import { itemStateLabel } from "@/domain/pr/words";
import { isStopped } from "@/domain/queue/attention";
import { shortSha } from "@/lib/text";
import { Chip } from "./graph-chip";
import { LiveMark } from "./live-mark";

type RunLook = "testing" | "failing" | "passed";

const RUN_BOXES: Record<RunLook, string> = {
  testing: "border-dashed border-primary-border bg-primary-muted",
  failing: "border-warning-border bg-warning-muted",
  passed: "border-success bg-success-muted",
};

const RUN_FEET: Record<RunLook, string> = {
  testing: "text-primary",
  failing: "text-warning",
  passed: "text-success",
};

const LANE = "relative flex flex-col justify-center gap-2 py-6.5";

const LANE_LABEL = "absolute top-0 left-0 text-xs font-medium whitespace-nowrap text-muted-foreground";

/** The commits of the branch before its head, which fade: a picture of the branch, as the API does not count them. */
const OLDER_COMMITS = ["opacity-100", "opacity-86", "opacity-72"];

/** The link between two lanes; the last one stretches to the branch. */
const LINK = "w-7 self-center border-t-[1.5px] border-dashed border-waiting";

/** The look of a run follows the state of its PRs: the API says what each state means. */
function lookOf(members: Pr[]): RunLook {
  if (members.some((item) => item.state === "pending_failure")) {
    return "failing";
  }
  if (members.length > 0 && members.every((item) => item.state === "tests_passed")) {
    return "passed";
  }
  return "testing";
}

function footOf(look: RunLook, run: GraphRun, members: Pr[]): string {
  const state = members.find((item) => item.state !== "testing")?.state;

  return look === "testing" || !state ? `Testing · attempt ${String(run.attempt)}` : itemStateLabel(state);
}

/** The smallest rank of the PRs of `run`: the run of the head of the queue merges first. */
function rankOf(run: GraphRun, ranks: Map<string, number>): number {
  return Math.min(...run.item_ids.map((id) => ranks.get(id) ?? Number.MAX_SAFE_INTEGER));
}

interface Lane {
  key: string;
  label: string;
  items: Pr[];
  /** What a chip says after the number of its PR. */
  textOf: (item: Pr) => string | null;
  /** The box of a run around its chips. */
  box?: string;
  foot?: string;
  footClass?: string;
}

const titleOf = (item: Pr) => item.title ?? null;

/**
 * Read from left to right: the PRs flow toward the target branch. The run of the head of the queue merges first,
 * so it is next to the branch; the items that wait are on the left. `ranks` comes from every live item of the
 * graph, `items` gives their titles.
 */
export function QueueGraph({
  queue,
  graph,
  items,
  ranks,
}: {
  queue: Queue;
  graph: Graph;
  items: Pr[];
  ranks: Map<string, number>;
}) {
  const graphRef = useRef<HTMLDivElement>(null);
  useFlip(graphRef);

  const byId = new Map(items.map((item) => [item.id, item]));
  const notReady = items.filter((item) => item.state === "not_ready");
  const queued = items.filter((item) => item.state === "pending");
  const runs = [...graph.runs].sort((a, b) => rankOf(a, ranks) - rankOf(b, ranks));
  const runLanes = runs.map((run, index): Lane => {
    const members = run.item_ids.flatMap((id) => byId.get(id) ?? []);
    const look = lookOf(members);

    return {
      key: run.run_id,
      label: `Run ${String(index + 1)}${run.item_ids.length > 1 ? " · batch" : ""}`,
      items: members,
      textOf: titleOf,
      box: cn("min-w-57.5 rounded-md border p-2", RUN_BOXES[look]),
      foot: footOf(look, run, members),
      footClass: RUN_FEET[look],
    };
  });
  const waitingLanes: Lane[] = [
    { key: "not-ready", label: "Not ready", items: notReady, textOf: (item: Pr) => item.state_description },
    {
      key: "queued",
      label: "Queued",
      items: queued,
      textOf: titleOf,
      foot: isStopped(queue) ? "Paused" : "Waits for room in the window",
    },
  ];
  const lanes = [...waitingLanes.filter((lane) => lane.items.length > 0), ...runLanes.reverse()];

  return (
    <section className={cn(cardVariants(), "min-w-0")} aria-labelledby="graph-title">
      <CardHeader>
        <CardTitle id="graph-title">Live graph</CardTitle>
        <LiveMark queue={queue} />
        <CardHint className="max-md:hidden">
          Each run tests its pull requests on top of the runs on its right.
        </CardHint>
      </CardHeader>
      <div ref={graphRef} className="flex min-h-37.5 items-stretch overflow-x-auto px-4 py-3.5">
        {lanes.length === 0 && (
          <>
            <p className="mr-3 self-center text-muted-foreground">
              No run in progress: <code>{queue.target_branch}</code> is up to date and green.
            </p>
            <span className={cn(LINK, "flex-[1_0_28px]")} />
          </>
        )}
        {lanes.map((lane, index) => (
          <Fragment key={lane.key}>
            <div className={LANE} role="group" aria-label={lane.label}>
              <span className={LANE_LABEL}>{lane.label}</span>
              <ul className={cn("grid gap-1.5", lane.box)}>
                {lane.items.map((item) => (
                  <Chip key={item.id} item={item} text={lane.textOf(item)} rank={ranks.get(item.id)} />
                ))}
              </ul>
              {lane.foot && (
                <span
                  className={cn(
                    "absolute right-0 bottom-0 left-0 text-center text-xs font-medium whitespace-nowrap text-muted-foreground",
                    lane.footClass,
                  )}
                >
                  {lane.foot}
                </span>
              )}
            </div>
            <span className={cn(LINK, index === lanes.length - 1 && "flex-[1_0_28px]")} />
          </Fragment>
        ))}
        <div className={LANE} role="group" aria-label={`Branch ${queue.target_branch}`}>
          <span className={LANE_LABEL}>{queue.target_branch}</span>
          <div className="relative flex items-center py-6.5">
            {/* A new head comes in: the merged PR went into its branch. */}
            <span
              key={graph.target_head_sha}
              className="grid size-4.5 place-items-center rounded-full bg-primary text-primary-foreground motion-safe:animate-arrive [&_svg]:size-3"
            >
              <CheckIcon />
            </span>
            {OLDER_COMMITS.map((opacity) => (
              <Fragment key={opacity}>
                <span className={cn("h-[1.5px] w-5 bg-primary", opacity)} />
                <span className={cn("size-2 flex-none rounded-full bg-primary", opacity)} />
              </Fragment>
            ))}
            {graph.target_head_sha && (
              <code className="absolute bottom-1 left-0 bg-transparent p-0 font-mono text-2xs text-muted-foreground">
                {shortSha(graph.target_head_sha)}
              </code>
            )}
          </div>
        </div>
      </div>
    </section>
  );
}

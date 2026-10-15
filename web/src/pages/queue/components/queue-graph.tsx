import { Fragment, useRef, useSyncExternalStore } from "react";

import type { Graph, GraphRun, Pr, Queue } from "@/client/types.gen";
import { CheckIcon } from "@/components/icons/check-icon";
import { CardHeader, CardHint, CardTitle, cardVariants } from "@/components/ui/card";
import { useFlip } from "@/components/ui/hooks/use-flip";
import { itemStateLabel } from "@/domain/pr/words";
import { isStreamOpen, onStreamOpenChanged } from "@/domain/queue/live";
import { cn } from "@/lib/cn";
import { shortSha } from "@/lib/text";
import { PrLink } from "./pr-link";

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

const DOTS: Record<string, string> = {
  not_ready: "border-2 border-dashed border-waiting",
  pending: "border-2 border-waiting",
  testing: "animate-spin border-2 border-primary-border border-t-primary motion-reduce:animate-none",
  pending_failure: "bg-warning",
  tests_passed: "bg-success",
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

/**
 * Whether the graph moves: the stream brings the changes, and the queue tests and merges. A draining queue still
 * merges the PRs in flight; a paused one, or one without access, merges nothing.
 */
function LiveMark({ queue }: { queue: Queue }) {
  const isStreamLive = useSyncExternalStore(onStreamOpenChanged, isStreamOpen);
  const isLive = isStreamLive && queue.state !== "paused" && queue.access === "ok";

  return (
    <span
      className={cn(
        "flex items-center gap-1.5 text-xs font-semibold uppercase",
        isLive ? "text-success" : "text-muted-foreground",
      )}
    >
      <span
        className={cn(
          "size-1.5 rounded-full",
          isLive ? "bg-success shadow-live motion-safe:animate-live" : "bg-muted-foreground",
        )}
      />
      {isLive ? "Live" : "Not live"}
    </span>
  );
}

function Chip({ item, text, rank }: { item: Pr; text: string | null; rank: number | undefined }) {
  return (
    <li data-flip={item.id}>
      <PrLink
        className="flex items-center gap-2 rounded-sm border border-border bg-card px-2.25 py-1.25 text-md whitespace-nowrap text-inherit hover:border-muted-foreground focus-visible:border-muted-foreground"
        number={item.number}
      >
        {rank !== undefined && (
          <span className="grid h-4 min-w-4 place-items-center rounded-xs bg-muted px-1 text-2xs font-medium text-secondary-foreground tabular-nums">
            {rank}
          </span>
        )}
        <span className={cn("size-2 flex-none rounded-full", DOTS[item.state])} />
        <b className="font-semibold">#{item.number}</b>
        {text && <span className="max-w-37.5 truncate text-secondary-foreground">{text}</span>}
      </PrLink>
    </li>
  );
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
    { key: "not-ready", label: "Waiting", items: notReady, textOf: (item: Pr) => item.state_description },
    {
      key: "queued",
      label: "Queued",
      items: queued,
      textOf: titleOf,
      foot: queue.state === "paused" ? "Paused" : "Waits for room in the window",
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

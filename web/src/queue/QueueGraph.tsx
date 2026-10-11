import { Fragment, type ReactNode, useRef } from "react";

import type { Graph, GraphRun, Pr, Queue } from "../client/types.gen";
import { itemStateLabel } from "../ui/StateBadge";
import { CheckIcon } from "../ui/icons";
import { useFlip } from "../ui/motion";
import { PrLink } from "./PrLink";
import styles from "./queue.module.css";

type RunLook = "testing" | "failing" | "passed";

const RUN_LOOKS: Record<RunLook, string> = {
  testing: styles.testing,
  failing: styles.failing,
  passed: styles.passed,
};

const DOTS: Record<string, string> = {
  not_ready: styles.dotNotReady,
  pending: styles.dotPending,
  testing: styles.dotTesting,
  pending_failure: styles.dotPendingFailure,
  tests_passed: styles.dotTestsPassed,
};

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

function Chip({ item, text, rank }: { item: Pr; text: string | null; rank: number | undefined }) {
  return (
    <li data-flip={item.id}>
      <PrLink className={styles.chip} number={item.number}>
        {rank !== undefined && <span className={styles.position}>{rank}</span>}
        <span className={`${styles.dot} ${DOTS[item.state] ?? ""}`} />
        <b>#{item.number}</b>
        {text && <span className={styles.chipText}>{text}</span>}
      </PrLink>
    </li>
  );
}

interface Lane {
  key: string;
  label: string;
  body: ReactNode;
  foot?: string;
  footClass?: string;
}

/**
 * Read from left to right: the PRs flow toward the target branch. The run of the head of the queue merges first,
 * so it is next to the branch; the items that wait are on the left. `ranks` comes from every live item of the
 * graph, `items` gives their titles.
 */
export function QueueGraph({
  detail,
  graph,
  items,
  ranks,
}: {
  detail: Queue;
  graph: Graph;
  items: Pr[];
  ranks: Map<string, number>;
}) {
  const graphRef = useRef<HTMLDivElement>(null);
  useFlip(graphRef);
  const byId = new Map(items.map((item) => [item.id, item]));
  const notReady = items.filter((item) => item.state === "not_ready");
  const queued = items.filter((item) => item.state === "pending");
  const lanes: Lane[] = [];

  if (notReady.length > 0) {
    lanes.push({
      key: "not-ready",
      label: "Waiting",
      body: (
        <ul className={styles.column}>
          {notReady.map((item) => (
            <Chip key={item.id} item={item} text={item.state_description} rank={ranks.get(item.id)} />
          ))}
        </ul>
      ),
    });
  }
  if (queued.length > 0) {
    lanes.push({
      key: "queued",
      label: "Queued",
      body: (
        <ul className={styles.column}>
          {queued.map((item) => (
            <Chip key={item.id} item={item} text={item.title ?? null} rank={ranks.get(item.id)} />
          ))}
        </ul>
      ),
      foot: detail.state === "paused" ? "Paused" : "Waits for room in the window",
    });
  }

  const runs = [...graph.runs].sort((a, b) => rankOf(a, ranks) - rankOf(b, ranks));
  const runLanes = runs.map((run, index): Lane => {
    const members = run.item_ids.flatMap((id) => byId.get(id) ?? []);
    const look = lookOf(members);

    return {
      key: run.run_id,
      label: `Run ${String(index + 1)}${run.item_ids.length > 1 ? " · batch" : ""}`,
      body: (
        <ul className={`${styles.run} ${RUN_LOOKS[look]}`}>
          {members.map((item) => (
            <Chip key={item.id} item={item} text={item.title ?? null} rank={ranks.get(item.id)} />
          ))}
        </ul>
      ),
      foot: footOf(look, run, members),
      footClass: RUN_LOOKS[look],
    };
  });
  lanes.push(...runLanes.reverse());

  return (
    <section className={styles.card} aria-labelledby="graph-title">
      <div className={styles.sectionHead}>
        <h2 id="graph-title">Live graph</h2>
        <div className={styles.right}>Each run tests its pull requests on top of the runs on its right.</div>
      </div>
      <div ref={graphRef} className={styles.graph}>
        {lanes.length === 0 && (
          <>
            <p className={styles.idle}>No pull request waits and no run is in progress.</p>
            <span className={`${styles.link} ${styles.toBranch}`} />
          </>
        )}
        {lanes.map((lane, index) => (
          <Fragment key={lane.key}>
            <div className={styles.lane} role="group" aria-label={lane.label}>
              <span className={styles.laneLabel}>{lane.label}</span>
              {lane.body}
              {lane.foot && <span className={`${styles.laneFoot} ${lane.footClass ?? ""}`}>{lane.foot}</span>}
            </div>
            <span className={`${styles.link} ${index === lanes.length - 1 ? styles.toBranch : ""}`} />
          </Fragment>
        ))}
        <div className={styles.lane} role="group" aria-label={`Branch ${detail.target_branch}`}>
          <span className={styles.laneLabel}>{detail.target_branch}</span>
          <div className={styles.commits}>
            {/* A new head comes in: the merged PR went into its branch. */}
            <span key={graph.target_head_sha} className={styles.head}>
              <CheckIcon />
            </span>
            {graph.target_head_sha && <code className={styles.sha}>{graph.target_head_sha.slice(0, 7)}</code>}
          </div>
        </div>
      </div>
    </section>
  );
}

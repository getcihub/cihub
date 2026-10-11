import type { Queue } from "../client/types.gen";
import styles from "./ui.module.css";

type Tone = "blue" | "green" | "amber" | "red" | "grey";

// The words of specs/DOMAIN.md, as the mockups v5 show them; a ring is a state that waits.
const ITEM_STATES: Record<string, [string, Tone, boolean]> = {
  not_ready: ["Waiting", "grey", true],
  pending: ["Queued", "grey", true],
  testing: ["Testing", "blue", true],
  pending_failure: ["Tests failed", "amber", false],
  tests_passed: ["Tests passed", "green", false],
  merged: ["Merged", "blue", false],
  failed: ["Failed", "red", false],
  cancelled: ["Cancelled", "grey", false],
};

const QUEUE_STATES: Record<string, [string, Tone]> = {
  running: ["Running", "green"],
  paused: ["Paused", "amber"],
  draining: ["Draining", "blue"],
  switching_modes: ["Switching modes", "blue"],
};

function Badge({ label, tone, isRing = false }: { label: string; tone: Tone; isRing?: boolean }) {
  return (
    <span className={`${styles.badge} ${styles[tone]} ${isRing ? styles.ring : ""}`}>
      <span className={styles.badgeDot} />
      {label}
    </span>
  );
}

/** The name of the state of an item, as the dashboard shows it. */
export function itemStateLabel(state: string): string {
  return ITEM_STATES[state]?.[0] ?? state;
}

export function ItemStateBadge({ state }: { state: string }) {
  const [label, tone, isRing] = ITEM_STATES[state] ?? [state, "grey", false];

  return <Badge label={label} tone={tone} isRing={isRing} />;
}

/** The state of a queue, unless something stops it: a lost access, a file that is not valid. */
export function QueueStateBadge({ queue }: { queue: Queue }) {
  if (queue.access === "suspended") {
    return <Badge label="Access lost" tone="red" />;
  }
  if (queue.config_error) {
    return <Badge label="Invalid .cihub.yaml" tone="red" />;
  }
  const [label, tone] = QUEUE_STATES[queue.state] ?? [queue.state, "grey"];

  return <Badge label={label} tone={tone} />;
}

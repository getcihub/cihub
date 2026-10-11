import type { OrgPr } from "../client/types.gen";
import { itemStateLabel } from "../ui/StateBadge";
import { CheckIcon } from "../ui/icons";
import styles from "./Mine.module.css";

function plural(count: number): string {
  return `${String(count)} pull request${count === 1 ? "" : "s"}`;
}

/** The most PRs ahead that the train shows; the others are a number. */
const SHOWN = 10;

const CARS: Record<string, string> = {
  testing: styles.testing,
  pending_failure: styles.failing,
  tests_passed: styles.passed,
};

/**
 * The line in front of a PR, toward the target branch: the PR, then the PRs that merge before it, the nearest
 * first, then the branch.
 */
export function Train({ pr, branch }: { pr: OrgPr; branch: string }) {
  const nearestFirst = [...pr.ahead].reverse();
  const shown = nearestFirst.slice(-SHOWN);
  const isMerged = pr.item.state === "merged";

  return (
    <div
      className={`${styles.train} ${pr.item.state === "not_ready" ? styles.outside : ""}`}
      role="img"
      aria-label={isMerged ? `Merged into ${branch}` : `${plural(pr.ahead.length)} before it, then ${branch}`}
    >
      {!isMerged && <span className={`${styles.car} ${styles.me} ${CARS[pr.item.state] ?? ""}`} />}
      {nearestFirst.length > shown.length && (
        <span className={styles.more}>+{nearestFirst.length - shown.length}</span>
      )}
      {shown.map((other) => (
        <span
          key={other.pr_number}
          className={`${styles.car} ${CARS[other.state] ?? ""}`}
          title={`#${String(other.pr_number)} · ${itemStateLabel(other.state)}`}
        />
      ))}
      <span className={`${styles.branch} ${isMerged ? styles.hasMe : ""}`} title={branch}>
        <CheckIcon />
      </span>
    </div>
  );
}

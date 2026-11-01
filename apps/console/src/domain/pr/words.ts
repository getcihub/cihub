import type { ItemState, Pr, Queue } from "@/client/types.gen";
import { isStopped } from "@/domain/queue/attention";

// The words of specs/DOMAIN.md, as the mockups v5 show them.
const ITEM_STATE_LABELS: Record<string, string> = {
  not_ready: "Not ready",
  pending: "Queued",
  testing: "Testing",
  pending_failure: "Tests failed",
  tests_passed: "Tests passed",
  merged: "Merged",
  failed: "Failed",
  cancelled: "Cancelled",
} satisfies Record<ItemState, string>;

/** The name of the state of an item, as the dashboard shows it; a state that the dashboard does not know, as is. */
export function itemStateLabel(state: string): string {
  return ITEM_STATE_LABELS[state] ?? state;
}

/** The states where a PR has left the queue: it merged, failed or was cancelled. */
export function isEnded(state: ItemState): boolean {
  return state === "merged" || state === "failed" || state === "cancelled";
}

const ORDINAL_SUFFIXES: Record<number, string> = { 1: "st", 2: "nd", 3: "rd" };

/** `number` as a place in line: "1st", "2nd", "11th", "21st". */
function ordinal(number: number): string {
  const tens = number % 100;
  // 11th, 12th and 13th, unlike 1st, 2nd and 3rd.
  const suffix = tens >= 11 && tens <= 13 ? "th" : (ORDINAL_SUFFIXES[number % 10] ?? "th");

  return `${String(number)}${suffix}`;
}

/** Where the PR is, in a few words: "2nd in line", "testing", "merged into main". */
export function whereOf(pr: Pr, isPaused: boolean, branch: string): string {
  switch (pr.state) {
    case "not_ready":
      return "not in line yet";
    case "pending":
      return `${ordinal(pr.ahead.length + 1)} in line${isPaused ? ", paused" : ""}`;
    case "testing":
      return "testing";
    case "pending_failure":
      return "tests failed";
    case "tests_passed":
      return "merging";
    case "merged":
      return `merged into ${branch}`;
    default:
      return pr.state;
  }
}

/** The queue of `pr` among `queues`; `undefined` while their read does not have it yet. */
export function queueOfPr(pr: Pr, queues: Queue[]): Queue | undefined {
  return queues.find((queue) => queue.repo === pr.repo && queue.target_branch === pr.queue);
}

/** The title of the tab: where the first PR of the person is, "#124 2nd in line · CIHub", from any page. */
export function tabTitleOf(firstPr: Pr | undefined, queues: Queue[]): string {
  return firstPr ? `#${String(firstPr.number)} ${whereAmong(firstPr, queues)} · CIHub` : "CIHub";
}

/** Where `pr` is, with the state and the target branch of its queue among `queues`. */
export function whereAmong(pr: Pr, queues: Queue[]): string {
  const queue = queueOfPr(pr, queues);

  return whereOf(pr, queue !== undefined && isStopped(queue), pr.queue);
}

/** The estimate of the server, as "about 12 min"; `null` without one. */
export function etaOf(pr: Pr): string | null {
  const seconds = pr.merges_in_seconds;
  if (seconds == null) {
    return null;
  }
  return seconds < 60 ? "a few seconds" : `about ${String(Math.ceil(seconds / 60))} min`;
}

/** What keeps a PR out of the queue, as a sentence where a check is a code span; `null` for unmet queue conditions, which the drawer shows one by one. */
// eslint-disable-next-line no-restricted-syntax -- a block reason is a JSON that the API gives no schema
export function blockSentence(reason: Record<string, unknown>): string | null {
  const field = (name: string) => String(reason[name]);
  switch (reason.kind) {
    case "review_required":
      return `${field("have")} of ${field("need")} required approvals`;
    case "changes_requested":
      return "A review asks for changes";
    case "failing_required_check":
      return `The required check \`${field("check")}\` failed`;
    case "awaiting_required_check":
      return reason.status === "missing"
        ? `The required check \`${field("check")}\` has not reported`
        : `The required check \`${field("check")}\` is ${field("status")}`;
    case "behind_base":
      return "The branch is behind its base";
    case "merge_conflict":
      return "The branch conflicts with its base";
    case "stack_needs_rebase":
      return `The branch must be rebased onto #${field("onto_pr_number")}`;
    case "undetermined":
      return "The SCM does not say yet why the PR is blocked";
    case "queue_conditions_unmet":
      return null;
    default:
      return field("kind");
  }
}

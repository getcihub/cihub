import type { ItemCounts } from "@/client/types.gen";

/** The PRs that are in the queue: every live state, waiting to enter included. */
export function inQueueCount(counts: ItemCounts): number {
  return counts.not_ready + counts.pending + counts.testing + counts.pending_failure + counts.tests_passed;
}

/** The PRs in a run: tested, failed and waiting for the ones ahead, or passed and waiting to merge. */
export function testingCount(counts: ItemCounts): number {
  return counts.testing + counts.pending_failure + counts.tests_passed;
}

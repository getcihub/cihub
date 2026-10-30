import { getPrOptions, getRunOptions, listEventsOptions } from "@/client/@tanstack/react-query.gen";
import type { QueuePath } from "@/domain/queue/queries";

/** The read of the PR `number` of the queue at `path`. */
export function prOptions(path: QueuePath, number: number): ReturnType<typeof getPrOptions> {
  return getPrOptions({ path: { org: path.org, repo: path.repo, number } });
}

/** The read of the first 100 events of the PR `number` of the queue at `path`. */
export function prEventsOptions(path: QueuePath, number: number): ReturnType<typeof listEventsOptions> {
  return listEventsOptions({ path: { org: path.org, repo: path.repo, number }, query: { limit: 100 } });
}

/** The read of the run `runId` of the queue at `path`: what it tests, and the checks of its test commit. */
export function runOptions(path: QueuePath, runId: string): ReturnType<typeof getRunOptions> {
  return getRunOptions({ path: { org: path.org, repo: path.repo, run_id: runId } });
}

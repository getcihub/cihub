import { getPrOptions, listEventsOptions } from "@/client/@tanstack/react-query.gen";
import type { QueuePath } from "@/domain/queue/queries";

/** The reads of the drawer of the PR `number`: the PR and its first 100 events. */
export function prReads(
  path: QueuePath,
  number: number,
): readonly [ReturnType<typeof getPrOptions>, ReturnType<typeof listEventsOptions>] {
  const pr = { org: path.org, repo: path.repo, number };
  return [getPrOptions({ path: pr }), listEventsOptions({ path: pr, query: { limit: 100 } })] as const;
}

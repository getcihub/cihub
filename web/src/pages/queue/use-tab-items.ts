import { useSuspenseQuery } from "@tanstack/react-query";

import type { PrList } from "@/client/types.gen";
import { type QueuePath, type Tab, queueReads } from "@/domain/queue/queries";

/**
 * The items of the tab `tab` at `now`: the same read as the live items for the tab of the live ones. Its loader
 * starts it without waiting: the items of a tab load under its tabs, and a change of tab never hides the page.
 */
export function useTabItems(path: QueuePath, tab: Tab, now: number): PrList {
  return useSuspenseQuery(queueReads(path, tab, now).tab).data;
}

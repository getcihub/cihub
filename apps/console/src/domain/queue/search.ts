import { z } from "zod";

/** The tabs of the items of a queue. */
export const TABS = ["live", "merged", "ended"] as const;
export type Tab = (typeof TABS)[number];

/** The panes of the page of a queue: the queue now, and what it did over a period. */
export const PANES = ["graph", "health"] as const;
export type Pane = (typeof PANES)[number];

/** The periods of the Health of a queue. */
export const RANGES = ["24h", "7d", "30d"] as const;
export type Range = (typeof RANGES)[number];

/** The tab of the live items, the graph and the last 24 hours are the page itself: its address carries none of them. */
export const QUEUE_SEARCH_DEFAULTS = { tab: "live", pane: "graph", range: "24h" } as const;

/** The search of the page of a queue; a value that is not valid opens the page as it is by default. */
export const queueSearch = z.object({
  tab: z.enum(TABS).default(QUEUE_SEARCH_DEFAULTS.tab).catch(QUEUE_SEARCH_DEFAULTS.tab),
  pane: z.enum(PANES).default(QUEUE_SEARCH_DEFAULTS.pane).catch(QUEUE_SEARCH_DEFAULTS.pane),
  range: z.enum(RANGES).default(QUEUE_SEARCH_DEFAULTS.range).catch(QUEUE_SEARCH_DEFAULTS.range),
  // A period that the person chose, two RFC 3339 times; the server checks it.
  from: z.string().optional().catch(undefined),
  to: z.string().optional().catch(undefined),
});

export type QueueSearch = z.infer<typeof queueSearch>;

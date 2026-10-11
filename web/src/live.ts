import { type QueryClient, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { z } from "zod";

import { listOrgQueuesQueryKey } from "./client/@tanstack/react-query.gen";
import { client } from "./client/client.gen";
import type { OrgQueueList } from "./client/types.gen";

/** The event `queue_changed` of the stream of an organization. */
const queueChanged = z.object({ queue_id: z.string(), seq: z.number() });

/** The changes that come together give one read: a busy queue does not use up the requests of the minute. */
const GATHER_MS = 500;

/** The waits before a stream that the browser closed opens again: an error of the server, a deploy. */
const REOPEN_AFTER_MS = [1_000, 2_000, 5_000, 10_000, 30_000];

interface KeyOf {
  _id?: string;
  path?: { org?: string; repo?: string; queue?: string };
}

const keyOf = (queryKey: readonly unknown[]) => (queryKey[0] ?? {}) as KeyOf;

const isOf = (key: KeyOf, org: string) => key.path?.org?.toLowerCase() === org.toLowerCase();

// The pages that wait for a change of a queue: a request shows as sent until then.
const listeners = new Set<(queueId: string) => void>();

/** Calls `listener` at each change of a queue that the stream signals, until the returned function. */
export function onQueueChanged(listener: (queueId: string) => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Reads again every read of `org`: when a stream opens, as the page may have missed changes. */
function readOrgAgain(queryClient: QueryClient, org: string): Promise<void> {
  return queryClient.invalidateQueries({ predicate: (query) => isOf(keyOf(query.queryKey), org) });
}

/**
 * Reads again what the changes of the queues `queueIds` of `org` show: their reads, the run of a drawer, the
 * queues and the PRs of the person. A queue that the page does not know yet is new: the list of the queues gives
 * it. A read in flight goes on.
 */
function readQueuesAgain(queryClient: QueryClient, org: string, queueIds: Set<string>): Promise<void> {
  const queues = queryClient.getQueryData<OrgQueueList>(listOrgQueuesQueryKey({ path: { org } }));
  const changed = queues?.queues.filter((entry) => queueIds.has(entry.queue.id)) ?? [];

  return queryClient.invalidateQueries(
    {
      predicate: (query) => {
        const key = keyOf(query.queryKey);
        if (!isOf(key, org)) {
          return false;
        }
        if (key._id === "listOrgQueues" || key._id === "listOrgPrs" || key._id === "testRun") {
          return true;
        }
        return changed.some((entry) => key.path?.repo === entry.repo && key.path.queue === entry.queue.name);
      },
    },
    { cancelRefetch: false },
  );
}

/**
 * Follows the changes of the queues of `org` while one of its pages is open, and opens the stream again when the
 * browser closes it.
 */
export function useQueueNews(org: string): void {
  const queryClient = useQueryClient();

  useEffect(() => {
    if (typeof EventSource === "undefined") {
      return;
    }

    let source: EventSource | null = null;
    let reopening: ReturnType<typeof setTimeout> | undefined;
    let gathering: ReturnType<typeof setTimeout> | undefined;
    let failures = 0;
    const gathered = new Set<string>();

    const open = () => {
      source = new EventSource(`${client.getConfig().baseUrl ?? ""}/orgs/${encodeURIComponent(org)}/stream`);
      source.onopen = () => {
        failures = 0;
        void readOrgAgain(queryClient, org);
      };
      source.onerror = () => {
        // The browser opens again a stream that ended; not one that the server refused.
        if (source?.readyState === EventSource.CLOSED) {
          source.close();
          reopening = setTimeout(open, REOPEN_AFTER_MS[Math.min(failures, REOPEN_AFTER_MS.length - 1)]);
          failures += 1;
        }
      };
      source.addEventListener("queue_changed", (event) => {
        const change = queueChanged.safeParse(JSON.parse(String(event.data)));
        if (!change.success) {
          return;
        }
        gathered.add(change.data.queue_id);
        gathering ??= setTimeout(() => {
          gathering = undefined;
          const queueIds = new Set(gathered);
          gathered.clear();
          void readQueuesAgain(queryClient, org, queueIds);
          for (const queueId of queueIds) {
            listeners.forEach((listener) => {
              listener(queueId);
            });
          }
        }, GATHER_MS);
      });
    };
    open();

    return () => {
      clearTimeout(reopening);
      clearTimeout(gathering);
      source?.close();
    };
  }, [org, queryClient]);
}

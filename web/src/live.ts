import { type QueryClient, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { z } from "zod";

import { listOrgQueuesQueryKey } from "./client/@tanstack/react-query.gen";
import { client } from "./client/client.gen";
import type { OrgQueueList } from "./client/types.gen";

/** The event `queue_changed` of the stream of an organization. */
const queueChanged = z.object({ queue_id: z.string(), seq: z.number() });

interface KeyOf {
  _id?: string;
  path?: { org?: string; repo?: string; queue?: string };
}

const keyOf = (queryKey: readonly unknown[]) => (queryKey[0] ?? {}) as KeyOf;

const isOf = (key: KeyOf, org: string) => key.path?.org?.toLowerCase() === org.toLowerCase();

/** Reads again every read of `org`: after a stream that lost changes. */
function readOrgAgain(queryClient: QueryClient, org: string): Promise<void> {
  return queryClient.invalidateQueries({ predicate: (query) => isOf(keyOf(query.queryKey), org) });
}

/**
 * Reads again what the change of the queue `queueId` of `org` shows: its reads, the run of a drawer, the queues
 * and the PRs of the person. A queue that the page does not know yet is new: the list of the queues gives it.
 */
function readQueueAgain(queryClient: QueryClient, org: string, queueId: string): Promise<void> {
  const queues = queryClient.getQueryData<OrgQueueList>(listOrgQueuesQueryKey({ path: { org } }));
  const changed = queues?.queues.find((entry) => entry.queue.id === queueId);

  return queryClient.invalidateQueries({
    predicate: (query) => {
      const key = keyOf(query.queryKey);
      if (!isOf(key, org)) {
        return false;
      }
      if (key._id === "listOrgQueues" || key._id === "listOrgPrs" || key._id === "testRun") {
        return true;
      }
      return (
        changed !== undefined && key.path?.repo === changed.repo && key.path.queue === changed.queue.name
      );
    },
  });
}

/**
 * Follows the changes of the queues of `org` while one of its pages is open. The browser opens the stream again
 * after a loss; the page then reads everything again, as it may have missed changes.
 */
export function useQueueNews(org: string): void {
  const queryClient = useQueryClient();

  useEffect(() => {
    if (typeof EventSource === "undefined") {
      return;
    }

    const source = new EventSource(
      `${client.getConfig().baseUrl ?? ""}/orgs/${encodeURIComponent(org)}/stream`,
    );
    let hasOpened = false;

    source.onopen = () => {
      if (hasOpened) {
        void readOrgAgain(queryClient, org);
      }
      hasOpened = true;
    };
    source.addEventListener("queue_changed", (event) => {
      const change = queueChanged.safeParse(JSON.parse(String(event.data)));
      if (change.success) {
        void readQueueAgain(queryClient, org, change.data.queue_id);
      }
    });

    return () => {
      source.close();
    };
  }, [org, queryClient]);
}

import type { QueryClient } from "@tanstack/react-query";
import { z } from "zod";

import { client } from "@/client/client.gen";
import type { QueueList } from "@/client/types.gen";
import { type ReadKey, orgQueuesOptions, readKeyOf } from "./queries";

/** The event `queue_changed` of the stream of an organization. */
const queueChanged = z.object({ queue_id: z.string(), seq: z.number() });

/** The changes that come together give one read: a busy queue does not use up the requests of the minute. */
const GATHER_MS = 500;

/** The waits before a stream that the browser closed opens again: an error of the server, a deploy. */
const REOPEN_AFTER_MS = [1_000, 2_000, 5_000, 10_000, 30_000];

function isReadOfOrg(key: ReadKey, org: string): boolean {
  return key.path?.org?.toLowerCase() === org.toLowerCase();
}

// The pages that wait for a change of a queue: a request shows as sent until then.
const listeners = new Set<(queueId: string) => void>();

// The pages that show whether the stream is open: a closed stream shows the queue as it was.
const openListeners = new Set<() => void>();
let isOpen = false;

function setIsOpen(value: boolean): void {
  isOpen = value;
  openListeners.forEach((listener) => {
    listener();
  });
}

/** Whether the stream of the organization is open, so that the page follows the changes as they come. */
export function isStreamOpen(): boolean {
  return isOpen;
}

/** Calls `listener` when the stream opens or closes, until the returned function. */
export function onStreamOpenChanged(listener: () => void): () => void {
  openListeners.add(listener);
  return () => openListeners.delete(listener);
}

/** Calls `listener` at each change of a queue that the stream signals, until the returned function. */
export function onQueueChanged(listener: (queueId: string) => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Reads again every read of `org`: when a stream opens, as the page may have missed changes. */
function readOrgAgain(queryClient: QueryClient, org: string): Promise<void> {
  return queryClient.invalidateQueries({ predicate: (query) => isReadOfOrg(readKeyOf(query.queryKey), org) });
}

/**
 * Reads again what the changes of the queues `queueIds` of `org` show: their reads, the run of a drawer, the
 * queues and the PRs of the person. A queue that the page does not know yet is new: the list of the queues gives
 * it. A read in flight goes on.
 */
function readQueuesAgain(queryClient: QueryClient, org: string, queueIds: Set<string>): Promise<void> {
  const queues = queryClient.getQueryData<QueueList>(orgQueuesOptions(org).queryKey);
  const changed = queues?.queues.filter((entry) => queueIds.has(entry.id)) ?? [];

  return queryClient.invalidateQueries(
    {
      predicate: (query) => {
        const key = readKeyOf(query.queryKey);
        if (!isReadOfOrg(key, org)) {
          return false;
        }
        if (key._id === "listOrgQueues" || key._id === "listPrs" || key._id === "getRun") {
          return true;
        }
        // A read of a queue names it; a read of a PR names its repository.
        return changed.some(
          (entry) =>
            key.path?.repo === entry.repo && (key.path.queue === undefined || key.path.queue === entry.name),
        );
      },
    },
    { cancelRefetch: false },
  );
}

/**
 * Follows the changes of the queues of `org`, and opens the stream again when the browser closes it, until the
 * returned function.
 */
export function followQueueNews(queryClient: QueryClient, org: string): () => void {
  if (typeof EventSource === "undefined") {
    return () => undefined;
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
      setIsOpen(true);
      void readOrgAgain(queryClient, org);
    };
    source.onerror = () => {
      setIsOpen(false);
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
    setIsOpen(false);
  };
}

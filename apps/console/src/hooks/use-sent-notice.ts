import { useEffect, useState } from "react";

import { onQueueChanged } from "@/domain/queue/live";

export interface SentNotice {
  /** What the page sent on the queue, until the stream signals a change of it; `null` else. */
  notice: string | null;
  showNotice: (notice: string) => void;
}

/**
 * The notice of a request that the page sent on the queue `queueId`. A request is sent, not done: the queue decides
 * at its next reconciliation, and the notice stays until the stream signals a change of the queue.
 */
export function useSentNotice(queueId: string): SentNotice {
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(
    () =>
      onQueueChanged((changedQueueId) => {
        if (changedQueueId === queueId) {
          setNotice(null);
        }
      }),
    [queueId],
  );

  return { notice, showNotice: setNotice };
}

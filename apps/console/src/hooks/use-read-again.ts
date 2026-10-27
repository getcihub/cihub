import { useEffect, useRef, useState } from "react";

import { READ_AGAIN_FOR_MS, readAgainWhile } from "@/domain/setup/read-again";

export interface ReadAgain<T> {
  /** The wait started: when an answer began to wait, or at a `restart`. */
  hasStarted: boolean;
  /** The wait lasted past `READ_AGAIN_FOR_MS`: the read stops, and the page says so if it still waits. */
  isOver: boolean;
  /** The `refetchInterval` of the read that the page waits for. */
  refetchInterval: (query: { state: { data: T | undefined } }) => number | false;
  /** Waits again, after a request of the person. */
  restart: () => void;
}

/**
 * The wait of a page for a read of the server, which reads again while `isWaiting` says so of its last answer.
 * When `isStartedByData`, it starts each time an answer begins to wait after one that did not, so that a setup
 * that the server reads again long after the page opened gets its whole wait; else it starts at the first
 * `restart` only.
 */
export function useReadAgain<T>(
  isWaiting: (data: T | undefined) => boolean,
  isStartedByData = true,
): ReadAgain<T> {
  const [since, setSince] = useState<number | null>(null);
  const [isOver, setIsOver] = useState(false);
  const wasWaiting = useRef(false);

  useEffect(() => {
    if (since === null) {
      return;
    }

    const timer = setTimeout(
      () => {
        setIsOver(true);
      },
      since + READ_AGAIN_FOR_MS - Date.now(),
    );
    return () => {
      clearTimeout(timer);
    };
  }, [since]);

  const restart = () => {
    setIsOver(false);
    setSince(Date.now());
  };

  return {
    hasStarted: since !== null,
    isOver,
    // The query evaluates it after each answer: the one place that sees the data begin to wait.
    refetchInterval: (query) => {
      const waiting = isWaiting(query.state.data);
      if (isStartedByData && waiting && !wasWaiting.current) {
        restart();
      }
      wasWaiting.current = waiting;
      return since === null ? false : readAgainWhile(isWaiting, since)(query);
    },
    restart,
  };
}

import { useEffect, useState } from "react";

import { READ_AGAIN_FOR_MS, readAgainWhile } from "@/domain/setup/read-again";

export interface ReadAgain<T> {
  /** The wait started: at the opening of the page, or at a `restart`. */
  hasStarted: boolean;
  /** The wait lasted past `READ_AGAIN_FOR_MS`: the read stops, and the page says so if it still waits. */
  isOver: boolean;
  /** The `refetchInterval` of the read that the page waits for. */
  refetchInterval: false | ReturnType<typeof readAgainWhile<T>>;
  /** Waits again, after a request of the person. */
  restart: () => void;
}

/**
 * The wait of a page for a read of the server, which reads again while `isWaiting` says so of its last answer.
 * It starts at the opening of the page when `isStartedAtOpen`, else at the first `restart`.
 */
export function useReadAgain<T>(
  isWaiting: (data: T | undefined) => boolean,
  isStartedAtOpen = true,
): ReadAgain<T> {
  const [since, setSince] = useState(() => (isStartedAtOpen ? Date.now() : null));
  const [isOver, setIsOver] = useState(false);

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

  return {
    hasStarted: since !== null,
    isOver,
    refetchInterval: since === null ? false : readAgainWhile(isWaiting, since),
    restart: () => {
      setIsOver(false);
      setSince(Date.now());
    },
  };
}

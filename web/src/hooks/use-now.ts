import { startTransition, useEffect, useState } from "react";

/** How often the times of a page ("6 minutes ago") move. */
const TIME_MOVES_EVERY = 15_000;

/**
 * The time of the page, in milliseconds, which moves every few seconds. In a transition: a read whose key follows
 * the time keeps its page while it loads.
 */
export function useNow(): number {
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    const timer = setInterval(() => {
      startTransition(() => {
        setNow(Date.now());
      });
    }, TIME_MOVES_EVERY);
    return () => {
      clearInterval(timer);
    };
  }, []);

  return now;
}

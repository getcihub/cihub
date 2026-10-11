import { startTransition, useEffect, useState } from "react";

import { READ_AGAIN_EVERY } from "./live";

const UNITS: [Intl.RelativeTimeFormatUnit, number][] = [
  ["day", 86_400],
  ["hour", 3_600],
  ["minute", 60],
];
const relative = new Intl.RelativeTimeFormat("en", { numeric: "auto" });

/** `at`, an RFC 3339 time, as "6 minutes ago" from `now`, in milliseconds. */
export function timeAgo(at: string, now: number): string {
  const seconds = Math.round((now - Date.parse(at)) / 1000);

  for (const [unit, size] of UNITS) {
    if (seconds >= size) {
      return relative.format(-Math.floor(seconds / size), unit);
    }
  }
  return "just now";
}

/**
 * The time of the page, in milliseconds, which moves as often as the page reads its queue again. In a transition:
 * a read whose key follows the time keeps its page while it loads.
 */
export function useNow(): number {
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    const timer = setInterval(() => {
      startTransition(() => {
        setNow(Date.now());
      });
    }, READ_AGAIN_EVERY);
    return () => {
      clearInterval(timer);
    };
  }, []);

  return now;
}

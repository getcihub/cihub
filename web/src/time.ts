import { startTransition, useEffect, useState } from "react";

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

/** The time from `start` to `end`, two RFC 3339 times, as "22 min" or "3 h 5 min". */
export function durationBetween(start: string, end: string): string {
  const minutes = Math.max(1, Math.round((Date.parse(end) - Date.parse(start)) / 60_000));
  const hours = Math.floor(minutes / 60);

  return hours === 0 ? `${String(minutes)} min` : `${String(hours)} h ${String(minutes % 60)} min`;
}

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

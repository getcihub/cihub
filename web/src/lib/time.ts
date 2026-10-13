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

/** `at`, an RFC 3339 time, as a date and a time: "Oct 11, 2026, 9:30 PM". */
export function dateAndTime(at: string): string {
  return new Date(at).toLocaleString("en", { dateStyle: "medium", timeStyle: "short" });
}

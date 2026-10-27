/** Hours, days and months in UTC, as milliseconds since the epoch. */

export const HOUR_MS = 3_600_000;
export const DAY_MS = 86_400_000;

/** The start of the UTC day of `at`. */
export function startOfDay(at: number): number {
  return Math.floor(at / DAY_MS) * DAY_MS;
}

/** The first day of the UTC month of `at`. */
export function startOfMonth(at: number): number {
  const date = new Date(at);
  return Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), 1);
}

/** The first day of the month `count` months after the month of `at`. */
export function addMonths(at: number, count: number): number {
  const date = new Date(at);
  return Date.UTC(date.getUTCFullYear(), date.getUTCMonth() + count, 1);
}

/** The Monday of the UTC week of `at`. */
export function mondayOf(at: number): number {
  return startOfDay(at) - ((new Date(at).getUTCDay() + 6) % 7) * DAY_MS;
}

/** `at` in English, in UTC: "Oct 9", "October 2026". */
export function utcText(at: number, options: Intl.DateTimeFormatOptions): string {
  return new Date(at).toLocaleDateString("en-US", { ...options, timeZone: "UTC" });
}

/** An hour of the day: "09:00"; 24 is the end of a day. */
export function hourText(hour: number): string {
  return `${String(hour).padStart(2, "0")}:00`;
}

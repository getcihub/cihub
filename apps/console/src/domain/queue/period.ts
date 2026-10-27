import {
  DAY_MS,
  HOUR_MS,
  addMonths,
  hourText,
  mondayOf,
  startOfDay,
  startOfMonth,
  utcText,
} from "@cihub/ui/utc-days";
import type { Range } from "./queries";

/** The history that a period of the Health can start in, as the server checks it: 13 months. */
export const KEPT_DAYS = 395;

/** A period of the Health: one of the ranges, or the UTC hours from `from` to `to`, two RFC 3339 times. */
export type HealthPeriod = { range: Range } | { from: string; to: string };

export const RANGE_LABELS: Record<Range, string> = { "24h": "24 hours", "7d": "7 days", "30d": "30 days" };

/** The period of the search of the page: `from` and `to` together, else the range. */
export function periodOfSearch({
  range,
  from,
  to,
}: {
  range: Range;
  from?: string;
  to?: string;
}): HealthPeriod {
  return from && to ? { from, to } : { range };
}

export type Grain = "hour" | "day" | "week";

/** The size of the buckets of a period of `span` milliseconds, as the server cuts them. */
export function grainOf(span: number): Grain {
  if (span <= 2 * DAY_MS) {
    return "hour";
  }
  return span <= 90 * DAY_MS ? "day" : "week";
}

/** The first day that a period can start on at `now`. */
export function oldestDay(now: number): number {
  return startOfDay(now - KEPT_DAYS * DAY_MS) + DAY_MS;
}

/** "Sep 12 – Oct 3", "Oct 8, 09:00 – 18:00", "Nov 1, 2025 – Oct 9, 2026": a period, its end excluded. */
export function rangeText(from: number, to: number, now: number): string {
  const last = to - 1;
  const year = new Date(now).getUTCFullYear();
  const isThisYear = new Date(from).getUTCFullYear() === year && new Date(last).getUTCFullYear() === year;
  const date = (at: number) =>
    utcText(
      at,
      isThisYear ? { month: "short", day: "numeric" } : { month: "short", day: "numeric", year: "numeric" },
    );
  const isSameDay = startOfDay(from) === startOfDay(last);

  if (from % DAY_MS === 0 && to % DAY_MS === 0) {
    return isSameDay ? date(from) : `${date(from)} – ${date(last)}`;
  }
  const start = hourText(new Date(from).getUTCHours());
  const end = to % DAY_MS === 0 ? "24:00" : hourText(new Date(to).getUTCHours());
  return isSameDay ? `${date(from)}, ${start} – ${end}` : `${date(from)}, ${start} – ${date(last)}, ${end}`;
}

/** The length of a period: "4 hours", "22 days", "11 months". */
export function spanText(span: number): string {
  const hours = Math.round(span / HOUR_MS);
  if (hours < 48) {
    return hours === 1 ? "1 hour" : `${String(hours)} hours`;
  }
  const days = Math.round(span / DAY_MS);
  return days < 120 ? `${String(days)} days` : `${String(Math.round(days / 30.4))} months`;
}

/** Whether the period before the one from `from` to `to` starts in the history that CIHub keeps. */
export function hasPeriodBefore(from: number, to: number, now: number): boolean {
  return from - (to - from) >= now - KEPT_DAYS * DAY_MS;
}

export interface PeriodWords {
  /** "in the last 7 days", "over Sep 12 – Oct 3". */
  during: string;
  /** "in the previous 7 days", "in the 22 days before". */
  previous: string;
  /** "last 7 days", "Sep 12 – Oct 3". */
  over: string;
}

/**
 * The words of a period in the sentences of the Health; `served` is the period that the server read, which ends at
 * the end of the current hour at the latest.
 */
export function periodWords(
  period: HealthPeriod,
  served: { from: string; to: string },
  now: number,
): PeriodWords {
  if ("range" in period) {
    const label = RANGE_LABELS[period.range];
    return { during: `in the last ${label}`, previous: `in the previous ${label}`, over: `last ${label}` };
  }
  const text = rangeText(Date.parse(period.from), Date.parse(period.to), now);
  const span = Date.parse(served.to) - Date.parse(served.from);
  return { during: `over ${text}`, previous: `in the ${spanText(span)} before`, over: text };
}

export interface Preset {
  id: string;
  label: string;
  /** The first and the last UTC day of the period. */
  first: number;
  last: number;
}

/** The shortcuts of the calendar at `now`. */
export function presetsOf(now: number): Preset[] {
  const today = startOfDay(now);
  const monday = mondayOf(now);
  const thisMonth = startOfMonth(now);
  return [
    { id: "today", label: "Today", first: today, last: today },
    { id: "yesterday", label: "Yesterday", first: today - DAY_MS, last: today - DAY_MS },
    { id: "this-week", label: "This week", first: monday, last: today },
    { id: "last-week", label: "Last week", first: monday - 7 * DAY_MS, last: monday - DAY_MS },
    { id: "this-month", label: "This month", first: thisMonth, last: today },
    { id: "last-month", label: "Last month", first: addMonths(thisMonth, -1), last: thisMonth - DAY_MS },
    { id: "90d", label: "Last 90 days", first: today - 89 * DAY_MS, last: today },
    { id: "6m", label: "Last 6 months", first: addMonths(thisMonth, -5), last: today },
    { id: "12m", label: "Last 12 months", first: addMonths(thisMonth, -11), last: today },
  ];
}

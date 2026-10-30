import { type HealthPeriod } from "@/domain/queue/period";
import { DAY_MS, HOUR_MS, addMonths, startOfDay, startOfMonth, utcText } from "@cihub/utils/utc-days";

/** The period that the person builds in the calendar, before Apply. */
export interface Draft {
  first: number | null;
  last: number | null;
  hasHours: boolean;
  fromHour: number;
  toHour: number;
  /** The first of the two months shown. */
  month: number;
  /** The dates as typed, until they read as days. */
  typed: { from: string; to: string } | null;
  invalid: "from" | "to" | null;
}

export const PRESET_DAYS = { "24h": 1, "7d": 6, "30d": 29 };

export function draftOf(period: HealthPeriod, now: number): Draft {
  const today = startOfDay(now);
  if ("range" in period) {
    return {
      first: today - PRESET_DAYS[period.range] * DAY_MS,
      last: today,
      hasHours: false,
      fromHour: 0,
      toHour: 24,
      month: addMonths(today, -1),
      typed: null,
      invalid: null,
    };
  }
  const [from, to] = [Date.parse(period.from), Date.parse(period.to)];
  const hasHours = from % DAY_MS !== 0 || to % DAY_MS !== 0;
  const last = startOfDay(to - 1);
  return {
    first: startOfDay(from),
    last,
    hasHours,
    fromHour: new Date(from).getUTCHours(),
    toHour: to % DAY_MS === 0 ? 24 : new Date(to).getUTCHours(),
    month: addMonths(startOfMonth(last), -1),
    typed: null,
    invalid: null,
  };
}

/** The period of a draft, its end excluded, once its two days are chosen. */
export function periodOf(draft: Draft): { from: number; to: number } | null {
  if (draft.first === null || draft.last === null) {
    return null;
  }
  const from = draft.first + (draft.hasHours ? draft.fromHour : 0) * HOUR_MS;
  const to = draft.last + (draft.hasHours ? draft.toHour : 24) * HOUR_MS;
  return to > from ? { from, to } : null;
}

export const dayText = (day: number | null) =>
  day === null ? "" : utcText(day, { month: "short", day: "numeric", year: "numeric" });

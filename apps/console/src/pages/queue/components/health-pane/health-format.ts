import type { HealthTotals } from "@/client/types.gen";

/** `seconds` in whole minutes, as the charts count them. */
export function minutesOf(seconds: number): number {
  return Math.round(seconds / 60);
}

/** A time to merge or in a state: "40 s", "22 min", "2.5 h". */
export function durationOf(seconds: number): string {
  if (seconds < 60) {
    return `${String(seconds)} s`;
  }
  const minutes = minutesOf(seconds);
  return minutes < 90 ? `${String(minutes)} min` : `${(minutes / 60).toFixed(1)} h`;
}

/** `share`, from 0 to 1, as "12%". */
export function percentOf(share: number): string {
  return `${String(Math.round(share * 100))}%`;
}

/** The share of the PRs that merged or failed that failed. */
export function failureRateOf({ merged, failed }: HealthTotals): number {
  return merged + failed === 0 ? 0 : failed / (merged + failed);
}

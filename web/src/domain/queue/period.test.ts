import { describe, expect, it } from "vitest";

import { DAY_MS, HOUR_MS } from "@/lib/utc-days";
import { grainOf, hasPeriodBefore, periodOfSearch, presetsOf, rangeText, spanText } from "./period";

// A Thursday.
const NOW = Date.parse("2026-10-15T09:41:00Z");
const at = (time: string) => Date.parse(time);

describe("a period of the Health", () => {
  it("has bars of an hour up to 2 days, of a day up to 90 days, and of a week beyond", () => {
    expect(grainOf(2 * DAY_MS)).toBe("hour");
    expect(grainOf(2 * DAY_MS + HOUR_MS)).toBe("day");
    expect(grainOf(90 * DAY_MS)).toBe("day");
    expect(grainOf(91 * DAY_MS)).toBe("week");
  });

  it("reads as its days, its hours, and its years when they are not this one", () => {
    expect(rangeText(at("2026-09-12T00:00:00Z"), at("2026-10-04T00:00:00Z"), NOW)).toBe("Sep 12 – Oct 3");
    expect(rangeText(at("2026-10-08T00:00:00Z"), at("2026-10-09T00:00:00Z"), NOW)).toBe("Oct 8");
    expect(rangeText(at("2026-10-08T09:00:00Z"), at("2026-10-08T18:00:00Z"), NOW)).toBe(
      "Oct 8, 09:00 – 18:00",
    );
    expect(rangeText(at("2026-10-07T09:00:00Z"), at("2026-10-09T00:00:00Z"), NOW)).toBe(
      "Oct 7, 09:00 – Oct 8, 24:00",
    );
    expect(rangeText(at("2025-11-01T00:00:00Z"), at("2026-10-16T00:00:00Z"), NOW)).toBe(
      "Nov 1, 2025 – Oct 15, 2026",
    );
  });

  it("says its length", () => {
    expect(spanText(HOUR_MS)).toBe("1 hour");
    expect(spanText(9 * HOUR_MS)).toBe("9 hours");
    expect(spanText(22 * DAY_MS)).toBe("22 days");
    expect(spanText(335 * DAY_MS)).toBe("11 months");
  });

  it("has a period before only in the 13 months that CIHub keeps", () => {
    expect(hasPeriodBefore(at("2026-09-12T00:00:00Z"), at("2026-10-04T00:00:00Z"), NOW)).toBe(true);
    expect(hasPeriodBefore(at("2025-11-01T00:00:00Z"), at("2026-10-16T00:00:00Z"), NOW)).toBe(false);
  });

  it("offers shortcuts from today", () => {
    const presets = new Map(presetsOf(NOW).map((preset) => [preset.id, [preset.first, preset.last]]));

    expect(presets.get("this-week")).toEqual([at("2026-10-12T00:00:00Z"), at("2026-10-15T00:00:00Z")]);
    expect(presets.get("last-week")).toEqual([at("2026-10-05T00:00:00Z"), at("2026-10-11T00:00:00Z")]);
    expect(presets.get("last-month")).toEqual([at("2026-09-01T00:00:00Z"), at("2026-09-30T00:00:00Z")]);
    expect(presets.get("12m")).toEqual([at("2025-11-01T00:00:00Z"), at("2026-10-15T00:00:00Z")]);
  });

  it("takes from and to together, else the range", () => {
    expect(periodOfSearch({ range: "7d", from: "2026-10-01T00:00:00Z", to: "2026-10-02T00:00:00Z" })).toEqual(
      {
        from: "2026-10-01T00:00:00Z",
        to: "2026-10-02T00:00:00Z",
      },
    );
    expect(periodOfSearch({ range: "7d", from: "2026-10-01T00:00:00Z" })).toEqual({ range: "7d" });
  });
});

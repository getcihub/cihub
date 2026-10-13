import { describe, expect, it } from "vitest";

import { durationBetween, timeAgo } from "./time";

describe("timeAgo", () => {
  const now = Date.parse("2026-10-11T12:00:00Z");

  it("says how long ago a time was, in its largest unit", () => {
    expect(timeAgo("2026-10-11T11:59:30Z", now)).toBe("just now");
    expect(timeAgo("2026-10-11T11:54:00Z", now)).toBe("6 minutes ago");
    expect(timeAgo("2026-10-11T09:00:00Z", now)).toBe("3 hours ago");
    expect(timeAgo("2026-10-08T12:00:00Z", now)).toBe("3 days ago");
  });
});

describe("durationBetween", () => {
  it("says a duration in minutes, then in hours and minutes", () => {
    expect(durationBetween("2026-10-11T12:00:00Z", "2026-10-11T12:22:10Z")).toBe("22 min");
    expect(durationBetween("2026-10-11T12:00:00Z", "2026-10-11T12:00:20Z")).toBe("1 min");
    expect(durationBetween("2026-10-11T09:00:00Z", "2026-10-11T12:05:00Z")).toBe("3 h 5 min");
  });
});

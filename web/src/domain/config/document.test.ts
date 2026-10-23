import { describe, expect, it } from "vitest";

import { settingsOf } from "@/test/config";
import { withOwnSettings } from "./document";

describe("withOwnSettings", () => {
  it("shows what the part sets, from the part, before the preview of the server", () => {
    const resolved = settingsOf({ batching_size: [6, "defaults"] });

    const shown = withOwnSettings(
      resolved,
      { batching_size: 12, merge_method: null, testing_concurrency: 3 },
      "queue",
    );

    expect(shown.batching_size).toBe(12);
    expect(shown.sources.batching_size).toBe("queue");
    expect([shown.window_floor, shown.window_ceiling]).toEqual([3, 3]);
    expect(shown.sources.window_ceiling).toBe("queue");
    expect(shown.merge_method).toBe(resolved.merge_method);
    expect(shown.sources.merge_method).toBe("default");
    expect(resolved.batching_size).toBe(6);
  });

  it("gives the required checks of the part as a list, or as those of the branch rules", () => {
    const resolved = settingsOf({ required_statuses: [["build"], "defaults"] });

    const listed = withOwnSettings(
      resolved,
      { required_statuses: { kind: "checks", names: ["lint"] } },
      "queue",
    );
    const ruled = withOwnSettings(resolved, { required_statuses: { kind: "branch_rules" } }, "queue");

    expect(listed.required_statuses).toEqual(["lint"]);
    expect(ruled.required_statuses).toBeNull();
    expect(ruled.sources.required_statuses).toBe("queue");
  });
});

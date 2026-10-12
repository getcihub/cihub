import { describe, expect, it } from "vitest";

import { shouldRetryRead, startReads } from "./queryClient";
import { problem } from "@/test/app";

describe("the reads of the dashboard", () => {
  it("try again once after a failure of the server or of the network", () => {
    expect(shouldRetryRead(0, problem(500, "internal"))).toBe(true);
    expect(shouldRetryRead(0, new TypeError("Failed to fetch"))).toBe(true);
    expect(shouldRetryRead(1, problem(500, "internal"))).toBe(false);
  });

  it("never try again a read that the API refused", () => {
    expect(shouldRetryRead(0, problem(404, "queue_not_found"))).toBe(false);
    expect(shouldRetryRead(0, problem(401, "unauthorized"))).toBe(false);
  });

  it("start in a loader without stopping on a failed read", async () => {
    await expect(
      startReads(Promise.resolve(1), Promise.reject(new Error("refused"))),
    ).resolves.toBeUndefined();
  });
});

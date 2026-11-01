import { describe, expect, it } from "vitest";

import { isRefusal } from "./problem";

describe("a refusal of the API", () => {
  it("is a problem that answers 4xx, and nothing else", () => {
    expect(isRefusal({ code: "not_found", status: 404 })).toBe(true);
    expect(isRefusal({ code: "internal", status: 500 })).toBe(false);
    expect(isRefusal(new Error("The network failed."))).toBe(false);
  });
});

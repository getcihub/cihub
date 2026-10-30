import { describe, expect, it } from "vitest";

import { pageOfThisSite } from "./search";

describe("pageOfThisSite", () => {
  it("accepts a path of this site", () => {
    for (const path of ["/", "/orgs/acme", "/q/web/main/121?tab=merged"]) {
      expect(pageOfThisSite.safeParse(path).success, path).toBe(true);
    }
  });

  it("refuses another site", () => {
    for (const path of ["https://evil.example", "//evil.example", "/\\evil.example", "orgs", "/a b"]) {
      expect(pageOfThisSite.safeParse(path).success, path).toBe(false);
    }
  });
});

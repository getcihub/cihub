import { describe, expect, it } from "vitest";

import type { ItemState, Pr } from "@/client/types.gen";
import { itemOf } from "@/test/fixtures";
import { etaOf, whereOf } from "./words";

const ahead = (...states: ItemState[]) => states.map((state, index) => ({ pr_number: 100 + index, state }));

function prOf(state: ItemState, changes: Partial<Pr> = {}): Pr {
  return itemOf(124, state, { author: "ada", title: "Fix the payment webhook retry", ...changes });
}

describe("the words of the PRs of a person", () => {
  it("say where a PR is", () => {
    expect(whereOf(prOf("pending", { ahead: ahead("testing", "pending") }), false, "main")).toBe(
      "3rd in line",
    );
    expect(whereOf(prOf("pending"), true, "main")).toBe("1st in line, paused");
    expect(whereOf(prOf("merged"), false, "main")).toBe("merged into main");
    expect(whereOf(prOf("not_ready"), false, "main")).toBe("not in line yet");
  });

  it("round the estimate of the server up to the minute", () => {
    expect(etaOf(prOf("pending", { merges_in_seconds: 610 }))).toBe("about 11 min");
    expect(etaOf(prOf("tests_passed", { merges_in_seconds: 0 }))).toBe("a few seconds");
    expect(etaOf(prOf("not_ready"))).toBeNull();
  });
});

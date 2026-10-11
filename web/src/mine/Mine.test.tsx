import { screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { ItemState, Pr } from "../client/types.gen";
import { renderApp } from "../test/app";
import { ada, answersOfMe, answersOfMine, itemOf, pageOf, queueOf } from "../test/fixtures";
import { etaOf, whereOf } from "./reads";

const ahead = (...states: ItemState[]) => states.map((state, index) => ({ pr_number: 100 + index, state }));

function prOf(state: ItemState, changes: Partial<Pr> = {}): Pr {
  return itemOf(124, state, { author: "ada", title: "Fix the payment webhook retry", ...changes });
}

afterEach(() => {
  vi.unstubAllGlobals();
  document.title = "";
});

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

describe("Your pull requests", () => {
  it("lists the PRs of the person on the queues, and names the first one in the tab", async () => {
    const pr = prOf("pending", { ahead: ahead("testing"), merges_in_seconds: 1200 });
    renderApp("/orgs/acme", {
      ...answersOfMe(ada),
      ...answersOfMine("acme", [pr]),
      "/api/v1/orgs/acme/queues": [200, pageOf("queues", [queueOf("web", "main")])],
    });

    const mine = await screen.findByRole("region", { name: "Your pull requests" });

    expect(within(mine).getByRole("link", { name: "#124 Fix the payment webhook retry" })).toHaveAttribute(
      "href",
      "/orgs/acme/repos/web/queues/main/prs/124",
    );
    expect(mine).toHaveTextContent("2nd in line");
    expect(mine).toHaveTextContent("merges in about 20 min");
    expect(
      within(mine).getByRole("img", { name: "1 pull request before it, then main" }),
    ).toBeInTheDocument();
    expect(document.title).toBe("#124 2nd in line · CIHub");
  });
});

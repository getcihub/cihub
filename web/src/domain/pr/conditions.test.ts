import { describe, expect, it } from "vitest";

import { conditionTree, conditionWords } from "./conditions";

describe("the words of a queue condition", () => {
  it("name the attribute, the operator and the value", () => {
    expect(conditionWords("labels.none: wip")).toEqual({
      attribute: "Labels",
      operator: "has none of",
      value: "wip",
    });
    expect(conditionWords("author: alice, bob")).toEqual({
      attribute: "Author",
      operator: "is",
      value: "alice, bob",
    });
    expect(conditionWords("files.count.max: 20")).toEqual({
      attribute: "Files",
      operator: "has at most",
      value: "20",
    });
    expect(conditionWords("queue_age.min: 2h")).toEqual({
      attribute: "Time in the queue",
      operator: "is longer than",
      value: "2h",
    });
  });

  it("show a check by its state, with its whole name", () => {
    expect(conditionWords("checks.success: test (ubuntu, 20)")).toEqual({
      attribute: "Checks",
      operator: "succeeded",
      value: "test (ubuntu, 20)",
    });
  });

  it("read a boolean without a value", () => {
    expect(conditionWords("draft: false")).toEqual({ attribute: "Draft", operator: "is not", value: null });
    expect(conditionWords("from_fork: true")).toEqual({
      attribute: "From a fork",
      operator: "is",
      value: null,
    });
  });

  it("leave a group and an unknown form as they are written", () => {
    expect(conditionWords("any: [draft: false; author: bot]")).toBeNull();
    expect(conditionWords("checks.success.none: lint")).toBeNull();
    expect(conditionWords("colour: blue")).toBeNull();
    expect(conditionWords("duration.max: 2h")).toBeNull();
  });
});

describe("the tree of a queue condition", () => {
  it("reads a group and the groups inside it", () => {
    const tree = conditionTree(
      "any: [author: dependabot[bot], renovate[bot]; none: [labels: wip; draft: true]]",
    );

    expect(tree).toMatchObject({
      type: "group",
      kind: "any",
      items: [
        { type: "condition", words: { attribute: "Author", value: "dependabot[bot], renovate[bot]" } },
        {
          type: "group",
          kind: "none",
          items: [{ words: { attribute: "Labels" } }, { words: { attribute: "Draft", operator: "is" } }],
        },
      ],
    });
  });

  it("keeps a condition as a leaf", () => {
    expect(conditionTree("labels.none: wip")).toMatchObject({
      type: "condition",
      written: "labels.none: wip",
    });
  });
});

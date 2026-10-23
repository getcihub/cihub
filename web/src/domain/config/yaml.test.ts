import { describe, expect, it } from "vitest";

import { yamlBlockOf } from "./yaml";

const FILE = `version: 1
queues:
  - branch: main
    strategy: batched
    queue_conditions:
      draft: false
      labels:
        none:
          - wip
    profiles:
      - name: bots
  - branch: "release/*"
    queue_conditions: { draft: false }
defaults:
  queue_conditions:
    conflict: false
`;

describe("the block of a key of a YAML file", () => {
  it("gives the key of an entry or of defaults, at the indent of the key", () => {
    expect(yamlBlockOf(FILE, "main", "queue_conditions")).toEqual([
      "queue_conditions:",
      "  draft: false",
      "  labels:",
      "    none:",
      "      - wip",
    ]);
    expect(yamlBlockOf(FILE, "release/*", "queue_conditions")).toEqual([
      "queue_conditions: { draft: false }",
    ]);
    expect(yamlBlockOf(FILE, null, "queue_conditions")).toEqual(["queue_conditions:", "  conflict: false"]);
  });

  it("gives nothing for a key that the part does not set", () => {
    expect(yamlBlockOf(FILE, "main", "merge_conditions")).toEqual([]);
    expect(yamlBlockOf(FILE, "dev", "queue_conditions")).toEqual([]);
  });
});

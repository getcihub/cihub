import type { ConditionBlock, ConditionDocument } from "@/client/types.gen";

// The conditions of the builder of the mockups v26: the attributes of schema/config/v1.json by subject, in the
// words of the builder, and the rows that the editor shows for the conditions of a document.

export type AttributeType =
  | "bool"
  | "string"
  | "list"
  | "int"
  | "duration"
  | "enum"
  | "permission"
  | "checks";

/** Where the values of an attribute come from: the placeholder of its field says it. */
type Source = "labels" | "people" | "reviewers" | "checks" | "branches" | "paths";

export interface AttributeWords {
  group: string;
  type: AttributeType;
  label: string;
  text: string;
  /** What a boolean says when it holds, and when it does not. */
  yes?: string;
  no?: string;
  /** The closed values of a choice. */
  options?: string[];
  unit?: string;
  placeholder?: string;
  source?: Source;
}

export const ATTRIBUTE_CATALOG: Record<string, AttributeWords> = {
  draft: {
    group: "Pull request",
    type: "bool",
    label: "Draft",
    yes: "is a draft",
    no: "is not a draft",
    text: "The PR is a draft.",
  },
  conflict: {
    group: "Pull request",
    type: "bool",
    label: "In conflict",
    yes: "conflicts with its base",
    no: "does not conflict with its base",
    text: "The PR conflicts with its base.",
  },
  behind_base: {
    group: "Pull request",
    type: "bool",
    label: "Behind its base",
    yes: "is behind its base",
    no: "is up to date with its base",
    text: "The branch of the PR is behind its base.",
  },
  from_fork: {
    group: "Pull request",
    type: "bool",
    label: "From a fork",
    yes: "comes from a fork",
    no: "does not come from a fork",
    text: "The PR comes from a fork.",
  },
  title: {
    group: "Pull request",
    type: "string",
    label: "Title",
    placeholder: "WIP*",
    text: "The title of the PR.",
  },
  body: {
    group: "Pull request",
    type: "string",
    label: "Description",
    placeholder: "Closes #*",
    text: "The description of the PR.",
  },
  base: {
    group: "Pull request",
    type: "string",
    label: "Base branch",
    source: "branches",
    text: "The base branch of the PR.",
  },
  head: {
    group: "Pull request",
    type: "string",
    label: "Head branch",
    source: "branches",
    text: "The head branch of the PR.",
  },
  files: {
    group: "Pull request",
    type: "list",
    label: "Files",
    source: "paths",
    text: "Paths of the files that the PR changes.",
  },
  additions: {
    group: "Pull request",
    type: "int",
    label: "Lines added",
    unit: "lines",
    text: "Lines added.",
  },
  deletions: {
    group: "Pull request",
    type: "int",
    label: "Lines deleted",
    unit: "lines",
    text: "Lines deleted.",
  },
  number: { group: "Pull request", type: "int", label: "Number", text: "The number of the PR." },
  pr_age: { group: "Pull request", type: "duration", label: "Age", text: "Time since the PR was opened." },
  review_decision: {
    group: "Reviews",
    type: "enum",
    label: "Review decision",
    options: ["approved", "changes_requested", "review_required", "none"],
    text: "The review decision of the PR.",
  },
  approved_reviews_by: {
    group: "Reviews",
    type: "list",
    label: "Approved by",
    source: "people",
    text: "Logins whose last review approves.",
  },
  changes_requested_reviews_by: {
    group: "Reviews",
    type: "list",
    label: "Changes requested by",
    source: "people",
    text: "Logins whose last review requests changes.",
  },
  review_requested: {
    group: "Reviews",
    type: "list",
    label: "Review requested from",
    source: "reviewers",
    text: "Logins and teams asked to review.",
  },
  review_threads_unresolved: {
    group: "Reviews",
    type: "list",
    label: "Unresolved threads on",
    source: "paths",
    text: "Paths of the unresolved review threads.",
  },
  labels: { group: "Labels", type: "list", label: "Labels", source: "labels", text: "The labels of the PR." },
  author: {
    group: "People",
    type: "string",
    label: "Author",
    source: "people",
    text: "The login of the author of the PR.",
  },
  sender: {
    group: "People",
    type: "string",
    label: "Sender",
    source: "people",
    text: "The login of the author of the command or of the submission.",
  },
  sender_permission: {
    group: "People",
    type: "permission",
    label: "Permission of the sender",
    options: ["none", "read", "write", "admin"],
    text: "The permission of the sender on the repository.",
  },
  checks: {
    group: "Checks",
    type: "checks",
    label: "Checks",
    source: "checks",
    text: "The checks of the head of the PR, by state.",
  },
  queue_position: {
    group: "Queue",
    type: "int",
    label: "Position in the queue",
    text: "Position in the queue, 0 at its head.",
  },
  queue_age: { group: "Queue", type: "duration", label: "Time in the queue", text: "Time in the queue." },
};

/** The operators of each type, in words: the key is the one of the row, which the file writes. */
export const OPERATORS: Record<AttributeType, [string, string][]> = {
  bool: [
    ["yes", "is"],
    ["no", "is not"],
  ],
  int: [
    ["is", "is"],
    ["min", "is at least"],
    ["max", "is at most"],
    ["between", "is between"],
  ],
  string: [
    ["is", "is"],
    ["not", "is not"],
    ["matches", "matches"],
    ["regex", "contains the regex"],
  ],
  list: [
    ["any", "has any of"],
    ["all", "has all of"],
    ["none", "has none of"],
    ["any_match", "has one that matches"],
    ["all_match", "all match"],
    ["none_match", "none match"],
    ["count_min", "has at least"],
    ["count_max", "has at most"],
  ],
  duration: [
    ["min", "is longer than"],
    ["max", "is shorter than"],
  ],
  enum: [["is", "is"]],
  permission: [
    ["min", "is at least"],
    ["is", "is"],
    ["max", "is at most"],
  ],
  checks: [
    ["success", "succeeded"],
    ["failure", "failed"],
    ["pending", "are pending"],
    ["neutral", "ended neutral"],
    ["skipped", "were skipped"],
  ],
};

/** What an operator means, from the descriptions of schema/config/v1.json: the list shows it under its name. */
export const OPERATOR_HINTS: Partial<Record<AttributeType, Record<string, string>>> = {
  list: {
    any: "At least one value is in the list.",
    all: "Every value is in the list.",
    none: "No value is in the list.",
    any_match: "An element matches one of the globs.",
    all_match: "Every element matches one of the globs, and the list is not empty.",
    none_match: "No element matches the globs.",
    count_min: "The list has at least this number of elements.",
    count_max: "The list has at most this number of elements.",
  },
  string: {
    is: "One of the values.",
    not: "None of the values.",
    matches: "Matches one of the globs.",
    regex: "A regular expression found anywhere.",
  },
  int: { between: "Both bounds are inclusive." },
  permission: {
    min: "In the order none < read < write < admin.",
    max: "In the order none < read < write < admin.",
  },
};

/** What a closed value means. */
export const VALUE_TEXTS: Record<string, string> = {
  approved: "the approval that the branch rules ask for",
  changes_requested: "a reviewer asks for changes",
  review_required: "a review is still required",
  none: "no decision yet",
  read: "can read the repository",
  write: "can push to the repository",
  admin: "administers the repository",
};

/** The placeholder of the values of an attribute. */
export function placeholderOf(attribute: AttributeWords, op: string): string {
  if (attribute.type === "enum" || attribute.type === "permission") {
    return "Add a value";
  }
  if (op.includes("match")) {
    return "Add a glob";
  }
  const bySource: Record<Source, string> = {
    labels: "Add a label",
    people: "Add a login",
    reviewers: "Add a login or a team",
    checks: "Add a check",
    branches: "Add a branch",
    paths: "Add a path",
  };
  return attribute.source
    ? bySource[attribute.source]
    : `Add a value, such as ${attribute.placeholder ?? "a text"}`;
}

/**
 * A row of the editor: an attribute of the catalog, an operator of its type, and its values, as the field of the
 * row holds them; a group of rows; or a condition of the file that the editor does not offer, shown as written.
 */
export type ConditionNode =
  | { kind: "row"; attribute: string; op: string; values: string[] }
  | { kind: "group"; block: ConditionBlock; items: ConditionNode[] }
  | { kind: "raw"; condition: ConditionDocument };

/** What the field of a row holds, by its operator. */
export type ValueKind = "none" | "num" | "range" | "dur" | "text" | "one" | "tokens";

export function valueKindOf(attribute: string, op: string): ValueKind {
  const type = ATTRIBUTE_CATALOG[attribute]?.type;
  if (type === "bool") {
    return "none";
  }
  if (type === "int") {
    return op === "between" ? "range" : "num";
  }
  if (type === "duration") {
    return "dur";
  }
  if (type === "string" && op === "regex") {
    return "text";
  }
  if (type === "list" && op.startsWith("count")) {
    return "num";
  }
  if (type === "permission" && op !== "is") {
    return "one";
  }
  return "tokens";
}

/** A new row of `attribute`: a boolean starts on "is not", a choice on the value that most rows take. */
export function newRow(attribute: string): ConditionNode {
  const words = ATTRIBUTE_CATALOG[attribute];
  const defaults: Record<string, string[]> = { review_decision: ["approved"], sender_permission: ["write"] };
  const op = words?.type === "bool" ? "no" : (OPERATORS[words?.type ?? "string"][0]?.[0] ?? "is");
  return { kind: "row", attribute, op, values: defaults[attribute] ?? [] };
}

const isCount = (text: string | undefined) => /^\d+$/.test(text ?? "");

/** The row has what the file needs; a row that does not stays out of the file. */
export function isComplete(node: ConditionNode): boolean {
  if (node.kind === "raw") {
    return true;
  }
  if (node.kind === "group") {
    return node.items.some(isComplete);
  }

  const [first, second] = node.values;
  switch (valueKindOf(node.attribute, node.op)) {
    case "none":
      return true;
    case "num":
      return isCount(first);
    case "range":
      return (
        (isCount(first) || isCount(second)) &&
        !(isCount(first) && isCount(second) && Number(first) > Number(second))
      );
    case "dur":
      return /^\d+[mhd]$/.test(first ?? "");
    case "text":
      return Boolean(first?.trim());
    case "one":
      return node.values.length === 1;
    case "tokens":
      return node.values.length > 0;
  }
}

/** How many rows of `nodes` are not complete. */
export function countIncomplete(nodes: ConditionNode[]): number {
  return nodes.reduce(
    (count, node) =>
      count + (node.kind === "group" ? countIncomplete(node.items) : Number(!isComplete(node))),
    0,
  );
}

/** The row of a condition of the file, or `null` for one that the editor does not offer. */
function rowOf(condition: Extract<ConditionDocument, { kind: "condition" }>): ConditionNode | null {
  const { attribute, values } = condition;
  const operator = condition.operator ?? null;
  if (attribute.startsWith("checks.") && operator === null) {
    return { kind: "row", attribute: "checks", op: attribute.slice("checks.".length), values };
  }

  const type = ATTRIBUTE_CATALOG[attribute]?.type;
  const row = (op: string, rowValues = values): ConditionNode => ({
    kind: "row",
    attribute,
    op,
    values: rowValues,
  });
  switch (type) {
    case "bool":
      return operator === null ? row(values[0] === "true" ? "yes" : "no", []) : null;
    case "int":
      return operator === null ? row("is") : ["min", "max"].includes(operator) ? row(operator) : null;
    case "duration":
      return operator !== null && ["min", "max"].includes(operator) ? row(operator) : null;
    case "string":
      return operator === null
        ? row("is")
        : ["not", "matches", "regex"].includes(operator)
          ? row(operator)
          : null;
    case "list": {
      const ops: Record<string, string> = { "count.min": "count_min", "count.max": "count_max" };
      if (operator === null) {
        return row("all");
      }
      const op = ops[operator] ?? operator;
      return OPERATORS.list.some(([key]) => key === op) ? row(op) : null;
    }
    case "enum":
      return operator === null ? row("is") : null;
    case "permission":
      return operator === null ? row("is") : ["min", "max"].includes(operator) ? row(operator) : null;
    default:
      return null;
  }
}

/** The rows of the conditions of a document: a `min` then a `max` of one number are one row "is between". */
export function nodesOf(conditions: ConditionDocument[]): ConditionNode[] {
  const nodes: ConditionNode[] = [];
  for (const condition of conditions) {
    if (condition.kind === "block") {
      nodes.push({ kind: "group", block: condition.block, items: nodesOf(condition.conditions) });
      continue;
    }

    const row = rowOf(condition);
    const last = nodes.at(-1);
    if (
      row?.kind === "row" &&
      last?.kind === "row" &&
      ATTRIBUTE_CATALOG[row.attribute]?.type === "int" &&
      last.attribute === row.attribute &&
      last.op === "min" &&
      row.op === "max"
    ) {
      nodes[nodes.length - 1] = {
        kind: "row",
        attribute: row.attribute,
        op: "between",
        values: [last.values[0] ?? "", row.values[0] ?? ""],
      };
      continue;
    }
    nodes.push(row ?? { kind: "raw", condition });
  }
  return nodes;
}

/** The conditions of the file of a row. */
function conditionsOfRow(node: Extract<ConditionNode, { kind: "row" }>): ConditionDocument[] {
  const { attribute, op, values } = node;
  const condition = (operator: string | null, conditionValues: string[]): ConditionDocument => ({
    kind: "condition",
    attribute,
    operator,
    values: conditionValues,
  });
  const type = ATTRIBUTE_CATALOG[attribute]?.type;
  if (type === "checks") {
    return [{ kind: "condition", attribute: `checks.${op}`, operator: null, values }];
  }
  if (type === "bool") {
    return [condition(null, [op === "yes" ? "true" : "false"])];
  }
  if (op === "between") {
    const bound = (value: string | undefined) => (value ? [value] : []);
    return [condition("min", bound(values[0])), condition("max", bound(values[1]))];
  }
  const keys: Record<string, string | null> = { is: null, count_min: "count.min", count_max: "count.max" };
  // A list without a key holds all its values: the file writes it so.
  const operator = type === "list" && op === "all" ? null : op in keys ? (keys[op] ?? null) : op;
  return [
    condition(
      operator,
      values.filter((value) => value !== ""),
    ),
  ];
}

/** The conditions of a document of `nodes`; `isComplete` leaves out what the file would refuse. */
export function conditionsOf(nodes: ConditionNode[], onlyComplete = false): ConditionDocument[] {
  return nodes.flatMap((node): ConditionDocument[] => {
    if (onlyComplete && !isComplete(node)) {
      return [];
    }
    switch (node.kind) {
      case "raw":
        return [node.condition];
      case "group": {
        const conditions = conditionsOf(node.items, onlyComplete);
        return onlyComplete && conditions.length === 0
          ? []
          : [{ kind: "block", block: node.block, conditions }];
      }
      case "row": {
        const conditions = conditionsOfRow(node);
        return onlyComplete
          ? conditions.filter((condition) => condition.kind !== "condition" || condition.values.length > 0)
          : conditions;
      }
    }
  });
}

/** The conditions of `conditions` that the file can hold: the editor keeps the others in the draft. */
export function completeConditions(conditions: ConditionDocument[]): ConditionDocument[] {
  return conditionsOf(nodesOf(conditions), true);
}

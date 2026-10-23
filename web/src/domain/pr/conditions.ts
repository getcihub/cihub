// The attributes and operators of crates/conditions, in the words of the condition builder of the mockups v26.

export type Kind = "bool" | "int" | "duration" | "text" | "list" | "choice";

export const ATTRIBUTES: Record<string, { label: string; kind: Kind }> = {
  draft: { label: "Draft", kind: "bool" },
  conflict: { label: "In conflict", kind: "bool" },
  from_fork: { label: "From a fork", kind: "bool" },
  behind_base: { label: "Behind its base", kind: "bool" },
  number: { label: "Number", kind: "int" },
  title: { label: "Title", kind: "text" },
  body: { label: "Description", kind: "text" },
  review_decision: { label: "Review decision", kind: "choice" },
  approved_reviews_by: { label: "Approved by", kind: "list" },
  changes_requested_reviews_by: { label: "Changes requested by", kind: "list" },
  review_requested: { label: "Review requested from", kind: "list" },
  review_threads_unresolved: { label: "Unresolved threads on", kind: "list" },
  labels: { label: "Labels", kind: "list" },
  author: { label: "Author", kind: "text" },
  sender: { label: "Sender", kind: "text" },
  sender_permission: { label: "Permission of the sender", kind: "choice" },
  files: { label: "Files", kind: "list" },
  additions: { label: "Lines added", kind: "int" },
  deletions: { label: "Lines deleted", kind: "int" },
  base: { label: "Base branch", kind: "text" },
  head: { label: "Head branch", kind: "text" },
  queue_position: { label: "Position in the queue", kind: "int" },
  queue_age: { label: "Time in the queue", kind: "duration" },
  pr_age: { label: "Age", kind: "duration" },
};

// The key of an operator under its attribute; "" when the attribute takes its values directly.
export const OPERATORS: Record<Kind, Record<string, string>> = {
  bool: {},
  int: { "": "is", min: "is at least", max: "is at most" },
  duration: { min: "is longer than", max: "is shorter than" },
  text: { "": "is", not: "is not", matches: "matches", regex: "contains the regex" },
  list: {
    "": "has all of",
    all: "has all of",
    any: "has any of",
    none: "has none of",
    any_match: "has one that matches",
    all_match: "all match",
    none_match: "none match",
    count: "has exactly",
    "count.min": "has at least",
    "count.max": "has at most",
  },
  choice: { "": "is", min: "is at least", max: "is at most" },
};

// The builder shows the checks as one attribute, with their state as the operator.
export const CHECK_STATES: Record<string, string> = {
  "checks.success": "succeeded",
  "checks.failure": "failed",
  "checks.pending": "are pending",
  "checks.neutral": "ended neutral",
  "checks.skipped": "were skipped",
};

/** A condition in words: `Labels` `has none of` `wip`; no value for a boolean. */
export interface ConditionWords {
  attribute: string;
  operator: string;
  value: string | null;
}

/**
 * A condition as the API writes it, `labels.none: wip`, in words; `null` for a group (`any: [a; b]`) or a form
 * that the dashboard does not know. The values stay one text: the API joins them with ", ", which a check name
 * can hold.
 */
export function conditionWords(written: string): ConditionWords | null {
  const split = written.indexOf(": ");
  if (split < 0) {
    return null;
  }

  const path = written.slice(0, split);
  const value = written.slice(split + 2);
  const checkState = CHECK_STATES[path];
  if (checkState !== undefined) {
    return { attribute: "Checks", operator: checkState, value };
  }

  const found = Object.entries(ATTRIBUTES).find(([key]) => path === key || path.startsWith(`${key}.`));
  if (found === undefined) {
    return null;
  }

  const [name, { label, kind }] = found;
  if (kind === "bool") {
    return path === name
      ? { attribute: label, operator: value === "true" ? "is" : "is not", value: null }
      : null;
  }

  const operator = OPERATORS[kind][path.slice(name.length + 1)];

  return operator === undefined ? null : { attribute: label, operator, value };
}

/** A queue condition as a tree: a condition, or a group of the file (`all`, `any`, `none`) of other ones. */
export type ConditionNode =
  | { type: "condition"; written: string; words: ConditionWords | null }
  | { type: "group"; kind: "all" | "any" | "none"; items: ConditionNode[] };

const GROUP = /^(all|any|none): \[(.*)\]$/s;

/** A condition as the API writes it, a group as `any: [a; b]`, as a tree. */
export function conditionTree(written: string): ConditionNode {
  const group = GROUP.exec(written);
  if (!group?.[1] || group[2] === undefined) {
    return { type: "condition", written, words: conditionWords(written) };
  }

  const kind = group[1] as "all" | "any" | "none";

  return { type: "group", kind, items: elementsOf(group[2]).map(conditionTree) };
}

// The API separates the elements of a group with "; ", and a group inside a group keeps its own brackets.
function elementsOf(list: string): string[] {
  const elements: string[] = [];
  let depth = 0;
  let start = 0;
  for (let index = 0; index < list.length; index += 1) {
    if (list[index] === "[") {
      depth += 1;
    } else if (list[index] === "]") {
      depth -= 1;
    } else if (depth === 0 && list.startsWith("; ", index)) {
      elements.push(list.slice(start, index));
      start = index + 2;
    }
  }
  elements.push(list.slice(start));

  return elements;
}

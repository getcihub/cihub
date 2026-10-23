import type { ReactNode } from "react";

import type { PriorityRuleDocument } from "@/client/types.gen";
import { PlusIcon } from "@/components/icons/plus-icon";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { PRIORITIES } from "@/domain/config/words";
import { ConditionEditor } from "./condition-editor";
import { ChoiceField, TextField } from "./controls";

/** Three common rules, to start from: a click adds one, which the person then adjusts. */
const TEMPLATES: { rule: PriorityRuleDocument; text: ReactNode }[] = [
  {
    rule: {
      name: "urgent",
      priority: "urgent",
      conditions: [{ kind: "condition", attribute: "labels", operator: "any", values: ["urgent", "hotfix"] }],
    },
    text: (
      <>
        A pull request with the label <code>urgent</code> or <code>hotfix</code> goes ahead of the others.
      </>
    ),
  },
  {
    rule: {
      name: "dependencies",
      priority: "low",
      conditions: [
        {
          kind: "condition",
          attribute: "author",
          operator: null,
          values: ["dependabot[bot]", "renovate[bot]"],
        },
      ],
    },
    text: "The updates of the dependency bots wait behind the work of the team.",
  },
  {
    rule: {
      name: "docs-only",
      priority: "high",
      conditions: [
        { kind: "condition", attribute: "files", operator: "all_match", values: ["docs/**", "**/*.md"] },
      ],
    },
    text: "A pull request that changes only the docs goes ahead: it is small.",
  },
];

/**
 * The priority rules of the file, in its order: the first one that a pull request meets gives its priority. A
 * rule without a condition stays out of the file.
 */
export function PriorityRules({
  rules,
  canEdit,
  onChange,
}: {
  rules: PriorityRuleDocument[];
  canEdit: boolean;
  onChange: (rules: PriorityRuleDocument[]) => void;
}) {
  const put = (index: number, rule: PriorityRuleDocument | null) => {
    onChange(
      rule === null
        ? rules.filter((_, other) => other !== index)
        : rules.map((other, at) => (at === index ? rule : other)),
    );
  };

  if (rules.length === 0) {
    return (
      <Card className="grid justify-items-start gap-2 p-4">
        <b className="font-medium">
          No rule: every pull request is <code>medium</code>
        </b>
        <p className="max-w-160 text-base text-secondary-foreground">
          The queue takes the pull requests in the order of their submission. A rule moves some of them ahead,
          or behind: an urgent fix before a dependency update. Start from one of these, then adjust it:
        </p>
        <div className="my-1 grid w-full gap-1.5">
          {TEMPLATES.map((template) => (
            <button
              key={template.rule.name}
              type="button"
              disabled={!canEdit}
              className="flex w-full cursor-pointer items-center justify-between gap-3 rounded-sm border border-dashed border-border px-3 py-2.5 text-left text-base enabled:hover:border-solid enabled:hover:border-primary-border enabled:hover:bg-primary-muted disabled:cursor-default"
              onClick={() => {
                onChange([structuredClone(template.rule)]);
              }}
            >
              <span>{template.text}</span>
              <span className="rounded-xs border border-border px-1.5 text-sm text-secondary-foreground">
                {template.rule.priority}
              </span>
            </button>
          ))}
        </div>
        {canEdit && (
          <Button
            className="[&>svg]:size-3"
            onClick={() => {
              onChange([emptyRule()]);
            }}
          >
            <PlusIcon />
            Start from an empty rule
          </Button>
        )}
      </Card>
    );
  }

  return (
    <>
      <p className="text-base text-secondary-foreground">
        The first rule that a pull request meets gives its priority; <code>--priority</code> on the command
        wins over the rules. Without a rule, a pull request is <code>medium</code>. An urgent pull request
        goes ahead of the others in the same queue, not in a second queue.
      </p>
      <ol className="grid gap-2" aria-label="Priority rules">
        {rules.map((rule, index) => (
          <li key={index}>
            <Card className="p-0">
              <div className="flex flex-wrap items-center gap-2.5 border-b border-divider px-4 py-2.5">
                <span className="mr-1 inline-grid size-4.5 place-items-center rounded-full border border-border bg-muted text-2xs">
                  {index + 1}
                </span>
                {canEdit ? (
                  <TextField
                    label="Name of the rule"
                    className="w-44 font-mono"
                    disabled={false}
                    value={rule.name}
                    onCommit={(name) => {
                      put(index, { ...rule, name });
                    }}
                  />
                ) : (
                  <b className="font-medium">{rule.name}</b>
                )}
                <span className="text-base text-secondary-foreground">gives the priority</span>
                <ChoiceField
                  label="Priority"
                  disabled={!canEdit}
                  value={rule.priority}
                  options={[
                    ...PRIORITIES,
                    ...(PRIORITIES.some((priority) => priority.value === rule.priority)
                      ? []
                      : [{ value: rule.priority, label: rule.priority }]),
                  ]}
                  onChange={(priority) => {
                    put(index, { ...rule, priority });
                  }}
                />
                {canEdit && (
                  <button
                    type="button"
                    className="ml-auto cursor-pointer rounded-sm px-2 py-1 text-base text-destructive hover:bg-destructive-muted"
                    onClick={() => {
                      put(index, null);
                    }}
                  >
                    Remove
                  </button>
                )}
              </div>
              <div className="grid gap-2 py-3 pr-4 pl-1.5">
                <span className="pl-2.5 text-md text-muted-foreground">
                  to a pull request that meets every condition:
                </span>
                <ConditionEditor
                  conditions={rule.conditions}
                  canEdit={canEdit}
                  empty="No condition yet: a rule needs one, so it stays out of the file."
                  onChange={(conditions) => {
                    put(index, { ...rule, conditions });
                  }}
                />
              </div>
            </Card>
          </li>
        ))}
      </ol>
      {canEdit && (
        <div>
          <Button
            className="[&>svg]:size-3"
            onClick={() => {
              onChange([...rules, emptyRule()]);
            }}
          >
            <PlusIcon />
            Add a rule
          </Button>
        </div>
      )}
    </>
  );
}

function emptyRule(): PriorityRuleDocument {
  return { name: "rule", priority: "high", conditions: [] };
}

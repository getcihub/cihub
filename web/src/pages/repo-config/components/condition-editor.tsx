import { type ReactNode, useState } from "react";

import type { ConditionBlock, ConditionDocument } from "@/client/types.gen";
import { PlusIcon } from "@/components/icons/plus-icon";
import { WarningIcon } from "@/components/icons/warning-icon";
import { Button } from "@/components/ui/button";
import { OptionPicker } from "@/components/ui/option-picker";
import { Segmented } from "@/components/ui/segmented";
import { TokenField } from "@/components/ui/token-field";
import {
  ATTRIBUTE_CATALOG,
  type ConditionNode,
  OPERATORS,
  OPERATOR_HINTS,
  VALUE_TEXTS,
  conditionsOf,
  countIncomplete,
  isComplete,
  newRow,
  nodesOf,
  placeholderOf,
  valueKindOf,
} from "@/domain/config/conditions";
import { cn } from "@/lib/cn";
import { AttributeModal } from "./attribute-modal";

type Row = Extract<ConditionNode, { kind: "row" }>;
type Group = Extract<ConditionNode, { kind: "group" }>;

/** A place in the tree of rows: the index of each item from the top. */
type Path = number[];

/** What the catalog of the attributes does once the person chooses one. */
type Adding = { mode: "add" | "group"; path: Path } | { mode: "replace"; path: Path };

const BLOCKS: Record<ConditionBlock, { word: string; hint: string }> = {
  any: { word: "or", hint: "At least one of these holds." },
  none: { word: "and not", hint: "None of these holds." },
  all: { word: "and", hint: "Every one of these holds." },
};

/** The node at `path`, and the list that holds it. */
function at(nodes: ConditionNode[], path: Path): { list: ConditionNode[]; index: number } {
  let list = nodes;
  for (const index of path.slice(0, -1)) {
    const node = list[index];
    if (node?.kind !== "group") {
      throw new Error("no group at this path");
    }
    list = node.items;
  }
  return { list, index: path.at(-1) ?? 0 };
}

/** The items of the group at `path`: the list itself for the top. */
function itemsAt(nodes: ConditionNode[], path: Path): ConditionNode[] {
  if (path.length === 0) {
    return nodes;
  }
  const { list, index } = at(nodes, path);
  const node = list[index];
  if (node?.kind !== "group") {
    throw new Error("no group at this path");
  }
  return node.items;
}

/**
 * The conditions of a map of the file, as the condition builder of the mockups v26: a row reads an attribute
 * with an operator and its values; a group of the rows holds if one of them does ("or") or if none does ("and
 * not"). A row that waits for a value stays out of the file. Read only, the rows show their values.
 */
export function ConditionEditor({
  conditions,
  canEdit,
  empty,
  onChange,
}: {
  conditions: ConditionDocument[];
  canEdit: boolean;
  /** What holds without a condition. */
  empty: string;
  onChange: (conditions: ConditionDocument[]) => void;
}) {
  const nodes = nodesOf(conditions);
  const [adding, setAdding] = useState<Adding | null>(null);
  const incomplete = countIncomplete(nodes);
  const change = (edit: (copy: ConditionNode[]) => void) => {
    const copy = structuredClone(nodes);
    edit(copy);
    onChange(conditionsOf(copy));
  };
  const choose = (attribute: string) => {
    if (!adding) {
      return;
    }
    const row = newRow(attribute);
    change((copy) => {
      if (adding.mode === "replace") {
        const { list, index } = at(copy, adding.path);
        list[index] = row;
      } else {
        itemsAt(copy, adding.path).push(
          adding.mode === "group" ? { kind: "group", block: "any", items: [row] } : row,
        );
      }
    });
    setAdding(null);
  };

  return (
    <div className="grid gap-1.5" data-slot="condition-editor">
      {nodes.length === 0 && <p className="pt-2 pb-0.5 pl-13.5 text-md text-muted-foreground">{empty}</p>}
      <Tree nodes={nodes} path={[]} depth={0} canEdit={canEdit} change={change} onAdd={setAdding} />
      {incomplete > 0 && (
        <p className="ml-13.5 flex items-center gap-1.5 text-md text-warning [&>svg]:size-3.5">
          <WarningIcon />
          {incomplete} {incomplete > 1 ? "conditions are" : "condition is"} not complete:{" "}
          {incomplete > 1 ? "they stay" : "it stays"} out of the file.
        </p>
      )}
      {adding && (
        <AttributeModal
          onClose={() => {
            setAdding(null);
          }}
          onChoose={choose}
        />
      )}
    </div>
  );
}

interface TreeProps {
  nodes: ConditionNode[];
  path: Path;
  depth: number;
  canEdit: boolean;
  change: (edit: (copy: ConditionNode[]) => void) => void;
  onAdd: (adding: Adding) => void;
  /** `any` and `none` join their items with "or", the others with "and". */
  block?: ConditionBlock;
}

function Tree({ nodes, path, depth, canEdit, change, onAdd, block = "all" }: TreeProps) {
  const join = block === "all" ? "and" : "or";

  return (
    <>
      {nodes.map((node, index) => {
        const nodePath = [...path, index];
        // An "and not" group says its own and: the word on its left would repeat it.
        const word =
          index === 0
            ? depth > 0
              ? ""
              : "where"
            : node.kind === "group" && node.block === "none" && join === "and"
              ? ""
              : join;
        const remove = () => {
          change((copy) => {
            const { list, index: slot } = at(copy, nodePath);
            list.splice(slot, 1);
          });
        };
        return (
          <Line
            key={index}
            word={word}
            depth={depth}
            isIncomplete={!isComplete(node)}
            onRemove={canEdit ? remove : null}
          >
            {node.kind === "group" ? (
              <GroupBlock
                group={node}
                path={nodePath}
                depth={depth + 1}
                canEdit={canEdit}
                change={change}
                onAdd={onAdd}
              />
            ) : node.kind === "row" ? (
              <RowFields
                row={node}
                canEdit={canEdit}
                onReplace={() => {
                  onAdd({ mode: "replace", path: nodePath });
                }}
                onChange={(row) => {
                  change((copy) => {
                    const { list, index: slot } = at(copy, nodePath);
                    list[slot] = row;
                  });
                }}
              />
            ) : (
              <code className="self-center text-md">{writtenOf(node.condition)}</code>
            )}
          </Line>
        );
      })}
      {canEdit && (
        <div className="mt-0.5 flex items-start gap-2.5">
          <span className={cn("flex-none", depth > 0 ? "w-8.5" : "w-11")} />
          <Button
            className="[&>svg]:size-3"
            onClick={() => {
              onAdd({ mode: "add", path });
            }}
          >
            <PlusIcon />
            Add a condition
          </Button>
          {depth === 0 && (
            <Button
              className="[&>svg]:size-3"
              onClick={() => {
                onAdd({ mode: "group", path });
              }}
            >
              <PlusIcon />
              Add a group
            </Button>
          )}
        </div>
      )}
    </>
  );
}

/** A line of the editor: the word that joins it to the line above, its fields, and ×. */
function Line({
  word,
  depth,
  isIncomplete,
  onRemove,
  children,
}: {
  word: string;
  depth: number;
  isIncomplete: boolean;
  onRemove: (() => void) | null;
  children: ReactNode;
}) {
  return (
    <div className="flex items-start gap-2.5" data-incomplete={isIncomplete || undefined}>
      <span
        className={cn(
          "flex-none pt-1.25 text-right text-md text-muted-foreground",
          depth > 0 ? "w-8.5" : "w-11 max-md:w-8",
        )}
      >
        {word}
      </span>
      {children}
      {onRemove && (
        <button
          type="button"
          className="size-7 flex-none cursor-pointer rounded-sm text-3xl text-muted-foreground hover:bg-muted hover:text-destructive"
          aria-label="Remove the condition"
          onClick={onRemove}
        >
          ×
        </button>
      )}
    </div>
  );
}

function GroupBlock({
  group,
  path,
  depth,
  canEdit,
  change,
  onAdd,
}: {
  group: Group;
  path: Path;
  depth: number;
  canEdit: boolean;
  change: TreeProps["change"];
  onAdd: TreeProps["onAdd"];
}) {
  // The top of a list is already an "and": a group is an "or" or an "and not"; a file can hold an "and" group.
  const kinds = (["any", "none", ...(group.block === "all" ? ["all" as const] : [])] as ConditionBlock[]).map(
    (block) => ({ value: block, label: BLOCKS[block].word, hint: BLOCKS[block].hint }),
  );

  return (
    <div className="grid min-w-0 flex-1 gap-1.5 border-l-2 border-primary-border pt-0.5 pb-1">
      <div className="flex items-center gap-2 pl-3">
        {canEdit ? (
          <OptionPicker
            label="Kind of the group"
            value={group.block}
            options={kinds}
            onChange={(block) => {
              change((copy) => {
                const { list, index } = at(copy, path);
                const node = list[index];
                if (node?.kind === "group") {
                  node.block = block;
                }
              });
            }}
          />
        ) : (
          <Chip>{BLOCKS[group.block].word}</Chip>
        )}
        <span className="text-sm text-muted-foreground">{BLOCKS[group.block].hint}</span>
      </div>
      {group.items.length === 0 && (
        <p className="pl-3 text-md text-muted-foreground">An empty group stays out of the file.</p>
      )}
      <Tree
        nodes={group.items}
        path={path}
        depth={depth}
        canEdit={canEdit}
        change={change}
        onAdd={onAdd}
        block={group.block}
      />
    </div>
  );
}

/** A chip of the builder: an attribute, the word of a group. */
function Chip({ children, isAttribute = false }: { children: ReactNode; isAttribute?: boolean }) {
  return (
    <span
      className={cn(
        "inline-flex h-7 items-center rounded-sm border px-2.5 text-base font-medium whitespace-nowrap",
        isAttribute ? "border-primary-border bg-primary-muted text-primary" : "border-border bg-muted",
      )}
    >
      {children}
    </span>
  );
}

/** The attribute, the operator and the value of a row. */
function RowFields({
  row,
  canEdit,
  onReplace,
  onChange,
}: {
  row: Row;
  canEdit: boolean;
  onReplace: () => void;
  onChange: (row: Row) => void;
}) {
  const words = ATTRIBUTE_CATALOG[row.attribute];
  if (!words) {
    return null;
  }

  const ops = OPERATORS[words.type];
  const opWords = ops.find(([op]) => op === row.op)?.[1] ?? row.op;

  return (
    <div className="flex min-w-0 flex-1 flex-wrap items-center gap-1.5">
      {canEdit ? (
        <button
          type="button"
          title={`${words.text} Click to read another attribute.`}
          className="inline-flex h-7 cursor-pointer items-center rounded-sm border border-primary-border bg-primary-muted px-2.5 text-base font-medium whitespace-nowrap text-primary"
          onClick={onReplace}
        >
          {words.label}
        </button>
      ) : (
        <Chip isAttribute>{words.label}</Chip>
      )}
      {words.type === "bool" ? (
        <>
          {canEdit ? (
            <Segmented
              label={words.label}
              value={row.op}
              options={[
                { value: "yes", label: "is", title: `The pull request ${words.yes ?? ""}` },
                { value: "no", label: "is not", title: `The pull request ${words.no ?? ""}` },
              ]}
              onChange={(op) => {
                onChange({ ...row, op });
              }}
            />
          ) : (
            <span className="text-base whitespace-nowrap text-secondary-foreground">{opWords}</span>
          )}
          <code className="ml-1 border-0 bg-transparent p-0 text-sm text-muted-foreground">
            {row.attribute}: {String(row.op === "yes")}
          </code>
        </>
      ) : canEdit && ops.length > 1 ? (
        <OptionPicker
          label="Operator"
          value={row.op}
          options={ops.map(([op, label]) => ({ value: op, label, hint: OPERATOR_HINTS[words.type]?.[op] }))}
          onChange={(op) => {
            const isSameKind = valueKindOf(row.attribute, op) === valueKindOf(row.attribute, row.op);
            onChange({ ...row, op, values: isSameKind ? row.values : [] });
          }}
        />
      ) : (
        <span className="text-base whitespace-nowrap text-secondary-foreground">{opWords}</span>
      )}
      <ValueFields row={row} canEdit={canEdit} onChange={onChange} />
    </div>
  );
}

const UNITS = [
  { value: "m", label: "minutes" },
  { value: "h", label: "hours" },
  { value: "d", label: "days" },
] as const;

function NumberInput({
  value,
  placeholder,
  label,
  canEdit,
  onChange,
}: {
  value: string;
  placeholder: string;
  label: string;
  canEdit: boolean;
  onChange: (value: string) => void;
}) {
  if (!canEdit) {
    return <b className="font-medium">{value}</b>;
  }
  return (
    <input
      className={cn(
        "h-7 w-16 rounded-sm border border-border bg-card px-2 text-right tabular-nums focus:border-primary focus:shadow-focus focus:outline-none",
        !/^\d+$/.test(value) && "border-dashed border-warning-border",
      )}
      inputMode="numeric"
      aria-label={label}
      placeholder={placeholder}
      value={value}
      onChange={(event) => {
        onChange(event.target.value.trim());
      }}
    />
  );
}

/** The value of a row, by its operator: values, a number, two bounds, a duration, a regex or one choice. */
function ValueFields({
  row,
  canEdit,
  onChange,
}: {
  row: Row;
  canEdit: boolean;
  onChange: (row: Row) => void;
}) {
  const words = ATTRIBUTE_CATALOG[row.attribute];
  if (!words) {
    return null;
  }

  const [first = "", second = ""] = row.values;
  const set = (values: string[]) => {
    onChange({ ...row, values });
  };
  const unit = row.op.startsWith("count") ? (words.source === "labels" ? "labels" : "items") : words.unit;
  const unitText = unit && <span className="text-base text-secondary-foreground">{unit}</span>;

  switch (valueKindOf(row.attribute, row.op)) {
    case "none":
      return null;
    case "num":
      return (
        <>
          <NumberInput
            value={first}
            placeholder="0"
            label="Number"
            canEdit={canEdit}
            onChange={(value) => {
              set([value]);
            }}
          />
          {unitText}
        </>
      );
    case "range":
      return (
        <>
          <NumberInput
            value={first}
            placeholder="min"
            label="Minimum"
            canEdit={canEdit}
            onChange={(value) => {
              set([value, second]);
            }}
          />
          <span className="text-base text-secondary-foreground">and</span>
          <NumberInput
            value={second}
            placeholder="max"
            label="Maximum"
            canEdit={canEdit}
            onChange={(value) => {
              set([first, value]);
            }}
          />
          {unitText}
        </>
      );
    case "dur": {
      // The format writes a duration as a number and a unit: m (minutes), h (hours) or d (days).
      const match = /^(\d*)([mhd]?)$/.exec(first);
      const amount = match?.[1] ?? "";
      const durationUnit = (
        match?.[2] === "" || match?.[2] === undefined ? "h" : match[2]
      ) as (typeof UNITS)[number]["value"];
      return (
        <>
          <NumberInput
            value={amount}
            placeholder="2"
            label="Duration"
            canEdit={canEdit}
            onChange={(value) => {
              set([`${value}${durationUnit}`]);
            }}
          />
          {canEdit ? (
            <OptionPicker
              label="Unit"
              value={durationUnit}
              options={[...UNITS]}
              onChange={(next) => {
                set([`${amount}${next}`]);
              }}
            />
          ) : (
            <span className="text-base text-secondary-foreground">
              {UNITS.find((option) => option.value === durationUnit)?.label}
            </span>
          )}
        </>
      );
    }
    case "text":
      return canEdit ? (
        <input
          className={cn(
            "h-7 w-60 rounded-sm border border-border bg-card px-2 font-mono text-md focus:border-primary focus:shadow-focus focus:outline-none",
            !first.trim() && "border-dashed border-warning-border",
          )}
          aria-label="Regular expression"
          placeholder="^(feat|fix):"
          value={first}
          onChange={(event) => {
            set([event.target.value]);
          }}
        />
      ) : (
        <code>{first}</code>
      );
    case "one":
      return canEdit ? (
        <OptionPicker
          label="Value"
          value={first || (words.options?.[0] ?? "")}
          options={(words.options ?? []).map((option) => ({
            value: option,
            label: option,
            hint: VALUE_TEXTS[option],
          }))}
          onChange={(value) => {
            set([value]);
          }}
        />
      ) : (
        <span className="inline-flex h-5.5 items-center rounded-xs border border-border bg-muted px-2 font-mono text-md">
          {first}
        </span>
      );
    case "tokens": {
      const isStrict = words.type === "enum" || words.type === "permission";
      const isGlob = row.op.includes("match");
      return (
        <TokenField
          label="Values"
          values={row.values}
          placeholder={placeholderOf(words, row.op)}
          readOnly={!canEdit}
          isIncomplete={row.values.length === 0}
          isStrict={isStrict}
          suggestions={(words.options ?? []).map((option) => ({ value: option, meta: VALUE_TEXTS[option] }))}
          note={
            isStrict ? undefined : isGlob ? (
              <>
                A glob: <code>*</code> stays in one <code>/</code> segment, <code>**</code> crosses them.
              </>
            ) : (
              "Type a value, then Enter."
            )
          }
          onChange={set}
        />
      );
    }
  }
}

/** A condition that the editor does not offer, as the file writes it. */
function writtenOf(condition: ConditionDocument): string {
  if (condition.kind === "block") {
    return `${condition.block}: [${condition.conditions.map(writtenOf).join("; ")}]`;
  }
  const path = condition.operator ? `${condition.attribute}.${condition.operator}` : condition.attribute;
  return `${path}: ${condition.values.join(", ")}`;
}

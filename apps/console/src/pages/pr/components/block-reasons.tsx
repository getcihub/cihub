import type { Pr } from "@/client/types.gen";
import { cn, Prose } from "@cihub/ui";
import { type ConditionNode, conditionTree } from "@/domain/pr/conditions";
import { blockSentence } from "@/domain/pr/words";

const CHIP = "inline-flex h-6 items-center rounded-xs border px-2 text-sm font-medium whitespace-nowrap";

// The words of a group in the condition builder of the mockups v26.
const GROUPS = {
  any: { word: "or", hint: "At least one of these holds." },
  none: { word: "and not", hint: "None of these holds." },
  all: { word: "and", hint: "Every one of these holds." },
};

/** A condition as the condition builder shows it: the attribute, the operator, the value. */
function ConditionRow({ node }: { node: Extract<ConditionNode, { type: "condition" }> }) {
  const { words } = node;
  if (!words) {
    return <code className="self-center">{node.written}</code>;
  }

  return (
    <div className="flex min-w-0 flex-wrap items-center gap-1.5">
      <span className={cn(CHIP, "border-primary-border bg-primary-muted text-primary")}>
        {words.attribute}
      </span>
      <span className="text-sm text-secondary-foreground">{words.operator}</span>
      {words.value === null ? (
        <code className="bg-transparent text-muted-foreground">{node.written}</code>
      ) : (
        <code className="border border-border">{words.value}</code>
      )}
    </div>
  );
}

function ConditionGroup({ node }: { node: Extract<ConditionNode, { type: "group" }> }) {
  const { word, hint } = GROUPS[node.kind];

  return (
    <div className="grid min-w-0 flex-1 gap-1.5 border-l-2 border-primary-border py-0.5">
      <div className="flex items-center gap-2 pl-3">
        <span className={cn(CHIP, "border-border bg-muted")}>{word}</span>
        <span className="text-sm text-muted-foreground">{hint}</span>
      </div>
      <ConditionList nodes={node.items} join={node.kind === "all" ? "and" : "or"} first="" />
    </div>
  );
}

/** Conditions one a line, with the word that joins each one to the one before, as the condition builder. */
function ConditionList({ nodes, join, first }: { nodes: ConditionNode[]; join: string; first: string }) {
  return (
    <ul className="grid gap-1.5">
      {nodes.map((node, index) => (
        <li key={index} className="flex items-start gap-2.5">
          <span className="w-12 flex-none pt-0.5 text-right text-sm text-muted-foreground">
            {/* An "and not" group says its own "and". */}
            {index === 0
              ? first
              : node.type === "group" && node.kind === "none" && join === "and"
                ? ""
                : join}
          </span>
          {node.type === "group" ? <ConditionGroup node={node} /> : <ConditionRow node={node} />}
        </li>
      ))}
    </ul>
  );
}

/** What keeps a PR out of the queue, one reason a line; the unmet queue conditions as the condition builder. */
export function BlockReasons({ reasons, className }: { reasons: Pr["block_reasons"]; className?: string }) {
  return (
    <ul className={cn("grid gap-2.5", className)}>
      {reasons.map((reason, index) => {
        const sentence = blockSentence(reason);
        const conditions = Array.isArray(reason.conditions) ? reason.conditions.map(String) : [];

        return (
          <li key={index}>
            {sentence === null ? (
              <>
                It does not meet {conditions.length > 1 ? "these queue conditions" : "this queue condition"}:
                <div className="mt-2" role="group" aria-label="Unmet queue conditions">
                  <ConditionList nodes={conditions.map(conditionTree)} join="and" first="where" />
                </div>
              </>
            ) : (
              <Prose text={sentence} />
            )}
          </li>
        );
      })}
    </ul>
  );
}

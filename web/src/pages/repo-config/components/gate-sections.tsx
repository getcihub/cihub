import { Link } from "@tanstack/react-router";
import type { ReactNode } from "react";

import type { ConditionDocument } from "@/client/types.gen";
import { Segmented } from "@/components/ui/segmented";
import { Switch } from "@/components/ui/switch";
import { TokenField } from "@/components/ui/token-field";
import { DEFAULTS_ENTRY } from "@/domain/config/queries";
import { INJECTIONS, type Owner } from "@/domain/config/words";
import type { Editor } from "@/pages/repo-config/setting-editor";
import { ConditionEditor } from "./condition-editor";
import { ChoiceField, NumberField } from "./controls";
import { Names, Setting, SettingRow, SettingsSection, SourceTag } from "./setting-parts";

/** The conditions of the part of the document that the builder shows, and those that it takes from `defaults`. */
export interface Gate {
  conditions: ConditionDocument[];
  checks: string[];
  /** The same, in the file. */
  baseConditions: ConditionDocument[];
  baseChecks: string[];
  /** Those of `defaults`, for a queue; none for `defaults` itself. */
  inheritedConditions: ConditionDocument[];
  inheritedChecks: string[];
  /** The YAML of the conditions, as the file holds them, or as the server writes the draft. */
  yaml: string[];
  setConditions: (conditions: ConditionDocument[]) => void;
  setChecks: (checks: string[]) => void;
}

type RequiredMode = "inherit" | "list" | "none";

/** The tag of a list of the file: set by the part, taken from `defaults`, or none. */
function listTag(owner: Owner, own: unknown[], inherited: unknown[], isChanged: boolean): ReactNode {
  return (
    <SourceTag
      source={own.length > 0 ? owner : inherited.length > 0 ? "defaults" : "default"}
      owner={owner}
      isChanged={isChanged}
    />
  );
}

/** What a pull request needs to enter the queue and to merge, and how long it waits. */
export function GateSections({
  org,
  repo,
  editor,
  gate,
  branch,
  ruleChecks,
}: {
  org: string;
  repo: string;
  editor: Editor;
  gate: Gate;
  /** The branch of the file, whose rules the server read. */
  branch: string;
  ruleChecks: string[] | null;
}) {
  const { resolved, canEdit, set } = editor;
  const isQueue = editor.owner === "queue";
  const required = resolved.required_statuses ?? null;
  const mode: RequiredMode = required === null ? "inherit" : required.length === 0 ? "none" : "list";
  const checkSuggestions = (ruleChecks ?? []).map((name) => ({
    value: name,
    meta: "required by the branch rules",
  }));
  const sameJson = (first: unknown, second: unknown) => JSON.stringify(first) === JSON.stringify(second);

  return (
    <>
      <SettingsSection
        id="cfg-conditions"
        title="Conditions"
        text={
          isQueue
            ? "Which pull requests enter the queue, and which checks the tested commit must pass."
            : "What every queue of the file checks first, before its own conditions."
        }
      >
        <SettingRow
          title="To enter the queue"
          tag={listTag(
            editor.owner,
            gate.conditions,
            gate.inheritedConditions,
            !sameJson(gate.conditions, gate.baseConditions),
          )}
          text="A pull request waits in “Waiting” until it meets every condition. The rules of the branch apply too."
          isColumn
        >
          {gate.inheritedConditions.length > 0 && (
            <div className="grid gap-1.5 rounded-sm border border-dashed border-border py-2.5 pr-3">
              <div className="flex items-center justify-between pl-3 text-sm text-muted-foreground">
                <span>From defaults, checked first</span>
                <Link
                  to="/orgs/$org/repos/$repo/config"
                  params={{ org, repo }}
                  search={(search) => ({ ...search, entry: DEFAULTS_ENTRY })}
                  resetScroll={false}
                  className="text-sm text-primary hover:underline"
                >
                  {canEdit ? "Edit in defaults" : "Open defaults"}
                </Link>
              </div>
              <ConditionEditor
                conditions={gate.inheritedConditions}
                canEdit={false}
                empty=""
                onChange={() => undefined}
              />
            </div>
          )}
          <ConditionEditor
            conditions={gate.conditions}
            canEdit={canEdit}
            empty={
              gate.inheritedConditions.length > 0
                ? "No condition of its own: the defaults above only."
                : "No condition: the rules of the branch only."
            }
            onChange={gate.setConditions}
          />
          {gate.yaml.length > 0 && (
            <pre
              aria-label="The conditions in the file"
              className="mt-1 overflow-x-auto rounded-sm border border-border bg-muted px-3 py-2 font-mono text-sm text-secondary-foreground"
            >
              {gate.yaml.join("\n")}
            </pre>
          )}
        </SettingRow>
        <SettingRow
          title="To merge"
          tag={listTag(
            editor.owner,
            gate.checks,
            gate.inheritedChecks,
            !sameJson(gate.checks, gate.baseChecks),
          )}
          text="The checks that must succeed on the tested commit, on top of the required checks below."
          isColumn
        >
          {gate.inheritedChecks.length > 0 && (
            <div className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
              From defaults: <Names names={gate.inheritedChecks} />
            </div>
          )}
          <div className="flex items-start gap-2.5">
            <span className="w-11 flex-none pt-1.25 text-right text-md text-muted-foreground">where</span>
            <div className="flex min-w-0 flex-1 flex-wrap items-center gap-1.5">
              <span className="inline-flex h-7 items-center rounded-sm border border-primary-border bg-primary-muted px-2.5 text-base font-medium text-primary">
                Checks
              </span>
              <span className="text-base text-secondary-foreground">succeeded</span>
              <TokenField
                label="Checks to merge"
                values={gate.checks}
                placeholder="Add a check"
                readOnly={!canEdit}
                suggestions={checkSuggestions}
                note="From the required checks of the branch rules. Type another check, then Enter."
                onChange={gate.setChecks}
              />
            </div>
          </div>
        </SettingRow>
      </SettingsSection>

      <SettingsSection
        id="cfg-checks"
        title="Required checks"
        text="The checks that a tested commit needs, and where the checks of the branch rules count."
      >
        <Setting
          editor={editor}
          row={{ keys: ["required_statuses"], source: "required_statuses" }}
          title="Required on the tested commit"
          text={
            mode === "inherit" ? (
              <RuleChecks branch={branch} checks={ruleChecks} />
            ) : mode === "none" ? (
              "No check is required on the tested commit."
            ) : (
              "Only these checks: the branch rules do not add theirs."
            )
          }
          isColumn
        >
          <div className="flex flex-wrap items-center gap-2">
            <Segmented<RequiredMode>
              label="Required checks"
              disabled={!canEdit}
              value={mode}
              options={[
                { value: "inherit", label: "From the branch rules" },
                { value: "list", label: "These checks" },
                { value: "none", label: "None" },
              ]}
              onChange={(next) => {
                set({
                  required_statuses:
                    next === "inherit"
                      ? { kind: "branch_rules" }
                      : { kind: "checks", names: next === "list" ? (ruleChecks ?? []) : [] },
                });
              }}
            />
            {mode === "list" && (
              <TokenField
                label="Required checks"
                values={required ?? []}
                placeholder="Add a check"
                readOnly={!canEdit}
                isIncomplete={(required ?? []).length === 0}
                suggestions={checkSuggestions}
                onChange={(names) => {
                  set({ required_statuses: { kind: "checks", names } });
                }}
              />
            )}
          </div>
        </Setting>
        <Setting
          editor={editor}
          row={{ keys: ["branch_protection_injection"], source: "branch_protection_injection" }}
          title="Checks of the branch rules"
          text="Where the checks that the branch rules require count."
        >
          <ChoiceField
            label="Checks of the branch rules"
            disabled={!canEdit}
            value={resolved.branch_protection_injection}
            options={INJECTIONS}
            onChange={(injection) => {
              set({ branch_protection_injection: injection });
            }}
          />
        </Setting>
      </SettingsSection>

      <SettingsSection
        id="cfg-waiting"
        title="Waiting"
        text="What happens to a pull request that waits, or that changes while it is in the queue."
      >
        <Setting
          editor={editor}
          row={{ keys: ["not_ready_timeout_hours"], source: "not_ready_timeout_hours" }}
          title="Time limit in “Waiting”"
          text="A pull request that waits for its conditions longer than this leaves the queue."
        >
          <NumberField
            label="Time limit in Waiting"
            unit="hours"
            disabled={!canEdit}
            value={resolved.not_ready_timeout_hours}
            onCommit={(value) => {
              set({ not_ready_timeout_hours: value });
            }}
          />
        </Setting>
        <Setting
          editor={editor}
          row={{ keys: ["resubmit_on_push"], source: "resubmit_on_push" }}
          title="Submit again on a push"
          text="A push on a queued pull request submits it again at the end of the queue."
        >
          <Switch
            aria-label="Submit again on a push"
            checked={resolved.resubmit_on_push}
            disabled={!canEdit}
            onClick={() => {
              set({ resubmit_on_push: !resolved.resubmit_on_push });
            }}
          />
        </Setting>
      </SettingsSection>
    </>
  );
}

/** The checks that the branch rules require today, as the last read of the rules gives them. */
function RuleChecks({ branch, checks }: { branch: string; checks: string[] | null }) {
  if (checks === null) {
    return (
      <>
        The required checks of the branch rules; CIHub has not read the rules of <code>{branch}</code> yet.
      </>
    );
  }

  if (checks.length === 0) {
    return (
      <>
        The required checks of the branch rules: <code>{branch}</code> requires none today.
      </>
    );
  }

  return (
    <>
      The required checks of the branch rules: today <Names names={checks} />.
    </>
  );
}

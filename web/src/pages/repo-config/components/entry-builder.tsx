import type { BranchConfig, SettingsDocument } from "@/client/types.gen";
import { Button } from "@/components/ui/button";
import {
  type Item,
  isGlob,
  itemOf,
  resolvedOf,
  settingsIn,
  settingsOf,
  withOwnSettings,
} from "@/domain/config/document";
import type { Editor } from "@/pages/repo-config/setting-editor";
import { yamlBlockOf } from "@/domain/config/yaml";
import type { ConfigDraft } from "@/pages/repo-config/use-config-draft";
import { type Gate, GateSections } from "./gate-sections";
import { MergeSections } from "./merge-sections";
import { PriorityRules } from "./priority-rules";
import { Profiles } from "./profiles";
import { SettingsSection } from "./setting-parts";
import { TrainSections } from "./train-sections";

/**
 * The builder of an entry of the document: `defaults`, or the queue of an entry. Each setting shows its value and
 * its source, in the sections of the mockups v26; a person who can propose changes them in the draft.
 */
export function EntryBuilder({
  org,
  repo,
  config,
  draft,
  entry,
  canEdit,
  onEntryRemoved,
}: {
  org: string;
  repo: string;
  config: BranchConfig;
  draft: ConfigDraft;
  entry: string;
  canEdit: boolean;
  onEntryRemoved: () => void;
}) {
  const { document, settings } = draft;
  const item: Item = itemOf(document, entry) ?? { kind: "defaults" };
  const baseItem = itemOf(draft.base, entry);
  if (!settings) {
    return <p className="text-md text-muted-foreground">CIHub writes the file…</p>;
  }

  const editor: Editor = {
    canEdit,
    owner: item.kind,
    resolved: withOwnSettings(resolvedOf(settings, item), settingsOf(document, item), item.kind),
    own: settingsOf(document, item),
    base: baseItem ? settingsOf(draft.base, baseItem) : {},
    set: (values: SettingsDocument) => {
      draft.change((copy) => {
        Object.assign(settingsIn(copy, item), values);
      });
    },
  };
  const queue = item.kind === "queue" ? document.queues[item.index] : undefined;
  const baseQueue = baseItem?.kind === "queue" ? draft.base.queues[baseItem.index] : undefined;
  const baseDefaults = draft.base.defaults;
  // The YAML that the server wrote for the draft, else the file as it is.
  const text = draft.preview?.text ?? config.text ?? "";
  const gate: Gate = {
    conditions: queue?.queue_conditions ?? document.defaults.queue_conditions,
    checks: queue?.merge_checks ?? document.defaults.merge_checks,
    baseConditions:
      item.kind === "queue" ? (baseQueue?.queue_conditions ?? []) : baseDefaults.queue_conditions,
    baseChecks: item.kind === "queue" ? (baseQueue?.merge_checks ?? []) : baseDefaults.merge_checks,
    yaml: yamlBlockOf(text, queue ? queue.branch : null, "queue_conditions"),
    inheritedConditions: queue ? document.defaults.queue_conditions : [],
    inheritedChecks: queue ? document.defaults.merge_checks : [],
    setConditions: (conditions) => {
      draft.change((copy) => {
        const target = item.kind === "queue" ? copy.queues[item.index] : copy.defaults;
        if (target) {
          target.queue_conditions = conditions;
        }
      });
    },
    setChecks: (checks) => {
      draft.change((copy) => {
        const target = item.kind === "queue" ? copy.queues[item.index] : copy.defaults;
        if (target) {
          target.merge_checks = checks;
        }
      });
    },
  };

  return (
    <div className="grid gap-6">
      {item.kind === "queue" && queue ? (
        <QueueHead
          branch={config.branch}
          entry={queue.branch}
          globs={document.queues.map((other) => other.branch).filter(isGlob)}
          canEdit={canEdit}
          onMove={(step) => {
            draft.change((copy) => {
              const [moved] = copy.queues.splice(item.index, 1);
              if (!moved) {
                return;
              }
              // A glob moves past the next glob: the order of the globs is the one that counts.
              let at = item.index + step;
              while (at > 0 && at < copy.queues.length && !isGlob(copy.queues[at]?.branch ?? "")) {
                at += step;
              }
              copy.queues.splice(Math.max(0, Math.min(at, copy.queues.length)), 0, moved);
            });
          }}
          onRemove={() => {
            draft.change((copy) => {
              copy.queues.splice(item.index, 1);
            });
            onEntryRemoved();
          }}
        />
      ) : (
        <div>
          <h2 className="text-2xl font-medium">Defaults</h2>
          <p className="mt-1 text-md text-secondary-foreground">
            Every queue of this file takes these values, unless it sets its own. A queue checks the conditions
            of the defaults first, then its own. The priority rules and the profiles are not defaults: they
            belong to the file and to each queue.
          </p>
        </div>
      )}
      <TrainSections
        editor={editor}
        defaultStrategy={settings.defaults.sources.strategy === "default" ? settings.defaults.strategy : null}
      />
      <GateSections
        org={org}
        repo={repo}
        editor={editor}
        gate={gate}
        branch={config.branch}
        ruleChecks={config.required_checks ?? null}
      />
      {item.kind === "queue" && queue && (
        <>
          <SettingsSection
            id="cfg-priority"
            title="Priority"
            text="The order of the pull requests in the queue. These rules apply to every queue of this file."
          >
            <PriorityRules
              rules={document.priority_rules}
              canEdit={canEdit}
              onChange={(rules) => {
                draft.change((copy) => {
                  copy.priority_rules = rules;
                });
              }}
            />
          </SettingsSection>
          <SettingsSection
            id="cfg-profiles"
            title="Profiles"
            text="Settings for a kind of pull request in the same train. A profile is not a second queue: its pull requests test on top of the others."
          >
            <Profiles
              profiles={queue.profiles}
              queue={editor.resolved}
              canEdit={canEdit}
              onChange={(profiles) => {
                draft.change((copy) => {
                  const target = copy.queues[item.index];
                  if (target) {
                    target.profiles = profiles;
                  }
                });
              }}
            />
          </SettingsSection>
        </>
      )}
      <MergeSections editor={editor} />
    </div>
  );
}

/** What an entry covers, and what an admin does with it: move a glob, remove the entry. */
function QueueHead({
  branch,
  entry,
  globs,
  canEdit,
  onMove,
  onRemove,
}: {
  branch: string;
  entry: string;
  globs: string[];
  canEdit: boolean;
  onMove: (step: 1 | -1) => void;
  onRemove: () => void;
}) {
  const glob = globs.indexOf(entry);

  return (
    <div className="flex flex-wrap items-start gap-3">
      <div className="min-w-0 flex-1">
        <h2 className="text-2xl font-medium">
          Queue of <code>{entry}</code>
        </h2>
        <p className="mt-1 text-md text-secondary-foreground">
          {isGlob(entry) ? (
            "One queue for each branch that matches, each with these settings."
          ) : (
            <>
              Every pull request to <code>{entry}</code> enters this queue.
            </>
          )}
          {entry !== branch && (
            <>
              {" "}
              CIHub reads the file at the head of each branch: this entry acts on a branch once the file is on
              it, as on <code>{branch}</code>.
            </>
          )}
          {glob >= 0 && globs.length > 1 && (
            <>
              {" "}
              Glob {glob + 1} of {globs.length}: a branch takes the first glob that matches it.
            </>
          )}
        </p>
      </div>
      {canEdit && (
        <div className="flex gap-2">
          {glob >= 0 && globs.length > 1 && (
            <>
              <Button
                disabled={glob === 0}
                onClick={() => {
                  onMove(-1);
                }}
              >
                Move up
              </Button>
              <Button
                disabled={glob === globs.length - 1}
                onClick={() => {
                  onMove(1);
                }}
              >
                Move down
              </Button>
            </>
          )}
          {entry !== branch && (
            <Button variant="destructive-outline" onClick={onRemove}>
              Remove the queue
            </Button>
          )}
        </div>
      )}
    </div>
  );
}

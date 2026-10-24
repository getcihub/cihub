import type { ReactNode } from "react";

import type { ConfigSettings, ProfileDocument, SettingsDocument } from "@/client/types.gen";
import { PlusIcon } from "@/components/icons/plus-icon";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { MERGE_TITLES, type ValueWords, mergeMethods, wordsOf } from "@/domain/config/words";
import { ConditionEditor } from "./condition-editor";
import { ChoiceField, InheritedNumberField, TextField } from "./controls";

/** The profile that the builder offers to start from: the updates of the dependency bots. */
const TEMPLATE: ProfileDocument = {
  name: "dependencies",
  conditions: [
    { kind: "condition", attribute: "author", operator: null, values: ["dependabot[bot]", "renovate[bot]"] },
  ],
  settings: { merge_method: "merge_commit", batching_enabled: true, batching_size: 10 },
};

/** The options of a setting of a profile: first the value of the queue, which an unset key takes. */
function withQueue<T extends string>(words: ValueWords<T>[], queue: T): ValueWords<T | "">[] {
  return [{ value: "", label: `from the queue: ${wordsOf(words, queue)}` }, ...words];
}

/** The profiles of a queue, in the order of the file: settings for a kind of pull request in the same train. */
export function Profiles({
  profiles,
  queue,
  canEdit,
  onChange,
}: {
  profiles: ProfileDocument[];
  /** The settings of the queue, which a profile takes unless it sets its own. */
  queue: ConfigSettings;
  canEdit: boolean;
  onChange: (profiles: ProfileDocument[]) => void;
}) {
  const put = (index: number, profile: ProfileDocument | null) => {
    onChange(
      profile === null
        ? profiles.filter((_, other) => other !== index)
        : profiles.map((other, at) => (at === index ? profile : other)),
    );
  };

  if (profiles.length === 0) {
    return (
      <Card className="grid justify-items-start gap-2 p-4">
        <b className="font-medium">No profile: every pull request takes the settings of the queue</b>
        <p className="max-w-160 text-base text-secondary-foreground">
          A profile gives other settings to a kind of pull request, in the same train: the dependency updates
          merged with a merge commit and in large batches, while the work of the team is squashed.
        </p>
        <button
          type="button"
          disabled={!canEdit}
          className="my-1 flex w-full cursor-pointer items-center justify-between gap-3 rounded-sm border border-dashed border-border px-3 py-2.5 text-left text-base enabled:hover:border-solid enabled:hover:border-primary-border enabled:hover:bg-primary-muted disabled:cursor-default"
          onClick={() => {
            onChange([structuredClone(TEMPLATE)]);
          }}
        >
          <span>The updates of the dependency bots: a merge commit, in batches of 10.</span>
          <span className="rounded-xs border border-border px-1.5 text-sm text-secondary-foreground">
            dependencies
          </span>
        </button>
        {canEdit && (
          <Button
            className="[&>svg]:size-3"
            onClick={() => {
              onChange([{ name: "profile", conditions: [], settings: {} }]);
            }}
          >
            <PlusIcon />
            Start from an empty profile
          </Button>
        )}
      </Card>
    );
  }

  return (
    <>
      <p className="text-base text-secondary-foreground">
        The command chooses a profile by its name, <code>/cihub merge --profile=&lt;name&gt;</code>; else the
        first profile whose conditions the pull request meets. Write conditions on what does not change: the
        author, the head branch, the files. A setting that a profile does not set is the one of the queue.
      </p>
      <ol className="grid gap-2" aria-label="Profiles">
        {profiles.map((profile, index) => (
          <li key={index}>
            <ProfileCard
              profile={profile}
              number={index + 1}
              queue={queue}
              canEdit={canEdit}
              onChange={(next) => {
                put(index, next);
              }}
            />
          </li>
        ))}
      </ol>
      {canEdit && (
        <div>
          <Button
            className="[&>svg]:size-3"
            onClick={() => {
              onChange([
                ...profiles,
                { name: `profile-${String(profiles.length + 1)}`, conditions: [], settings: {} },
              ]);
            }}
          >
            <PlusIcon />
            Add a profile
          </Button>
        </div>
      )}
    </>
  );
}

function ProfileCard({
  profile,
  number,
  queue,
  canEdit,
  onChange,
}: {
  profile: ProfileDocument;
  number: number;
  queue: ConfigSettings;
  canEdit: boolean;
  onChange: (profile: ProfileDocument | null) => void;
}) {
  const own = profile.settings;
  const set = (values: SettingsDocument) => {
    onChange({ ...profile, settings: { ...own, ...values } });
  };
  const isBatching = own.batching_enabled ?? queue.batching_enabled;

  return (
    <Card className="p-0">
      <div className="flex flex-wrap items-center gap-2.5 border-b border-divider px-4 py-2.5">
        <span className="mr-1 inline-grid size-4.5 place-items-center rounded-full border border-border bg-muted text-2xs">
          {number}
        </span>
        {canEdit ? (
          <TextField
            label="Name of the profile"
            className="w-44 font-mono"
            disabled={false}
            value={profile.name}
            onCommit={(name) => {
              onChange({ ...profile, name });
            }}
          />
        ) : (
          <b className="font-medium">{profile.name}</b>
        )}
        <code className="border-0 bg-transparent p-0 text-sm text-muted-foreground">
          /cihub merge --profile={profile.name}
        </code>
        {canEdit && (
          <button
            type="button"
            className="ml-auto cursor-pointer rounded-sm px-2 py-1 text-base text-destructive hover:bg-destructive-muted"
            onClick={() => {
              onChange(null);
            }}
          >
            Remove
          </button>
        )}
      </div>
      <div className="grid gap-2 py-3 pr-4 pl-1.5">
        <span className="pl-2.5 text-md text-muted-foreground">
          chosen for a pull request that meets every condition:
        </span>
        <ConditionEditor
          conditions={profile.conditions}
          canEdit={canEdit}
          empty="No condition: only a command that names the profile chooses it."
          onChange={(conditions) => {
            onChange({ ...profile, conditions });
          }}
        />
        <div className="mt-1.5 ml-2.5 grid grid-cols-[repeat(auto-fill,minmax(260px,1fr))] gap-x-4 gap-y-2 border-t border-divider pt-2.5">
          <ProfileSetting title="Merge method">
            <ChoiceField
              label="Merge method of the profile"
              disabled={!canEdit}
              value={own.merge_method ?? ""}
              options={withQueue(mergeMethods(false, false), queue.merge_method)}
              onChange={(method) => {
                set({ merge_method: method === "" ? null : method });
              }}
            />
          </ProfileSetting>
          <ProfileSetting title="Title of the merge commit">
            <ChoiceField
              label="Title of the merge commit of the profile"
              disabled={!canEdit}
              value={own.merge_commit_title ?? ""}
              options={withQueue(MERGE_TITLES, queue.merge_commit_title)}
              onChange={(title) => {
                set({ merge_commit_title: title === "" ? null : title });
              }}
            />
          </ProfileSetting>
          <ProfileSetting title="Batches">
            <ChoiceField
              label="Batches of the profile"
              disabled={!canEdit}
              value={own.batching_enabled == null ? "" : own.batching_enabled ? "on" : "off"}
              options={withQueue(
                [
                  { value: "on", label: "on" },
                  { value: "off", label: "off" },
                ],
                queue.batching_enabled ? "on" : "off",
              )}
              onChange={(value) => {
                set({ batching_enabled: value === "" ? null : value === "on" });
              }}
            />
          </ProfileSetting>
          {isBatching && (
            <>
              <ProfileSetting title="Batch size">
                <InheritedNumberField
                  label="Batch size of the profile"
                  unit="PRs"
                  disabled={!canEdit}
                  value={own.batching_size ?? null}
                  inherited={queue.batching_size}
                  onCommit={(value) => {
                    set({ batching_size: value });
                  }}
                />
              </ProfileSetting>
              <ProfileSetting title="Batch wait">
                <InheritedNumberField
                  label="Batch wait of the profile"
                  unit="min"
                  disabled={!canEdit}
                  value={own.batching_max_wait_minutes ?? null}
                  inherited={queue.batching_max_wait_minutes}
                  onCommit={(value) => {
                    set({ batching_max_wait_minutes: value });
                  }}
                />
              </ProfileSetting>
            </>
          )}
          <ProfileSetting title="Retries">
            <InheritedNumberField
              label="Retries of the profile"
              unit="retries"
              disabled={!canEdit}
              value={own.failure_budget_retries ?? null}
              inherited={queue.failure_budget_retries}
              onCommit={(value) => {
                set({ failure_budget_retries: value });
              }}
            />
          </ProfileSetting>
        </div>
        <p className="ml-2.5 text-sm text-muted-foreground">
          An empty field takes the value of the queue, shown in grey.
        </p>
      </div>
    </Card>
  );
}

/** A setting that a profile can set, and its field. */
function ProfileSetting({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-2.5 text-base text-secondary-foreground">
      <span>{title}</span>
      {children}
    </div>
  );
}

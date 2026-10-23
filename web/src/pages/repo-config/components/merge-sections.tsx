import { Switch } from "@/components/ui/switch";
import { MERGE_TITLES, mergeMethods } from "@/domain/config/words";
import type { Editor } from "@/pages/repo-config/setting-editor";
import { ChoiceField, NumberField, TextField } from "./controls";
import { Setting, SettingsSection } from "./setting-parts";

/** How a pull request lands, what CIHub writes on it, and its test branches. */
export function MergeSections({ editor }: { editor: Editor }) {
  const { resolved, canEdit, set } = editor;
  const isSequential = resolved.strategy === "sequential";
  const isLabelOn = resolved.label_commands_enabled;
  const prefix = resolved.branch_prefix;

  return (
    <>
      <SettingsSection
        id="cfg-merging"
        title="Merging"
        text="How a pull request lands on the branch once its run passes."
      >
        <Setting
          editor={editor}
          row={{ keys: ["merge_method"], source: "merge_method" }}
          title="Merge method"
          text={
            isSequential
              ? "Fast-forward keeps the tested commit as is."
              : "Fast-forward needs the sequential strategy."
          }
        >
          <ChoiceField
            label="Merge method"
            disabled={!canEdit}
            value={resolved.merge_method}
            options={mergeMethods(isSequential)}
            onChange={(method) => {
              set({ merge_method: method });
            }}
          />
        </Setting>
        <Setting
          editor={editor}
          row={{ keys: ["merge_commit_title"], source: "merge_commit_title" }}
          title="Title of the merge commit"
          text="The title that CIHub gives to the commit that lands."
        >
          <ChoiceField
            label="Title of the merge commit"
            disabled={!canEdit}
            value={resolved.merge_commit_title}
            options={MERGE_TITLES}
            onChange={(title) => {
              set({ merge_commit_title: title });
            }}
          />
        </Setting>
        <Setting
          editor={editor}
          row={{ keys: ["direct_merge_mode"], source: "direct_merge_mode" }}
          title="Direct merge"
          text="With no test in flight, CIHub merges the first pull request without a test branch when it holds the head of the branch and its checks are green."
        >
          <Switch
            aria-label="Direct merge"
            checked={resolved.direct_merge_mode === "always"}
            disabled={!canEdit}
            onClick={() => {
              set({ direct_merge_mode: resolved.direct_merge_mode === "always" ? "off" : "always" });
            }}
          />
        </Setting>
      </SettingsSection>

      <SettingsSection
        id="cfg-comments"
        title="On the pull requests"
        text="What CIHub writes and reads on a pull request."
      >
        {(
          [
            [
              "comments_enabled",
              "Status comment",
              "One comment per pull request, edited at each step: where it is and why it waits.",
            ],
            ["commands_enabled", "Commands", "CIHub reads the /cihub commands in the comments."],
            [
              "status_check_enabled",
              "Status checks",
              "CIHub reports its two statuses on the pull request: the one of the queue and the one of the conditions.",
            ],
            [
              "state_labels_enabled",
              "State labels",
              "The labels cihub:queued, cihub:testing and cihub:failed.",
            ],
          ] as const
        ).map(([key, title, text]) => (
          <Setting key={key} editor={editor} row={{ keys: [key], source: key }} title={title} text={text}>
            <Switch
              aria-label={title}
              checked={resolved[key]}
              disabled={!canEdit}
              onClick={() => {
                set({ [key]: !resolved[key] });
              }}
            />
          </Setting>
        ))}
        <Setting
          editor={editor}
          row={{
            keys: ["label_commands_enabled", "enqueue_label"],
            source: isLabelOn ? "enqueue_label" : "label_commands_enabled",
          }}
          title="Submit label"
          text={
            isLabelOn
              ? "Adding this label to a pull request submits it, like the command."
              : "Off: a label does not submit a pull request."
          }
        >
          {isLabelOn && (
            <TextField
              label="Submit label"
              disabled={!canEdit}
              value={resolved.enqueue_label}
              onCommit={(label) => {
                set({ enqueue_label: label });
              }}
            />
          )}
          <Switch
            aria-label="Labels submit"
            checked={isLabelOn}
            disabled={!canEdit}
            onClick={() => {
              set({ label_commands_enabled: !isLabelOn });
            }}
          />
        </Setting>
      </SettingsSection>

      <SettingsSection
        id="cfg-test-branches"
        title="Test branches"
        text="The branches where CIHub tests the pull requests."
      >
        <Setting
          editor={editor}
          row={{ keys: ["branch_prefix"], source: "branch_prefix" }}
          title="Prefix"
          text={
            <>
              The test branches are <code>{prefix}-merge/…</code> and <code>{prefix}-temp/…</code>. The branch
              rules must exclude them.
            </>
          }
        >
          <TextField
            label="Prefix"
            className="w-32"
            disabled={!canEdit}
            value={prefix}
            onCommit={(value) => {
              set({ branch_prefix: value });
            }}
          />
        </Setting>
        <Setting
          editor={editor}
          row={{ keys: ["keep_test_branches_minutes"], source: "keep_test_branches_minutes" }}
          title="Keep a test branch"
          text="How long a test branch stays after its test."
        >
          <NumberField
            label="Keep a test branch"
            unit="min"
            disabled={!canEdit}
            value={resolved.keep_test_branches_minutes}
            onCommit={(value) => {
              set({ keep_test_branches_minutes: value });
            }}
          />
        </Setting>
      </SettingsSection>
    </>
  );
}

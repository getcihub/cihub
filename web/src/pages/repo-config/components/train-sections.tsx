import { Switch } from "@/components/ui/switch";
import type { Strategy } from "@/client/types.gen";
import { MODES, STRATEGIES, TRIGGERS, WINDOW_KINDS } from "@/domain/config/words";
import { cn } from "@/lib/cn";
import type { Editor } from "@/pages/repo-config/setting-editor";
import { ChoiceField, NumberField } from "./controls";
import { Setting, SettingsSection } from "./setting-parts";
import { StrategyArt } from "./strategy-art";

/** The runs in flight that the window lets, drawn, up to 12. */
function MiniRuns({ ceiling }: { ceiling: number }) {
  const drawn = Math.min(ceiling, 12);
  return (
    <span className="flex items-center gap-0.75" aria-hidden="true">
      {Array.from({ length: drawn }, (_, index) => (
        <i key={index} className="h-4 w-1.75 rounded-xs bg-primary" style={{ opacity: 1 - index * 0.06 }} />
      ))}
      {ceiling > drawn && (
        <em className="ml-1 text-xs text-muted-foreground not-italic">+{ceiling - drawn}</em>
      )}
    </span>
  );
}

/**
 * How the queue tests: its strategy, its window, its batches and what it does after a failure. `defaultStrategy`
 * is the strategy of CIHub, when the server gives it.
 */
export function TrainSections({
  editor,
  defaultStrategy,
}: {
  editor: Editor;
  defaultStrategy: Strategy | null;
}) {
  const { resolved, canEdit, set } = editor;
  const isFixed = editor.own.testing_concurrency != null;
  const floor = resolved.window_floor;
  const ceiling = resolved.window_ceiling;

  return (
    <>
      <SettingsSection
        id="cfg-strategy"
        title="Strategy"
        text="How the queue tests the pull requests. The strategy sets the defaults of the other settings."
      >
        <Setting
          editor={editor}
          row={{ keys: ["strategy"], source: "strategy" }}
          title="Strategy"
          text="The strategy of the queue."
          isColumn
        >
          <div className="grid grid-cols-2 gap-2 lg:grid-cols-4" role="radiogroup" aria-label="Strategies">
            {STRATEGIES.map((strategy) => {
              const isChosen = strategy.value === resolved.strategy;
              return (
                <button
                  key={strategy.value}
                  type="button"
                  role="radio"
                  aria-checked={isChosen}
                  disabled={!canEdit || (!strategy.isReady && !isChosen)}
                  onClick={() => {
                    set({ strategy: strategy.value });
                  }}
                  className={cn(
                    "relative grid cursor-pointer content-start gap-1 rounded-md border bg-card p-3 text-left disabled:cursor-default",
                    isChosen ? "border-primary shadow-chosen" : "border-border",
                    !strategy.isReady && "opacity-55",
                  )}
                >
                  <b className="font-medium">{strategy.title}</b>
                  <span className="text-sm text-secondary-foreground">{strategy.text}</span>
                  <StrategyArt strategy={strategy.value} isChosen={isChosen} />
                  {(strategy.value === defaultStrategy || !strategy.isReady) && (
                    <em className="absolute top-2.5 right-2.5 text-2xs text-muted-foreground not-italic">
                      {strategy.isReady ? "default" : "not ready yet"}
                    </em>
                  )}
                </button>
              );
            })}
          </div>
        </Setting>
        <Setting
          editor={editor}
          row={{ keys: ["mode"], source: "mode" }}
          title="Lanes"
          text="One line, or independent lines for unrelated changes. A change with pull requests in flight waits until the last one ends."
        >
          <ChoiceField
            label="Lanes"
            disabled={!canEdit}
            value={resolved.mode}
            options={MODES}
            onChange={(mode) => {
              set({ mode });
            }}
          />
        </Setting>
      </SettingsSection>

      <SettingsSection
        id="cfg-testing"
        title="Testing"
        text="How much CI the queue uses at once, and how it starts a test."
      >
        <Setting
          editor={editor}
          row={{ keys: ["window_floor", "window_ceiling", "testing_concurrency"], source: "window_ceiling" }}
          title="Runs at a time"
          text={
            isFixed
              ? "A fixed window: this many runs test at the same time."
              : "The window: CIHub starts with the ceiling and lowers it after failures, never below the floor."
          }
        >
          <ChoiceField
            label="Kind of window"
            disabled={!canEdit}
            value={isFixed ? "fixed" : "adaptive"}
            options={WINDOW_KINDS}
            onChange={(kind) => {
              set(
                kind === "fixed"
                  ? { testing_concurrency: ceiling, window_floor: null, window_ceiling: null }
                  : { testing_concurrency: null, window_floor: floor, window_ceiling: ceiling },
              );
            }}
          />
          {isFixed ? (
            <NumberField
              label="Runs at a time"
              unit="runs"
              disabled={!canEdit}
              value={ceiling}
              onCommit={(value) => {
                set({ testing_concurrency: value });
              }}
            />
          ) : (
            <>
              <NumberField
                label="Floor of the window"
                unit="to"
                disabled={!canEdit}
                value={floor}
                onCommit={(value) => {
                  set({ window_floor: value });
                }}
              />
              <NumberField
                label="Ceiling of the window"
                unit="runs"
                disabled={!canEdit}
                value={ceiling}
                onCommit={(value) => {
                  set({ window_ceiling: value });
                }}
              />
            </>
          )}
          <MiniRuns ceiling={ceiling} />
        </Setting>
        <Setting
          editor={editor}
          row={{ keys: ["trigger"], source: "trigger" }}
          title="CI trigger"
          text="What runs your CI on a test commit."
        >
          <ChoiceField
            label="CI trigger"
            disabled={!canEdit}
            value={resolved.trigger}
            options={TRIGGERS}
            onChange={(trigger) => {
              set({ trigger });
            }}
          />
        </Setting>
        <Setting
          editor={editor}
          row={{ keys: ["testing_timeout_minutes"], source: "testing_timeout_minutes" }}
          title="Time limit of a test"
          text="A test without a conclusion after this time fails."
        >
          <NumberField
            label="Time limit of a test"
            unit="min"
            disabled={!canEdit}
            value={resolved.testing_timeout_minutes}
            onCommit={(value) => {
              set({ testing_timeout_minutes: value });
            }}
          />
        </Setting>
        <Setting
          editor={editor}
          row={{ keys: ["update_method"], source: "update_method" }}
          title="Update method"
          text={
            <>
              How CIHub updates a pull request in place. <code>merge</code> is the only method for now.
            </>
          }
        >
          <span className="text-base">{resolved.update_method}</span>
        </Setting>
      </SettingsSection>

      <SettingsSection
        id="cfg-batches"
        title="Batches"
        text="Several pull requests in one run. A failed batch is split to find the pull request that broke it."
      >
        <Setting
          editor={editor}
          row={{ keys: ["batching_enabled"], source: "batching_enabled" }}
          title="Batches"
          text="Test several pull requests in one run."
        >
          <Switch
            aria-label="Batches"
            checked={resolved.batching_enabled}
            disabled={!canEdit}
            onClick={() => {
              set({ batching_enabled: !resolved.batching_enabled });
            }}
          />
        </Setting>
        {resolved.batching_enabled && (
          <>
            <Setting
              editor={editor}
              row={{ keys: ["batching_size"], source: "batching_size" }}
              title="Batch size"
              text="At most this many pull requests in one run."
            >
              <NumberField
                label="Batch size"
                unit="PRs"
                disabled={!canEdit}
                value={resolved.batching_size}
                onCommit={(value) => {
                  set({ batching_size: value });
                }}
              />
            </Setting>
            <Setting
              editor={editor}
              row={{ keys: ["batching_max_wait_minutes"], source: "batching_max_wait_minutes" }}
              title="Batch wait"
              text="A batch waits this long for more pull requests before its run starts."
            >
              <NumberField
                label="Batch wait"
                unit="min"
                disabled={!canEdit}
                value={resolved.batching_max_wait_minutes}
                onCommit={(value) => {
                  set({ batching_max_wait_minutes: value });
                }}
              />
            </Setting>
            <Setting
              editor={editor}
              row={{ keys: ["bisection_concurrency"], source: "bisection_concurrency" }}
              title="Runs while a batch is split"
              text={`Tests in flight while CIHub splits a failed batch. Without a value: the ceiling of the window, ${String(ceiling)}.`}
            >
              <NumberField
                label="Runs while a batch is split"
                unit="runs"
                disabled={!canEdit}
                value={resolved.bisection_concurrency}
                onCommit={(value) => {
                  set({ bisection_concurrency: value });
                }}
              />
            </Setting>
            <Setting
              editor={editor}
              row={{ keys: ["optimization_mode"], source: "optimization_mode" }}
              title="Skip redundant tests"
              text="While a batch is split, CIHub does not test again what a result already tells."
            >
              <Switch
                aria-label="Skip redundant tests"
                checked={resolved.optimization_mode !== "off"}
                disabled={!canEdit}
                onClick={() => {
                  set({
                    optimization_mode:
                      resolved.optimization_mode === "off" ? "bisection_skip_redundant_tests" : "off",
                  });
                }}
              />
            </Setting>
          </>
        )}
      </SettingsSection>

      <SettingsSection id="cfg-failures" title="Failures" text="What the queue does when a run fails.">
        <Setting
          editor={editor}
          row={{ keys: ["failure_budget_retries"], source: "failure_budget_retries" }}
          title="Retries"
          text="A failed run runs again this many times before the pull request fails: it catches flaky tests."
        >
          <NumberField
            label="Retries"
            unit="retries"
            disabled={!canEdit}
            value={resolved.failure_budget_retries}
            onCommit={(value) => {
              set({ failure_budget_retries: value });
            }}
          />
        </Setting>
        <Setting
          editor={editor}
          row={{ keys: ["pending_failure_depth"], source: "pending_failure_depth" }}
          title="Runs a failed pull request waits for"
          text="A pull request whose test failed waits for this many of the runs after it before it fails."
        >
          <NumberField
            label="Runs a failed pull request waits for"
            unit="runs"
            disabled={!canEdit}
            value={resolved.pending_failure_depth}
            onCommit={(value) => {
              set({ pending_failure_depth: value });
            }}
          />
        </Setting>
        <Setting
          editor={editor}
          row={{ keys: ["optimistic_merging"], source: "optimistic_merging" }}
          title="Optimistic merging"
          text="A passing run of a later pull request merges the pull requests ahead of it whose own test failed or is pending."
        >
          <Switch
            aria-label="Optimistic merging"
            checked={resolved.optimistic_merging}
            disabled={!canEdit}
            onClick={() => {
              set({ optimistic_merging: !resolved.optimistic_merging });
            }}
          />
        </Setting>
      </SettingsSection>
    </>
  );
}

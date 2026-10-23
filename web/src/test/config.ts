// The configuration of a branch, as the API gives it: the unit tests and the visual views share it.
import type { BranchConfig, ConfigSettings, SettingSource } from "@/client/types.gen";

type Values = Omit<ConfigSettings, "sources">;
type Sources = ConfigSettings["sources"];

/** The values of CIHub without a file, and those of the strategy `pipelined`. */
const DEFAULT_VALUES: Values = {
  strategy: "pipelined",
  mode: "single",
  window_floor: 2,
  window_ceiling: 20,
  bisection_concurrency: 20,
  testing_timeout_minutes: 300,
  not_ready_timeout_hours: 168,
  merge_method: "squash",
  merge_commit_title: "pr_title",
  trigger: "draft_pr",
  update_method: "merge",
  batching_enabled: false,
  batching_size: 4,
  batching_max_wait_minutes: 5,
  failure_budget_retries: 1,
  optimistic_merging: false,
  pending_failure_depth: 0,
  direct_merge_mode: "always",
  optimization_mode: "bisection_skip_redundant_tests",
  comments_enabled: true,
  commands_enabled: true,
  status_check_enabled: true,
  label_commands_enabled: true,
  enqueue_label: "cihub-merge-queue-submit",
  state_labels_enabled: false,
  resubmit_on_push: false,
  keep_test_branches_minutes: 0,
  branch_prefix: "cihub",
  branch_protection_injection: "both",
  required_statuses: null,
};

/** The settings that the strategy table gives. */
const OF_STRATEGY: (keyof Values)[] = [
  "mode",
  "window_floor",
  "window_ceiling",
  "batching_enabled",
  "failure_budget_retries",
  "optimistic_merging",
  "pending_failure_depth",
];

/**
 * The settings of a queue without a file, then `changes`, each with the source that it gives: `[value, source]`.
 */
export function settingsOf(
  changes: { [K in keyof Values]?: readonly [Values[K], SettingSource] } = {},
): ConfigSettings {
  const values: Record<string, unknown> = { ...DEFAULT_VALUES };
  const sources: Record<string, SettingSource> = {};
  for (const key of Object.keys(DEFAULT_VALUES) as (keyof Values)[]) {
    sources[key] = OF_STRATEGY.includes(key) ? "strategy" : "default";
    const change = changes[key];
    if (change) {
      values[key] = change[0];
      sources[key] = change[1];
    }
  }
  return { ...(values as Values), sources: sources as Sources };
}

/**
 * The file of `main` of `acme/web`: `defaults` sets a time limit and a condition; `main` is batched, with a
 * profile for the dependency bots; `release/*` is sequential; one priority rule.
 */
export function branchConfigOf(): BranchConfig {
  const fromDefaults = { testing_timeout_minutes: [60, "defaults"] } as const;
  const ofMain = {
    ...fromDefaults,
    strategy: ["batched", "queue"],
    batching_enabled: [true, "strategy"],
    batching_size: [6, "queue"],
    pending_failure_depth: [1, "strategy"],
    optimistic_merging: [true, "strategy"],
  } as const;
  const bots = ["dependabot[bot]", "renovate[bot]"];
  return {
    branch: "main",
    state: "valid",
    path: ".cihub.yaml",
    sha: "3f2a1c9e0b1d",
    text: [
      "version: 1",
      "queues:",
      "  - branch: main",
      "    strategy: batched",
      "    batching:",
      "      size: 6",
      "    queue_conditions:",
      "      review_decision: approved",
      "      any:",
      "        - sender_permission:",
      "            min: write",
      "        - author:",
      "            - dependabot[bot]",
      "            - renovate[bot]",
      "    merge_conditions:",
      "      checks:",
      "        success: integration-tests",
      "  - branch: release/*",
      "    strategy: sequential",
      "defaults:",
      "  testing_timeout_minutes: 60",
      "  queue_conditions:",
      "    draft: false",
      "",
    ].join("\n"),
    document: {
      defaults: {
        settings: { testing_timeout_minutes: 60 },
        queue_conditions: [{ kind: "condition", attribute: "draft", operator: null, values: ["false"] }],
        merge_checks: [],
      },
      queues: [
        {
          branch: "main",
          settings: { strategy: "batched", batching_size: 6 },
          queue_conditions: [
            { kind: "condition", attribute: "review_decision", operator: null, values: ["approved"] },
            {
              kind: "block",
              block: "any",
              conditions: [
                { kind: "condition", attribute: "sender_permission", operator: "min", values: ["write"] },
                { kind: "condition", attribute: "author", operator: null, values: bots },
              ],
            },
          ],
          merge_checks: ["integration-tests"],
          profiles: [
            {
              name: "dependencies",
              conditions: [{ kind: "condition", attribute: "author", operator: null, values: bots }],
              settings: { merge_method: "merge_commit", failure_budget_retries: 2 },
            },
          ],
        },
        {
          branch: "release/*",
          settings: { strategy: "sequential" },
          queue_conditions: [],
          merge_checks: [],
          profiles: [],
        },
      ],
      priority_rules: [
        {
          name: "urgent",
          conditions: [
            { kind: "condition", attribute: "labels", operator: "any", values: ["urgent", "hotfix"] },
          ],
          priority: "urgent",
        },
      ],
    },
    settings: {
      defaults: settingsOf(fromDefaults),
      queues: [
        {
          settings: settingsOf(ofMain),
          profiles: [
            settingsOf({
              ...ofMain,
              merge_method: ["merge_commit", "profile"],
              failure_budget_retries: [2, "profile"],
            }),
          ],
        },
        {
          settings: settingsOf({
            ...fromDefaults,
            strategy: ["sequential", "queue"],
            window_floor: [1, "strategy"],
            window_ceiling: [1, "strategy"],
          }),
          profiles: [],
        },
      ],
    },
    required_checks: ["build", "test"],
    proposal: null,
    read_at: "2026-10-01T09:00:00Z",
  };
}

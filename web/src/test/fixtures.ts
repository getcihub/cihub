import type { Graph, Item, Me, OrgQueue, QueueDetail } from "../client/types.gen";

export const ada: Me = {
  login: "ada",
  orgs: [
    { slug: "acme", role: "admin" },
    { slug: "acme-labs", role: "member" },
  ],
  version: "0.2.0",
  app_install_url: "https://github.com/apps/cihub/installations/new",
};

/** A queue of `acme`, running, with `pending` PRs in line. */
export function queueOf(
  repo: string,
  name: string,
  pending = 0,
  configError: string | null = null,
): OrgQueue {
  return {
    repo,
    queue: {
      id: `queue_${repo}_${name}`,
      repo_id: `repo_${repo}`,
      name,
      target_branch: name,
      state: "running",
      mode: "single",
      state_changed_at: "2026-10-01T09:00:00Z",
      strategy: "pipelined",
      access: "ok",
      has_config_file: true,
      config_error: configError,
    },
    counts: { not_ready: 0, pending, testing: 0, pending_failure: 0, tests_passed: 0 },
    merged_last_24h: 0,
    failed_last_24h: 0,
    last_merged_at: null,
  };
}

/** The detail of the queue of `entry`, with no PR and the default settings. */
export function detailOf(entry: OrgQueue, changes: Partial<QueueDetail> = {}): QueueDetail {
  return {
    queue: entry.queue,
    counts: entry.counts,
    merged_last_24h: entry.merged_last_24h,
    failed_last_24h: entry.failed_last_24h,
    last_merged_at: entry.last_merged_at,
    live_runs: 0,
    window: { current: 2, floor: 2, ceiling: 20 },
    settings: {
      batching_enabled: false,
      batching_size: 4,
      batching_max_wait_minutes: 5,
      failure_budget_retries: 1,
      merge_method: "squash",
    },
    ...changes,
  };
}

/** The current item of the PR `number`, submitted by its author with a comment. */
export function itemOf(number: number, state: string, changes: Partial<Item> = {}): Item {
  return {
    id: `item_${String(number)}`,
    queue_id: "queue_web_main",
    pr_number: number,
    title: `Change ${String(number)}`,
    author: "marie",
    head_sha: "abc",
    state,
    state_reason: "submitted",
    state_description: "the PR was submitted",
    state_changed_at: new Date().toISOString(),
    priority: 100,
    position: null,
    is_current: true,
    block_reasons: [],
    group_id: null,
    run_id: null,
    merged_sha: null,
    no_batch: false,
    submitted_by: "marie",
    submitted_via: "comment",
    submitted_at: new Date().toISOString(),
    ...changes,
  };
}

export const noGraph: Graph = { target_head_sha: null, nodes: [], edges: [], runs: [] };

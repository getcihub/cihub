import type { Graph, ItemState, Pr, Queue } from "../client/types.gen";
import type { Me } from "../session";
import type { Answers } from "./app";

export const ada: Me = {
  login: "ada",
  orgs: [
    { slug: "acme", role: "admin" },
    { slug: "acme-labs", role: "member" },
  ],
  version: "0.2.0",
  app_install_url: "https://github.com/apps/cihub/installations/new",
};

/** A page of a list of the API that holds every entry. */
export function pageOf<K extends string, T>(name: K, entries: T[]) {
  return { [name]: entries, has_more: false, count: entries.length, cursor: null } as Record<K, T[]> & {
    has_more: boolean;
    count: number;
    cursor: null;
  };
}

/** The answers of `/user`, `/user/orgs` and `/instance` for the person `me`. */
export function answersOfMe(me: Me): Answers {
  return {
    "/api/v1/user": [200, { login: me.login }],
    "/api/v1/user/orgs": [200, pageOf("orgs", me.orgs)],
    "/api/v1/instance": [200, { version: me.version, app_install_url: me.app_install_url }],
  };
}

/** The answers of the reads of the person of a session, when there is none. */
export const signedOut: Answers = {
  "/api/v1/user": [
    401,
    { type: "about:blank", title: "Unauthorized", status: 401, detail: "", code: "unauthorized" },
  ],
  "/api/v1/user/orgs": [
    401,
    { type: "about:blank", title: "Unauthorized", status: 401, detail: "", code: "unauthorized" },
  ],
  "/api/v1/instance": [
    401,
    { type: "about:blank", title: "Unauthorized", status: 401, detail: "", code: "unauthorized" },
  ],
};

/** A queue of `acme`, running, with `pending` PRs in line, no PR else, and the default settings. */
export function queueOf(repo: string, name: string, pending = 0, configError: string | null = null): Queue {
  return {
    id: `queue_${repo}_${name}`,
    repo_id: `repo_${repo}`,
    repo,
    name,
    target_branch: name,
    state: "running",
    mode: "single",
    state_changed_at: "2026-10-01T09:00:00Z",
    strategy: "pipelined",
    access: "ok",
    has_config_file: true,
    config_error: configError,
    counts: { not_ready: 0, pending, testing: 0, pending_failure: 0, tests_passed: 0 },
    merged_last_24h: 0,
    failed_last_24h: 0,
    last_merged_at: null,
    live_runs: 0,
    window: { current: 2, floor: 2, ceiling: 20 },
    settings: {
      batching_enabled: false,
      batching_size: 4,
      batching_max_wait_minutes: 5,
      failure_budget_retries: 1,
      merge_method: "squash",
    },
    config_check: { findings: [], checked_at: null },
  };
}

/** The queue `entry`, as its page reads it, with `changes`. */
export function detailOf(entry: Queue, changes: Partial<Queue> = {}): Queue {
  return { ...entry, ...changes };
}

/** The PR `number` of `acme/web` in the queue `main`, submitted by its author with a comment. */
export function itemOf(number: number, state: ItemState, changes: Partial<Pr> = {}): Pr {
  return {
    id: `item_${String(number)}`,
    queue_id: "queue_web_main",
    repo: "web",
    queue: "main",
    number,
    title: `Change ${String(number)}`,
    author: "marie",
    head_sha: "abc",
    state,
    state_reason: "submitted",
    state_description: "the PR was submitted",
    state_changed_at: new Date().toISOString(),
    priority: 100,
    position: null,
    block_reasons: [],
    group_id: null,
    run_id: null,
    merged_sha: null,
    no_batch: false,
    submitted_by: "marie",
    submitted_via: "comment",
    submitted_at: new Date().toISOString(),
    ahead: [],
    merges_in_seconds: null,
    ...changes,
  };
}

export const noGraph: Graph = { target_head_sha: null, nodes: [], edges: [], runs: [] };

const LIVE_STATES = "not_ready,pending,testing,pending_failure,tests_passed";

/** The read of the live PRs of the queue `queue` of `acme/web`, as its page asks for it. */
export function liveOf(queue = "main"): string {
  return `/api/v1/orgs/acme/prs?repo=web&queue=${queue}&state=${LIVE_STATES}&sort=position&limit=100`;
}

/** The answers of the reads of the PRs of the person in `org`: the live ones, then the ones merged lately. */
export function answersOfMine(
  org: string,
  live: Pr[] | [number, unknown][] = [],
  merged: Pr[] = [],
): Answers {
  const reads = `/api/v1/orgs/${org}/prs?author=@me`;
  const isSequence = live.length > 0 && Array.isArray(live[0]);
  return {
    [`${reads}&state=${LIVE_STATES}&sort=position&limit=100`]: isSequence
      ? (live as [number, unknown][])
      : [200, pageOf("prs", live as Pr[])],
    [`${reads}&state=merged&limit=10`]: [200, pageOf("prs", merged)],
  };
}

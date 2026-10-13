// The API of the visual views: an organization with queues in every state, PRs, runs and a history, answered at a
// fixed time so that two captures of the same build are the same.
import type { ErrorCode, Event, Graph, ItemState, Pr, Problem, Queue, Run } from "@/client/types.gen";

export const NOW = Date.parse("2026-10-12T10:00:00Z");

const ago = (minutes: number): string => new Date(NOW - minutes * 60_000).toISOString();

const LIVE_STATES: ItemState[] = ["not_ready", "pending", "testing", "pending_failure", "tests_passed"];

/** A status and a body, or `null` for an empty body. */
export type Answer = [number, unknown];

/** The request of the page to the API: the path after `/api/v1`, the query and the method. */
export interface ApiRequest {
  path: string;
  query: URLSearchParams;
  method: string;
}

/** The person of the session: `ada`, `grace`, who has no organization, or nobody. */
export type Person = "ada" | "grace" | "nobody";

export function problem(status: number, code: ErrorCode, detail = ""): Problem {
  return { type: "about:blank", title: code, status, detail, code };
}

function listOf(name: string, entries: unknown[]): Record<string, unknown> {
  return { [name]: entries, has_more: false, count: entries.length, cursor: null };
}

function queueOf(repo: string, name: string, changes: Partial<Queue> = {}): Queue {
  return {
    id: `queue_${repo}_${name}`,
    repo_id: `repo_${repo}`,
    repo,
    name,
    target_branch: name,
    state: "running",
    mode: "single",
    state_changed_at: ago(6),
    strategy: "pipelined",
    access: "ok",
    has_config_file: false,
    config_sha: null,
    config_error: null,
    counts: { not_ready: 0, pending: 0, testing: 0, pending_failure: 0, tests_passed: 0 },
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
    ...changes,
  };
}

// Nine queues: the filter of the cards shows from eight.
const QUEUES: Queue[] = [
  queueOf("web", "main", {
    has_config_file: true,
    config_sha: "4f2a9c1e77",
    counts: { not_ready: 1, pending: 1, testing: 1, pending_failure: 1, tests_passed: 1 },
    merged_last_24h: 3,
    failed_last_24h: 1,
    last_merged_at: ago(14),
    live_runs: 3,
    settings: {
      batching_enabled: true,
      batching_size: 4,
      batching_max_wait_minutes: 5,
      failure_budget_retries: 2,
      merge_method: "merge_commit",
    },
  }),
  queueOf("web", "release", {
    state: "paused",
    state_changed_by: "hugo",
    config_error: "line 3: `batching.size` must be 1 or more",
    config_check: {
      findings: [
        {
          code: "required_check_missing",
          message: "The branch protection of release requires `ci/test`, which no run reports.",
        },
      ],
      checked_at: ago(3),
    },
    counts: { not_ready: 0, pending: 2, testing: 0, pending_failure: 0, tests_passed: 0 },
  }),
  queueOf("api", "main", {
    state: "draining",
    counts: { not_ready: 0, pending: 1, testing: 1, pending_failure: 1, tests_passed: 0 },
    merged_last_24h: 12,
    last_merged_at: ago(90),
  }),
  queueOf("docs", "main", { config_error: "line 1: unknown key `strategy2`" }),
  queueOf("mobile", "main", { access: "suspended" }),
  queueOf("infra", "main", { merged_last_24h: 1, last_merged_at: ago(300) }),
  queueOf("site", "main"),
  queueOf("sdk", "main"),
  queueOf("cli", "main", {
    counts: { not_ready: 0, pending: 4, testing: 2, pending_failure: 0, tests_passed: 0 },
  }),
];

function prOf(repo: string, number: number, state: ItemState, changes: Partial<Pr> = {}): Pr {
  return {
    id: `item_${repo}_${String(number)}`,
    queue_id: `queue_${repo}_main`,
    repo,
    queue: "main",
    number,
    title: `Change ${String(number)}`,
    author: "marie",
    head_sha: "abc",
    state,
    state_reason: "submitted",
    state_description: "the PR was submitted to the queue",
    state_changed_at: ago(5),
    priority: 100,
    position: null,
    block_reasons: [],
    group_id: null,
    run_id: null,
    merged_sha: null,
    no_batch: false,
    submitted_by: "marie",
    submitted_via: "comment",
    submitted_at: ago(30),
    ahead: [],
    merges_in_seconds: null,
    ...changes,
  };
}

const PRS: Pr[] = [
  prOf("web", 118, "testing", {
    title: "Export invoices as CSV",
    position: 1000,
    priority: 0,
    run_id: "run_1",
    state_description: "a test run started",
    submitted_at: ago(42),
  }),
  prOf("web", 121, "pending_failure", {
    title: "Fix the login redirect",
    author: "lea",
    position: 2000,
    priority: 10,
    run_id: "run_2",
    state_description: "the tests failed; the PRs ahead may explain it",
    ahead: [{ pr_number: 118, state: "testing" }],
  }),
  prOf("web", 124, "tests_passed", {
    title: "Fix the payment webhook retry",
    author: "ada",
    position: 3000,
    run_id: "run_3",
    state_description: "the tests passed; it merges after the PRs ahead",
    merges_in_seconds: 300,
    ahead: [
      { pr_number: 121, state: "pending_failure" },
      { pr_number: 118, state: "testing" },
    ],
  }),
  prOf("web", 130, "pending", {
    title: "Bump the version of the parser to 4.2",
    author: "alex",
    position: 4000,
    priority: 200,
    ahead: [
      { pr_number: 124, state: "tests_passed" },
      { pr_number: 121, state: "pending_failure" },
      { pr_number: 118, state: "testing" },
    ],
  }),
  prOf("web", 133, "not_ready", {
    title: "Update the install guide",
    author: "grace",
    state_description: "waits: the required check `ci` is in_progress",
  }),
  prOf("web", 110, "merged", {
    title: "Add the audit log",
    merged_sha: "77e1c0ffee",
    state_description: "merged into main",
    state_changed_at: ago(14),
    submitted_at: ago(55),
  }),
  prOf("web", 111, "failed", {
    title: "Remove the old cache",
    author: "lea",
    state_description: "the tests failed: `ci/test`",
    state_changed_at: ago(120),
  }),
  prOf("web", 112, "cancelled", {
    title: "Try the new runner",
    author: "alex",
    state_description: "cancelled by @alex",
    state_changed_at: ago(200),
  }),
  prOf("web", 70, "merged", {
    title: "Speed up the sign-in",
    author: "ada",
    merged_sha: "1234567abc",
    state_changed_at: ago(2),
    submitted_at: ago(32),
  }),
  prOf("api", 77, "pending", {
    title: "Retry the webhook sweep",
    author: "ada",
    position: 2000,
    merges_in_seconds: 1500,
    ahead: [
      { pr_number: 75, state: "testing" },
      { pr_number: 76, state: "pending_failure" },
    ],
  }),
];

const GRAPHS: Record<string, Graph> = {
  "web/main": {
    target_head_sha: "9c1e2a4f00aa",
    nodes: PRS.filter((pr) => pr.repo === "web" && LIVE_STATES.includes(pr.state)).map((pr) => ({
      item_id: pr.id,
      pr_number: pr.number,
      state: pr.state,
      position: pr.position,
    })),
    edges: [],
    runs: [
      {
        run_id: "run_1",
        item_ids: ["item_web_118"],
        predecessor_ids: [],
        attempt: 1,
        status: "in_progress",
        started_at: ago(8),
      },
      {
        run_id: "run_2",
        item_ids: ["item_web_121"],
        predecessor_ids: ["item_web_118"],
        attempt: 2,
        status: "failed",
        started_at: ago(7),
      },
      {
        run_id: "run_3",
        item_ids: ["item_web_124"],
        predecessor_ids: ["item_web_118", "item_web_121"],
        attempt: 1,
        status: "succeeded",
        started_at: ago(6),
      },
    ],
  },
};

const RUNS: Record<string, Run> = {
  run_1: {
    id: "run_1",
    queue_id: "queue_web_main",
    group_id: "group_1",
    attempt: 1,
    status: "in_progress",
    status_reason: "test_run_started",
    test_branch: "cihub-merge/main/run_1",
    test_sha: "abc",
    base_sha: "9c1e2a4f00",
    draft_pr_number: 4521,
    tested_pr_numbers: [118],
    head_shas: ["abc"],
    predecessor_pr_numbers: [],
    required_statuses: ["ci"],
    required_statuses_source: "repo_config",
    started_at: ago(8),
    ended_at: null,
    last_report_at: null,
    failure_reason: null,
    failure_causing_statuses: [],
    conflict_culprit: null,
    checks: [
      {
        name: "build",
        kind: "check",
        required: true,
        conclusion: "success",
        details_url: null,
        observed_at: ago(4),
      },
      {
        name: "test",
        kind: "check",
        required: true,
        conclusion: "failure",
        details_url: null,
        observed_at: ago(3),
      },
      {
        name: "lint",
        kind: "check",
        required: false,
        conclusion: null,
        details_url: null,
        observed_at: ago(5),
      },
      {
        name: "docs",
        kind: "status",
        required: false,
        conclusion: "skipped",
        details_url: null,
        observed_at: ago(5),
      },
    ],
    impacted_targets: null,
    dependents: [],
  },
};

function eventOf(seq: number, itemId: string, changes: Partial<Event>): Event {
  return {
    id: `event_${String(seq)}`,
    seq,
    item_id: itemId,
    type: "item.transitioned",
    actor_kind: "system",
    actor: null,
    occurred_at: ago(1),
    description: "",
    payload: {},
    ...changes,
  };
}

const EVENTS: Record<string, Event[]> = {
  "web/118": [
    eventOf(1, "item_web_118", {
      actor_kind: "user",
      actor: "marie",
      occurred_at: ago(42),
      description: "the PR was submitted to the queue",
      payload: { to: "not_ready" },
    }),
    eventOf(2, "item_web_118", {
      type: "item.readiness_changed",
      occurred_at: ago(30),
      description: "nothing blocks the PR",
    }),
    eventOf(3, "item_web_118", {
      occurred_at: ago(29),
      description: "the PR entered the queue",
      payload: { to: "pending" },
    }),
    eventOf(4, "item_web_118", {
      type: "item.restart_requested",
      actor_kind: "user",
      actor: "ada",
      occurred_at: ago(10),
      description: "a new test was asked",
    }),
    eventOf(5, "item_web_118", {
      occurred_at: ago(8),
      description: "a test run started on `cihub-merge/main/run_1`",
      payload: { to: "testing" },
    }),
  ],
  "web/133": [
    eventOf(6, "item_web_133", {
      type: "item.readiness_changed",
      occurred_at: ago(3),
      description: "waits: the required check `ci` is in_progress",
    }),
  ],
};

const PEOPLE: Record<
  Exclude<Person, "nobody">,
  { login: string; orgs: { slug: string; role: "admin" | "member" }[] }
> = {
  ada: {
    login: "ada",
    orgs: [
      { slug: "acme", role: "admin" },
      { slug: "acme-labs", role: "member" },
    ],
  },
  grace: { login: "grace", orgs: [] },
};

function answerOfSession(path: string, person: Person): Answer | null {
  if (path !== "/user" && path !== "/user/orgs" && path !== "/instance") {
    return null;
  }
  if (person === "nobody") {
    return [401, problem(401, "unauthorized")];
  }

  const { login, orgs } = PEOPLE[person];
  if (path === "/user") {
    return [200, { login }];
  }
  if (path === "/user/orgs") {
    return [200, listOf("orgs", orgs)];
  }
  return [200, { version: "0.2.0", app_install_url: "https://github.com/apps/cihub/installations/new" }];
}

function answerOfQueue(path: string): Answer | null {
  const match = /^\/orgs\/acme\/repos\/([^/]+)\/queues\/([^/]+)(\/graph)?$/.exec(path);
  if (!match) {
    return null;
  }

  const [, repo, name, isGraph] = match;
  const queue = QUEUES.find((entry) => entry.repo === repo && entry.name === name);
  if (!queue) {
    return [404, problem(404, "queue_not_found")];
  }
  if (isGraph) {
    return [
      200,
      GRAPHS[`${repo ?? ""}/${name ?? ""}`] ?? { target_head_sha: null, nodes: [], edges: [], runs: [] },
    ];
  }
  return [200, queue];
}

function answerOfPr(path: string): Answer | null {
  const match = /^\/orgs\/acme\/repos\/([^/]+)\/prs\/(\d+)(\/events)?$/.exec(path);
  if (!match) {
    return null;
  }

  const [, repo, number, isEvents] = match;
  const pr = PRS.find((entry) => entry.repo === repo && entry.number === Number(number));
  if (!pr) {
    return [404, problem(404, "pr_not_found")];
  }
  return isEvents ? [200, listOf("events", EVENTS[`${repo ?? ""}/${number ?? ""}`] ?? [])] : [200, pr];
}

/** The PRs that a list of the API gives: by state, author, repository, queue and time, as the server filters. */
function answerOfPrList(query: URLSearchParams, org: string): Answer {
  const states = (query.get("state") ?? "").split(",");
  const since = query.get("state_changed_since");
  const repo = query.get("repo");
  const queue = query.get("queue");
  const found = PRS.filter(
    (pr) =>
      org === "acme" &&
      states.includes(pr.state) &&
      (query.get("author") !== "@me" || pr.author === "ada") &&
      (!repo || pr.repo === repo) &&
      (!queue || pr.queue === queue) &&
      (!since || Date.parse(pr.state_changed_at) >= Date.parse(since)),
  );

  if (query.get("sort") === "position") {
    found.sort((a, b) => (a.position ?? Infinity) - (b.position ?? Infinity));
  }
  return [200, listOf("prs", found)];
}

/** What the API answers to `request` for `person`: a read answers the fixtures, an action is accepted. */
export function answerOf(request: ApiRequest, person: Person): Answer {
  const { path, query, method } = request;
  const session = answerOfSession(path, person);
  if (session) {
    return session;
  }
  if (path === "/orgs/acme/queues") {
    return [200, listOf("queues", QUEUES)];
  }
  if (path === "/orgs/acme-labs/queues") {
    return [200, listOf("queues", [])];
  }
  if (/^\/orgs\/[^/]+\/queues$/.test(path)) {
    return [404, problem(404, "org_not_found")];
  }
  if (method !== "GET") {
    return [202, null];
  }

  const prList = /^\/orgs\/([^/]+)\/prs$/.exec(path);
  if (prList) {
    return answerOfPrList(query, prList[1] ?? "");
  }
  return (
    answerOfQueue(path) ?? answerOfPr(path) ?? answerOfRun(path) ?? [404, problem(404, "route_not_found")]
  );
}

function answerOfRun(path: string): Answer | null {
  const match = /^\/orgs\/acme\/runs\/(.+)$/.exec(path);
  if (!match) {
    return null;
  }

  const run = RUNS[match[1] ?? ""];
  return run ? [200, run] : [404, problem(404, "run_not_found")];
}

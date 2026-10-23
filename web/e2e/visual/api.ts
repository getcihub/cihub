// The API of the visual views: an organization with queues in every state, PRs, runs and a history, answered at a
// fixed time so that two captures of the same build are the same.
import type {
  ErrorCode,
  Event,
  Graph,
  GraphNode,
  HealthBucket,
  ItemState,
  Pr,
  Problem,
  Queue,
  QueueHealth,
  RepoDetail,
  RuleCheck,
  Run,
  SetupState,
  ConfigFile,
} from "@/client/types.gen";

export const NOW = Date.parse("2026-10-12T10:00:00Z");

// The queue conditions that #133 does not meet, as the API writes them: a condition, a check, a boolean, a group.
const CONDITIONS_OF_133 = [
  "labels.none: wip",
  "checks.success: Build Success",
  "draft: false",
  "any: [sender_permission.min: write; author: dependabot[bot], renovate[bot]]",
];

const ago = (minutes: number): string => new Date(NOW - minutes * 60_000).toISOString();

const LIVE_STATES: ItemState[] = ["not_ready", "pending", "testing", "pending_failure", "tests_passed"];

/** A status and a body, or `null` for an empty body. */
export type Answer = [number, object | null];

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

function listOf(name: string, entries: object[]): object {
  return { [name]: entries, has_more: false, count: entries.length, cursor: null };
}

function queueOf(repo: string, name: string, changes: Partial<Queue> = {}): Queue {
  return {
    id: `queue_${repo}_${name}`,
    repo_id: `repo_${repo}`,
    repo,
    target_branch: name,
    is_default_branch: name === "main",
    state: "running",
    mode: "single",
    state_changed_at: ago(6),
    strategy: "pipelined",
    is_suspended: false,
    has_config_file: false,
    config_sha: null,
    config_error: null,
    counts: { not_ready: 0, pending: 0, testing: 0, pending_failure: 0, tests_passed: 0 },
    merged_last_24h: 0,
    failed_last_24h: 0,
    failed_or_cancelled_last_24h: 0,
    last_merged_at: null,
    live_runs: 0,
    window: { current: 2, floor: 2, ceiling: 20 },
    settings: {
      batching_enabled: false,
      batching_size: 4,
      batching_max_wait_minutes: 5,
      failure_budget_retries: 1,
      merge_method: "squash",
      enqueue_label: "cihub-merge-queue-submit",
    },
    setup: { findings: [], checked_at: null },
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
    failed_or_cancelled_last_24h: 2,
    last_merged_at: ago(14),
    live_runs: 3,
    settings: {
      batching_enabled: true,
      batching_size: 4,
      batching_max_wait_minutes: 5,
      failure_budget_retries: 2,
      merge_method: "merge_commit",
      enqueue_label: "ready-to-merge",
    },
  }),
  queueOf("web", "release", {
    state: "paused",
    state_changed_by: "hugo",
    config_error: "line 3: `batching.size` must be 1 or more",
    setup: {
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
    live_runs: 2,
  }),
  queueOf("docs", "main", { config_error: "line 1: unknown key `strategy2`" }),
  queueOf("mobile", "main", { is_suspended: true }),
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
    author: { login: "marie" },
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
    url: `https://scm.example/acme/${repo}/pull/${String(number)}`,
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
    author: { login: "lea" },
    position: 2000,
    priority: 10,
    run_id: "run_2",
    state_description: "the tests failed; the PRs ahead may explain it",
    ahead: [{ pr_number: 118, state: "testing" }],
  }),
  prOf("web", 124, "tests_passed", {
    title: "Fix the payment webhook retry",
    author: { login: "ada" },
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
    author: { login: "alex" },
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
    author: { login: "grace" },
    state_description: "the PR was submitted to the queue",
    block_reasons: [
      { kind: "awaiting_required_check", check: "ci", status: "running" },
      { kind: "queue_conditions_unmet", conditions: CONDITIONS_OF_133 },
    ],
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
    author: { login: "lea" },
    state_description: "the tests failed: `ci/test`",
    state_changed_at: ago(120),
  }),
  prOf("web", 112, "cancelled", {
    title: "Try the new runner",
    author: { login: "alex" },
    state_description: "cancelled by @alex",
    state_changed_at: ago(200),
  }),
  prOf("web", 70, "merged", {
    title: "Speed up the sign-in",
    author: { login: "ada" },
    merged_sha: "1234567abc",
    state_changed_at: ago(2),
    submitted_at: ago(32),
  }),
  prOf("api", 75, "testing", {
    title: "Log the slow queries",
    author: { login: "hugo" },
    position: 1000,
    run_id: "run_api_1",
    state_description: "a test run started",
  }),
  prOf("api", 76, "pending_failure", {
    title: "Drop the old token table",
    author: { login: "lea" },
    position: 1500,
    run_id: "run_api_2",
    state_description: "the tests failed; the PRs ahead may explain it",
    ahead: [{ pr_number: 75, state: "testing" }],
  }),
  prOf("api", 77, "pending", {
    title: "Retry the webhook sweep",
    author: { login: "ada" },
    position: 2000,
    merges_in_seconds: 1500,
    ahead: [
      { pr_number: 75, state: "testing" },
      { pr_number: 76, state: "pending_failure" },
    ],
  }),
];

/** The live PRs of the queue of the branch `name` of `repo`, as the nodes of its graph: the server gives every one. */
function nodesOf(repo: string, name: string): GraphNode[] {
  return PRS.filter((pr) => pr.repo === repo && pr.queue === name && LIVE_STATES.includes(pr.state)).map(
    (pr) => ({
      item_id: pr.id,
      pr_number: pr.number,
      state: pr.state,
      position: pr.position,
    }),
  );
}

const GRAPHS: Record<string, Graph> = {
  "web/main": {
    target_head_sha: "9c1e2a4f00aa",
    nodes: nodesOf("web", "main"),
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
  "api/main": {
    target_head_sha: "51d0c8e0bb",
    nodes: nodesOf("api", "main"),
    edges: [],
    runs: [
      {
        run_id: "run_api_1",
        item_ids: ["item_api_75"],
        predecessor_ids: [],
        attempt: 1,
        status: "in_progress",
        started_at: ago(9),
      },
      {
        run_id: "run_api_2",
        item_ids: ["item_api_76"],
        predecessor_ids: ["item_api_75"],
        attempt: 1,
        status: "failed",
        started_at: ago(8),
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
    draft_pr_url: "https://scm.example/acme/web/pull/4521",
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
        status: "completed",
        started_at: ago(8),
        completed_at: ago(4),
        details_url: null,
        observed_at: ago(4),
      },
      {
        name: "test",
        kind: "check",
        required: true,
        conclusion: "failure",
        status: "completed",
        started_at: ago(7),
        completed_at: ago(3),
        details_url: null,
        observed_at: ago(3),
      },
      {
        name: "lint",
        kind: "check",
        required: false,
        conclusion: null,
        status: "in_progress",
        started_at: ago(5),
        completed_at: null,
        details_url: null,
        observed_at: ago(5),
      },
      {
        name: "docs",
        kind: "status",
        required: false,
        conclusion: "skipped",
        status: "completed",
        started_at: null,
        completed_at: ago(5),
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
      payload: { from: null, to: "not_ready" },
    }),
    eventOf(2, "item_web_118", {
      type: "item.readiness_changed",
      occurred_at: ago(30),
      description: "nothing blocks the PR",
    }),
    eventOf(3, "item_web_118", {
      occurred_at: ago(29),
      description: "the PR entered the queue",
      payload: { from: "not_ready", to: "pending" },
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
      payload: { from: "pending", to: "testing" },
    }),
  ],
  "web/133": [
    eventOf(6, "item_web_133", {
      actor_kind: "user",
      actor: "grace",
      occurred_at: ago(30),
      description: "the PR was submitted to the queue",
      payload: { from: null, to: "not_ready" },
    }),
    eventOf(7, "item_web_133", {
      type: "item.readiness_changed",
      occurred_at: ago(3),
      description: "the required check `ci` is running; the queue conditions … are not met",
      payload: {
        block_reasons: [
          { kind: "awaiting_required_check", check: "ci", status: "running" },
          { kind: "queue_conditions_unmet", conditions: CONDITIONS_OF_133 },
        ],
      },
    }),
  ],
};

const PEOPLE: Record<
  Exclude<Person, "nobody">,
  { login: string; name?: string; orgs: { slug: string; role: "admin" | "member"; is_suspended: boolean }[] }
> = {
  ada: {
    login: "ada",
    name: "Ada Lovelace",
    orgs: [
      { slug: "acme", role: "admin", is_suspended: false },
      { slug: "acme-labs", role: "member", is_suspended: false },
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

  const { login, name, orgs } = PEOPLE[person];
  if (path === "/user") {
    return [200, { login, name, synced_at: "2026-10-01T09:00:00Z" }];
  }
  if (path === "/user/orgs") {
    return [200, listOf("orgs", orgs)];
  }
  return [200, { version: "0.2.0", app_install_url: "https://github.com/apps/cihub/installations/new" }];
}

/** The hours of the last 24 hours, the oldest first: a busy day, quiet at night. */
function bucketsOf(merged: number[], failed: number[]): HealthBucket[] {
  return merged.map((count, index) => ({
    start: new Date(NOW - (23 - index) * 3_600_000 - 3_600_000).toISOString(),
    merged: count,
    failed: failed[index] ?? 0,
    time_to_merge: count > 0 ? { median_seconds: 1_200 + index * 40, p90_seconds: 2_400 + index * 90 } : null,
  }));
}

/** The days of a period that the person chose: a busy week, quiet on the weekends. */
function daysOf(from: string, to: string): HealthBucket[] {
  const days = Math.round((Date.parse(to) - Date.parse(from)) / 86_400_000);
  const merged = [6, 8, 7, 9, 5, 1, 0];
  return Array.from({ length: days }, (_, index) => {
    const count = merged[index % 7] ?? 0;
    return {
      start: new Date(Date.parse(from) + index * 86_400_000).toISOString(),
      merged: count,
      failed: index % 5 === 2 ? 1 : 0,
      time_to_merge:
        count > 0 ? { median_seconds: 1_260 + index * 20, p90_seconds: 2_700 + index * 45 } : null,
    };
  });
}

/** The Health of `web/main`: a check that passes on a retry; the other queues ended nothing. */
function healthOf(repo: string, name: string, query: URLSearchParams): QueueHealth {
  const isBusy = repo === "web" && name === "main";
  const empty = { merged: 0, failed: 0, cancelled: 0, time_to_merge: null };
  const range = query.get("range") ?? "24h";
  const chosen = { from: query.get("from"), to: query.get("to") };
  const from = chosen.from ?? new Date(NOW - 24 * 3_600_000).toISOString();

  if (!isBusy) {
    return {
      from,
      to: new Date(NOW).toISOString(),
      verdict: { kind: "quiet" },
      current: empty,
      previous: empty,
      buckets: bucketsOf(Array<number>(24).fill(0), []),
      time_in_states: [],
      left_without_merge: [],
      checks: [],
    };
  }
  return {
    from,
    to: chosen.to ?? new Date(NOW).toISOString(),
    verdict: {
      kind: "check_passes_on_retry",
      check: "ci / e2e",
      times: range === "24h" && !chosen.to ? 4 : 19,
    },
    current: {
      merged: 31,
      failed: 3,
      cancelled: 2,
      time_to_merge: { median_seconds: 1_380, p90_seconds: 2_940 },
    },
    previous: {
      merged: 27,
      failed: 2,
      cancelled: 1,
      time_to_merge: { median_seconds: 1_560, p90_seconds: 3_120 },
    },
    buckets:
      chosen.from && chosen.to
        ? daysOf(chosen.from, chosen.to)
        : bucketsOf(
            [0, 0, 0, 0, 0, 0, 0, 1, 3, 4, 3, 2, 4, 3, 2, 3, 2, 1, 1, 0, 0, 0, 1, 1],
            [0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0],
          ),
    time_in_states: [
      { state: "not_ready", median_seconds: 420 },
      { state: "pending", median_seconds: 240 },
      { state: "testing", median_seconds: 660 },
      { state: "pending_failure", median_seconds: 0 },
      { state: "tests_passed", median_seconds: 60 },
    ],
    left_without_merge: [
      {
        conclusion: "failed",
        reason: "test_run_required_status_failed",
        description: "a required check failed",
        count: 2,
      },
      {
        conclusion: "cancelled",
        reason: "pr_pushed_to",
        description: "the PR received a new commit after its submission",
        count: 2,
      },
      {
        conclusion: "failed",
        reason: "start_test_run_merge_conflict",
        description: "the PR conflicts with the target branch or with a PR ahead of it",
        count: 1,
      },
    ],
    checks: [
      { name: "ci / build", kind: "check", required: true, runs: 41, failed: 1, passed_on_retry: 0 },
      { name: "ci / coverage", kind: "check", required: false, runs: 41, failed: 0, passed_on_retry: 0 },
      { name: "ci / e2e", kind: "check", required: true, runs: 45, failed: 6, passed_on_retry: 4 },
      { name: "ci / lint", kind: "check", required: true, runs: 41, failed: 0, passed_on_retry: 0 },
    ],
  };
}

function answerOfQueue(path: string, query: URLSearchParams): Answer | null {
  const match = /^\/orgs\/acme\/repos\/([^/]+)\/queues\/([^/]+)(\/graph|\/health)?$/.exec(path);
  if (!match) {
    return null;
  }

  const [, repo, name, part] = match;
  const queue = QUEUES.find((entry) => entry.repo === repo && entry.target_branch === name);
  if (!queue) {
    return [404, problem(404, "queue_not_found")];
  }
  if (part === "/health") {
    return [200, healthOf(queue.repo, queue.target_branch, query)];
  }
  if (part === "/graph") {
    return [
      200,
      GRAPHS[`${repo ?? ""}/${name ?? ""}`] ?? {
        target_head_sha: "c7f2210ab4",
        nodes: nodesOf(queue.repo, queue.target_branch),
        edges: [],
        runs: [],
      },
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
function answerOfPrList(query: URLSearchParams, org: string, repo: string | undefined): Answer {
  const states = (query.get("state") ?? "").split(",");
  const since = query.get("state_changed_since");
  const author = query.get("author");
  const queue = query.get("queue");
  const found = PRS.filter(
    (pr) =>
      org === "acme" &&
      states.includes(pr.state) &&
      (!author || pr.author.login === author) &&
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

  const prList = /^\/orgs\/([^/]+)(?:\/repos\/([^/]+))?\/prs$/.exec(path);
  if (prList) {
    return answerOfPrList(query, prList[1] ?? "", prList[2]);
  }
  return (
    answerOfRepo(path, person) ??
    answerOfQueue(path, query) ??
    answerOfPr(path) ??
    answerOfRun(path) ?? [404, problem(404, "route_not_found")]
  );
}

/** The repositories of `acme` without a queue: `handbook`, whose App is not exempt, and `landing`, ready. */
const WITHOUT_QUEUE: Record<string, "cannot_merge" | "ready"> = {
  handbook: "cannot_merge",
  landing: "ready",
};

/** A repository without a queue: its verdict, its default branch, the rules that fail, and its file. */
interface Start {
  state: SetupState;
  branch?: string;
  failed?: string[];
  config?: Partial<ConfigFile>;
}

/** The repositories of `acme-labs`, which has no queue yet: two ready, one with a warning, two blocked. */
const LABS: Record<string, Start> = {
  app: { state: "ready" },
  handbook: { state: "ready_with_warnings", failed: ["up_to_date_required"] },
  playground: {
    state: "cannot_merge",
    branch: "trunk",
    failed: ["app_not_exempt", "test_branches_not_excluded"],
  },
  site: {
    state: "ready",
    config: { state: "valid", path: ".cihub.yaml", sha: "a41c9e0", entries: ["main", "release"] },
  },
  terraform: {
    state: "invalid_config",
    config: {
      state: "invalid",
      path: ".cihub.yaml",
      sha: "c03f7b1",
      error: "line 4, column 15: `fast` is not a strategy; use sequential, pipelined, batched or parallel",
      line: 4,
      column: 15,
      text: "version: 1\nqueues:\n  - branch: main\n    strategy: fast\n",
    },
  },
};

/** The four rules, as the server writes them: when they pass, when they fail, and what to change. */
const RULES: { code: string; blocks_merge: boolean; passed: string; failed: string; change: string }[] = [
  {
    code: "update_restriction_missing",
    blocks_merge: false,
    passed: "A rule restricts the updates of `{branch}`",
    failed: "No rule restricts the updates of `{branch}`",
    change:
      "add a ruleset that restricts the updates of `{branch}`, with the App of CIHub in its bypass list, in exempt mode: without it, a PR can merge without the queue",
  },
  {
    code: "app_not_exempt",
    blocks_merge: true,
    passed: "The App of CIHub may update `{branch}`",
    failed: "A rule restricts the updates of `{branch}` without an exemption for the App",
    change: "add the App of CIHub to the bypass list of the ruleset, in exempt mode",
  },
  {
    code: "test_branches_not_excluded",
    blocks_merge: true,
    passed: "No rule covers the test branches of CIHub",
    failed: "A rule covers `cihub-temp/**` or `cihub-merge/**`",
    change: "exclude `cihub-temp/**/*` and `cihub-merge/**/*` from every ruleset",
  },
  {
    code: "up_to_date_required",
    blocks_merge: false,
    passed: "`{branch}` asks for no branch up to date",
    failed: "`{branch}` asks for branches up to date",
    change:
      'turn off "Require branches to be up to date before merging" on `{branch}`, unless the queue merges directly',
  },
];

/** The rules of the default branch `branch`, where `failed` fail. */
function rulesOf(branch: string, failed: string[]): RuleCheck[] {
  return RULES.map((rule) => {
    const passed = !failed.includes(rule.code);
    return {
      code: rule.code,
      passed,
      blocks_merge: rule.blocks_merge,
      title: (passed ? rule.passed : rule.failed).replaceAll("{branch}", branch),
      message: passed ? null : rule.change.replaceAll("{branch}", branch),
    };
  });
}

/** The repository `name` of `org` and its setup: `start` without a queue, else its queue on `main`. */
function repoOf(org: string, name: string, isAdmin: boolean, start: Start | undefined): RepoDetail {
  const state = start?.state ?? "started";
  const branch = start?.branch ?? "main";
  return {
    id: `repo_${name}`,
    namespace: org,
    name,
    default_branch: branch,
    permission: { read: true, write: true, admin: isAdmin },
    setup: {
      state,
      queue: state === "started" ? "main" : null,
      rules: rulesOf(branch, start?.failed ?? []),
      target_branch_missing: false,
      rules_url: `https://github.com/${org}/${name}/settings/rules`,
      config: {
        state: "none",
        path: null,
        sha: "9f2c4e1",
        error: null,
        line: null,
        column: null,
        text: null,
        entries: [],
        strategy: "pipelined",
        settings: {
          batching_enabled: false,
          batching_size: 1,
          batching_max_wait_minutes: 0,
          failure_budget_retries: 1,
          merge_method: "squash",
        },
        ...start?.config,
      },
      checked_at: ago(4),
      requested_at: null,
    },
  };
}

/** The repositories of `org` that `person` reads, by name, with their start; `undefined` for another organization. */
function reposOf(
  org: string,
  person: Person,
): Record<string, { start: Start | undefined; isAdmin: boolean }> | undefined {
  const orgs = person === "nobody" ? [] : PEOPLE[person].orgs;
  if (org === "acme-labs") {
    // A member of acme-labs created its repositories, and administers them.
    const isMember = orgs.some((membership) => membership.slug === "acme-labs");
    return Object.fromEntries(
      Object.entries(LABS).map(([name, start]) => [name, { start, isAdmin: isMember }]),
    );
  }
  if (org !== "acme") {
    return undefined;
  }

  // An admin of the organization administers its repositories, a member writes to them.
  const isAdmin = orgs.some((membership) => membership.slug === "acme" && membership.role === "admin");
  const names = [...new Set([...QUEUES.map((queue) => queue.repo), ...Object.keys(WITHOUT_QUEUE)])].sort();
  return Object.fromEntries(
    names.map((name) => {
      const state = WITHOUT_QUEUE[name];
      const start = state && { state, failed: state === "cannot_merge" ? ["app_not_exempt"] : [] };
      return [name, { start, isAdmin }];
    }),
  );
}

function answerOfRepo(path: string, person: Person): Answer | null {
  const match = /^\/orgs\/([^/]+)\/repos(?:\/([^/]+))?$/.exec(path);
  const org = match?.[1] ?? "";
  const repos = reposOf(org, person);
  if (!match || !repos) {
    return null;
  }

  const name = match[2];
  if (name === undefined) {
    const list = Object.entries(repos).map(([each, { start, isAdmin }]) => {
      const { setup, ...repo } = repoOf(org, each, isAdmin, start);
      return { ...repo, setup: { state: setup.state, checked_at: setup.checked_at, requested_at: null } };
    });
    return [200, listOf("repos", list)];
  }
  const found = repos[name];
  return found ? [200, repoOf(org, name, found.isAdmin, found.start)] : [404, problem(404, "repo_not_found")];
}

function answerOfRun(path: string): Answer | null {
  const match = /^\/orgs\/acme\/repos\/[^/]+\/runs\/(.+)$/.exec(path);
  if (!match) {
    return null;
  }

  const run = RUNS[match[1] ?? ""];
  return run ? [200, run] : [404, problem(404, "run_not_found")];
}

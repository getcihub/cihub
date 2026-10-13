import { screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { GraphRun, Pr } from "@/client/types.gen";
import { sinceOf } from "@/domain/queue/queries";
import { type Answers, problem, renderApp } from "@/test/app";
import {
  ada,
  answersOfMe,
  answersOfMine,
  detailOf,
  itemOf,
  liveOf,
  noGraph,
  pageOf,
  queueOf,
} from "@/test/fixtures";

const QUEUE = "/api/v1/orgs/acme/repos/web/queues/main";
const LIVE = liveOf();
const entry = queueOf("web", "main");

const nodeOf = ({ id, number, state, position }: Pr) => ({ item_id: id, pr_number: number, state, position });

function runOf(item: Pr, startedAt: string, predecessors: Pr[] = []): GraphRun {
  return {
    run_id: `run_${String(item.number)}`,
    item_ids: [item.id],
    predecessor_ids: predecessors.map((predecessor) => predecessor.id),
    attempt: 1,
    status: "in_progress",
    started_at: startedAt,
  };
}

function answersFor(changes: Answers): Answers {
  return {
    ...answersOfMe(ada),
    ...answersOfMine("acme"),
    "/api/v1/orgs/acme/queues": [200, pageOf("queues", [entry])],
    [QUEUE]: [200, detailOf(entry)],
    [`${QUEUE}/graph`]: [200, noGraph],
    [LIVE]: [200, pageOf("prs", [])],
    ...changes,
  };
}

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("the page of a queue", () => {
  it("shows its counters, its settings, its graph and its items in their order", async () => {
    const testing = itemOf(118, "testing", {
      position: 1000,
      run_id: "run_1",
      title: "Export invoices as CSV",
    });
    const pending = itemOf(124, "pending", { position: 2000, priority: 10, author: "lea" });
    const waiting = itemOf(125, "not_ready", { state_description: "a required check is in progress" });
    const detail = detailOf(entry, {
      counts: { not_ready: 1, pending: 1, testing: 1, pending_failure: 0, tests_passed: 0 },
      live_runs: 1,
      merged_last_24h: 3,
    });
    const graph = {
      target_head_sha: "9c1e2a4f00",
      nodes: [testing, pending, waiting].map(nodeOf),
      edges: [],
      runs: [
        {
          run_id: "run_1",
          item_ids: [testing.id],
          predecessor_ids: [],
          attempt: 1,
          status: "in_progress",
          started_at: new Date().toISOString(),
        },
      ],
    };
    renderApp(
      "/orgs/acme/repos/web/queues/main",
      answersFor({
        [QUEUE]: [200, detail],
        [`${QUEUE}/graph`]: [200, graph],
        [LIVE]: [200, pageOf("prs", [testing, pending, waiting])],
      }),
    );

    expect(await screen.findByRole("heading", { level: 1, name: /main/ })).toBeInTheDocument();
    expect(screen.getByText("1 of them waiting to enter")).toBeInTheDocument();
    expect(screen.getByText("pull requests in 1 run")).toBeInTheDocument();
    expect(screen.getByText("Pipelined")).toBeInTheDocument();
    expect(screen.getByText("2 now · 2 to 20")).toBeInTheDocument();
    expect(screen.getByText("1 retry")).toBeInTheDocument();

    const run = screen.getByRole("group", { name: "Run 1" });
    expect(within(run).getByText("Export invoices as CSV")).toBeInTheDocument();
    expect(within(run).getByText("Testing · attempt 1")).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Branch main" })).toHaveTextContent("9c1e2a4");

    // The cell of a PR starts with the letter of the avatar of its author.
    const rows = within(screen.getByRole("table")).getAllByRole("row").slice(1);
    expect(rows.map((row) => within(row).getAllByRole("cell")[1]?.textContent)).toEqual([
      "MExport invoices as CSV#118 · marie",
      "LChange 124#124 · lea",
      "MChange 125#125 · marie",
    ]);
    expect(rows.map((row) => within(row).getAllByRole("cell")[0]?.textContent)).toEqual(["1", "2", "–"]);
    expect(rows[1]).toHaveTextContent("high");
    expect(rows[2]).toHaveTextContent("a required check is in progress");
  });

  it("puts the run of the head of the queue next to the branch, even when it started again last", async () => {
    const head = itemOf(1, "testing", { position: 1000, title: "Head of the queue" });
    const next = itemOf(2, "testing", { position: 2000, title: "Second in line" });
    const graph = {
      ...noGraph,
      nodes: [head, next].map(nodeOf),
      runs: [runOf(next, "2026-10-11T10:00:00Z", [head]), runOf(head, "2026-10-11T10:05:00Z")],
    };
    renderApp(
      "/orgs/acme/repos/web/queues/main",
      answersFor({
        [QUEUE]: [200, detailOf(entry, { counts: { ...entry.counts, testing: 2 } })],
        [`${QUEUE}/graph`]: [200, graph],
        [LIVE]: [200, pageOf("prs", [head, next])],
      }),
    );

    expect(await screen.findByRole("group", { name: "Run 1" })).toHaveTextContent("Head of the queue");
    expect(screen.getByRole("group", { name: "Run 2" })).toHaveTextContent("Second in line");
  });

  it("says who paused it, and what the configuration of the SCM misses", async () => {
    const finding = {
      code: "up_to_date_required",
      message: "the rules of `main` ask for branches up to date",
    };
    const paused = detailOf(entry, {
      state: "paused",
      state_changed_by: "hugo",
      config_check: { findings: [finding], checked_at: new Date().toISOString() },
    });
    renderApp("/orgs/acme/repos/web/queues/main", answersFor({ [QUEUE]: [200, paused] }));

    expect(await screen.findByText(/^Paused by @hugo/)).toBeInTheDocument();
    expect(screen.getByText(finding.message)).toBeInTheDocument();
  });

  it("says that a lost access stops it, rather than a pause", async () => {
    const lost = detailOf(entry, { state: "paused", access: "suspended" });
    renderApp("/orgs/acme/repos/web/queues/main", answersFor({ [QUEUE]: [200, lost] }));

    expect(await screen.findByText("CIHub lost its access to acme/web.")).toBeInTheDocument();
    expect(screen.queryByText(/^Paused/)).not.toBeInTheDocument();
  });

  it("says that a queue with nothing in it and nothing in the last 24 hours is empty", async () => {
    renderApp("/orgs/acme/repos/web/queues/main", answersFor({}));

    expect(await screen.findByRole("heading", { name: "No pull request in the queue" })).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  it("opens the tab of the URL, with the items that ended in the last 24 hours", async () => {
    // The read of the tab starts at a full hour: the test and the page compute the same one.
    vi.useFakeTimers({ now: Date.parse("2026-10-11T12:30:00Z"), toFake: ["Date"] });
    const merged = itemOf(116, "merged", { title: "Add the audit log page" });
    const old = itemOf(90, "merged", { state_changed_at: "2026-01-01T00:00:00Z" });
    renderApp(
      "/orgs/acme/repos/web/queues/main?tab=merged",
      answersFor({
        [QUEUE]: [200, detailOf(entry, { merged_last_24h: 1 })],
        [`/api/v1/orgs/acme/prs?repo=web&queue=main&state=merged&state_changed_since=${sinceOf(Date.now())}&limit=100`]:
          [200, pageOf("prs", [merged, old])],
      }),
    );

    expect(await screen.findByText("Add the audit log page")).toBeInTheDocument();
    expect(screen.queryByText("Change 90")).not.toBeInTheDocument();
    expect(screen.getByRole("link", { name: /Merged/ })).toHaveAttribute("aria-current", "page");
  });

  it("keeps its head when a read of the page fails, and says what the server answered", async () => {
    renderApp(
      "/orgs/acme/repos/web/queues/main",
      answersFor({ [`${QUEUE}/graph`]: [503, problem(503, "internal")] }),
    );

    expect(
      await screen.findByRole("heading", { level: 3, name: "CIHub could not load this page" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 1, name: "main" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Submit a pull request" })).toBeInTheDocument();
  });

  it("sends a person whose session ended to the sign-in, then back to the page", async () => {
    const ended = problem(401, "unauthorized");
    const router = renderApp(
      "/orgs/acme/repos/web/queues/main",
      answersFor({
        // The page reads the person, then the session ends as it reads the graph.
        "/api/v1/user": [
          [200, { login: ada.login }],
          [401, ended],
        ],
        [`${QUEUE}/graph`]: [401, ended],
      }),
    );

    await waitFor(() => {
      expect(router.state.location.pathname).toBe("/sign-in");
    });
    expect(router.state.location.search).toEqual({ next: "/orgs/acme/repos/web/queues/main" });
  });

  it("shows a 401 of a read as a failure while the session still holds, never in a loop", async () => {
    const router = renderApp(
      "/orgs/acme/repos/web/queues/main",
      answersFor({ [`${QUEUE}/graph`]: [401, problem(401, "unauthorized")] }),
    );

    expect(await screen.findByText("401 unauthorized")).toBeInTheDocument();
    expect(router.state.location.pathname).toBe("/orgs/acme/repos/web/queues/main");
  });
});

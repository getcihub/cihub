import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { GraphRun, HealthBucket, Pr, QueueHealth } from "@/client/types.gen";
import { sinceOf } from "@/domain/queue/queries";
import { type Answers, mockSessionCookie, problem, renderApp } from "@/test/app";
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
  repoOf,
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
    "/api/v1/orgs/acme/repos/web": [200, repoOf("web")],
    [QUEUE]: [200, detailOf(entry)],
    [`${QUEUE}/graph`]: [200, noGraph],
    [LIVE]: [200, pageOf("prs", [])],
    ...changes,
  };
}

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

async function emptyQueue(): Promise<HTMLElement> {
  const title = await screen.findByRole("heading", { name: "No pull request in the queue" });
  const empty = title.closest<HTMLElement>("[data-slot=empty-state]");
  if (!empty) throw new Error("the empty state holds its title");
  return empty;
}

describe("the page of a queue", () => {
  it("shows its counters, its settings, its graph and its items in their order", async () => {
    const testing = itemOf(118, "testing", {
      position: 1000,
      run_id: "run_1",
      title: "Export invoices as CSV",
    });
    const pending = itemOf(124, "pending", {
      position: 2000,
      priority: 10,
      profile: "dependencies",
      author: { login: "lea", avatar_url: "https://avatars.example.com/u/3" },
    });
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

    // The cell of a PR starts with the avatar of its author: their image, else the letter of their login.
    const rows = within(screen.getByRole("table")).getAllByRole("row").slice(1);
    expect(rows.map((row) => within(row).getAllByRole("cell")[1]?.textContent)).toEqual([
      "MExport invoices as CSV#118 · marie",
      "Change 124#124 · lea",
      "MChange 125#125 · marie",
    ]);
    expect(rows[1]?.querySelector("img[data-slot=avatar]")).toHaveAttribute(
      "src",
      "https://avatars.example.com/u/3",
    );
    expect(rows.map((row) => within(row).getAllByRole("cell")[0]?.textContent)).toEqual(["1", "2", "–"]);
    expect(rows[1]).toHaveTextContent("high · dependencies");
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
      setup: { findings: [finding], checked_at: new Date().toISOString() },
    });
    renderApp("/orgs/acme/repos/web/queues/main", answersFor({ [QUEUE]: [200, paused] }));

    expect(await screen.findByText(/^Paused by @hugo/)).toBeInTheDocument();
    expect(screen.getByText(finding.message)).toBeInTheDocument();
  });

  it("says that a lost access stops it, rather than a pause", async () => {
    const lost = detailOf(entry, { state: "running", is_suspended: true });
    renderApp("/orgs/acme/repos/web/queues/main", answersFor({ [QUEUE]: [200, lost] }));

    expect(await screen.findByText("CIHub lost its access to acme/web.")).toBeInTheDocument();
    expect(screen.queryByText(/^Paused/)).not.toBeInTheDocument();
  });

  it("keeps the pause of an admin in view while the access is lost", async () => {
    const lost = detailOf(entry, { state: "paused", state_changed_by: "hugo", is_suspended: true });
    renderApp("/orgs/acme/repos/web/queues/main", answersFor({ [QUEUE]: [200, lost] }));

    expect(await screen.findByText("CIHub lost its access to acme/web.")).toBeInTheDocument();
    expect(screen.getByText(/^Paused by @hugo/)).toBeInTheDocument();
  });

  it("says how a pull request enters a queue with nothing in it and nothing in the last 24 hours", async () => {
    renderApp("/orgs/acme/repos/web/queues/main", answersFor({}));
    const empty = await emptyQueue();

    expect(empty).toHaveTextContent("Nothing waits in the queue of main, and nothing merged here yet.");
    expect(empty).toHaveTextContent("Or add the label cihub-merge-queue-submit to the pull request.");
    expect(within(empty).getByRole("button", { name: "Submit a pull request" })).toBeEnabled();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByRole("navigation", { name: "Tabs of the pull requests" })).not.toBeInTheDocument();
  });

  it("lets no one without write on the repository submit, in the head nor in the empty queue", async () => {
    const reader = repoOf("web", { read: true, write: false, admin: false });
    renderApp(
      "/orgs/acme/repos/web/queues/main",
      answersFor({ "/api/v1/orgs/acme/repos/web": [200, reader] }),
    );
    await emptyQueue();

    for (const submit of screen.getAllByRole("button", { name: "Submit a pull request" })) {
      expect(submit).toBeDisabled();
    }
    expect(screen.getAllByRole("button", { name: "Submit a pull request" })).toHaveLength(2);
  });

  it("names no label when the labels of the queue do not submit", async () => {
    const queue = detailOf(entry, { settings: { ...entry.settings, enqueue_label: null } });
    renderApp("/orgs/acme/repos/web/queues/main", answersFor({ [QUEUE]: [200, queue] }));
    const empty = await emptyQueue();

    expect(empty).toHaveTextContent("/cihub merge");
    expect(empty).not.toHaveTextContent("label");
  });

  it("copies the command that submits a pull request", async () => {
    // `setup` keeps its clipboard for the whole test: the page writes to it after the click.
    const user = userEvent.setup();
    renderApp("/orgs/acme/repos/web/queues/main", answersFor({}));
    const empty = await emptyQueue();

    await user.click(within(empty).getByRole("button", { name: "Copy" }));

    expect(await within(empty).findByRole("button", { name: "Copied" })).toBeInTheDocument();
    expect(screen.getByText("Copied /cihub merge to the clipboard.")).toBeInTheDocument();
    expect(await navigator.clipboard.readText()).toBe("/cihub merge");
  });

  it("says that it copied the command for a moment only", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime.bind(vi) });
    renderApp("/orgs/acme/repos/web/queues/main", answersFor({}));
    const empty = await emptyQueue();

    await user.click(within(empty).getByRole("button", { name: "Copy" }));
    await within(empty).findByRole("button", { name: "Copied" });
    await vi.advanceTimersByTimeAsync(2_000);

    expect(await within(empty).findByRole("button", { name: "Copy" })).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.queryByText("Copied /cihub merge to the clipboard.")).not.toBeInTheDocument();
    });
  });

  it("says when the queue last merged, in the tab of an empty queue that merged today", async () => {
    const fortyMinutesAgo = new Date(Date.now() - 40 * 60 * 1000).toISOString();
    renderApp(
      "/orgs/acme/repos/web/queues/main",
      answersFor({
        [QUEUE]: [200, detailOf(entry, { merged_last_24h: 1, last_merged_at: fortyMinutesAgo })],
      }),
    );
    const empty = await emptyQueue();

    expect(empty).toHaveTextContent("Nothing waits in the queue of main; the last merge was 40 minutes ago.");
    expect(screen.getByRole("link", { name: /In the queue/ })).toHaveAttribute("aria-current", "page");
  });

  it("shows in its head what the empty queue sent", async () => {
    mockSessionCookie();
    renderApp(
      "/orgs/acme/repos/web/queues/main",
      answersFor({ "/api/v1/orgs/acme/repos/web/prs/124/submit": [202, null] }),
    );
    const empty = await emptyQueue();

    await userEvent.click(within(empty).getByRole("button", { name: "Submit a pull request" }));
    const dialog = screen.getByRole("dialog", { name: "Submit a pull request" });
    await userEvent.type(within(dialog).getByRole("textbox", { name: /Pull request number/ }), "124");
    await userEvent.click(within(dialog).getByRole("button", { name: "Submit" }));

    expect(await screen.findByText(/^#124 sent/)).toBeInTheDocument();
  });

  it("opens the tab of the URL, with the items that ended in the last 24 hours and the count of each tab", async () => {
    // The read of the tab starts at a full hour: the test and the page compute the same one.
    vi.useFakeTimers({ now: Date.parse("2026-10-11T12:30:00Z"), toFake: ["Date"] });
    const merged = itemOf(116, "merged", { title: "Add the audit log page" });
    const old = itemOf(90, "merged", { state_changed_at: "2026-01-01T00:00:00Z" });
    renderApp(
      "/orgs/acme/repos/web/queues/main?tab=merged",
      answersFor({
        [QUEUE]: [
          200,
          detailOf(entry, { merged_last_24h: 1, failed_last_24h: 4, failed_or_cancelled_last_24h: 3 }),
        ],
        [`/api/v1/orgs/acme/repos/web/prs?queue=main&state=merged&state_changed_since=${sinceOf(Date.now())}&limit=100`]:
          [200, pageOf("prs", [merged, old])],
      }),
    );

    expect(await screen.findByText("Add the audit log page")).toBeInTheDocument();
    expect(screen.queryByText("Change 90")).not.toBeInTheDocument();
    expect(screen.getByRole("link", { name: /Merged/ })).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("link", { name: /^Failed and cancelled\s*3$/ })).toBeInTheDocument();
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

function healthOf(changes: Partial<QueueHealth> = {}): QueueHealth {
  const bucket = (hour: number, merged: number, failed: number): HealthBucket => ({
    start: `2026-10-14T${String(hour).padStart(2, "0")}:00:00Z`,
    merged,
    failed,
    time_to_merge: merged > 0 ? { median_seconds: 1380, p90_seconds: 2940 } : null,
  });

  return {
    from: "2026-10-13T10:00:00Z",
    to: "2026-10-14T10:00:00Z",
    verdict: { kind: "check_passes_on_retry", check: "ci / test", times: 4 },
    current: {
      merged: 9,
      failed: 1,
      cancelled: 1,
      time_to_merge: { median_seconds: 1380, p90_seconds: 2940 },
    },
    previous: {
      merged: 6,
      failed: 1,
      cancelled: 0,
      time_to_merge: { median_seconds: 1500, p90_seconds: 3000 },
    },
    buckets: [bucket(8, 4, 1), bucket(9, 5, 0)],
    time_in_states: [
      { state: "not_ready", median_seconds: 300 },
      { state: "pending", median_seconds: 180 },
      { state: "testing", median_seconds: 840 },
      { state: "pending_failure", median_seconds: 0 },
      { state: "tests_passed", median_seconds: 60 },
    ],
    left_without_merge: [
      {
        conclusion: "failed",
        reason: "test_run_required_status_failed",
        description: "a required check failed",
        count: 1,
      },
      { conclusion: "cancelled", reason: "pr_closed", description: "the PR was closed", count: 1 },
    ],
    checks: [
      { name: "ci / lint", kind: "check", required: false, runs: 12, failed: 0, passed_on_retry: 0 },
      { name: "ci / test", kind: "check", required: true, runs: 14, failed: 5, passed_on_retry: 4 },
    ],
    ...changes,
  };
}

describe("the Health of a queue", () => {
  it("says what the queue did, against the period before, and which check looks flaky", async () => {
    renderApp(
      "/orgs/acme/repos/web/queues/main?pane=health",
      answersFor({ [`${QUEUE}/health?range=24h`]: [200, healthOf()] }),
    );

    expect(await screen.findByText("looks flaky")).toBeInTheDocument();
    expect(screen.getByText(/passed on a retry, 4 times in the last 24 hours/)).toBeInTheDocument();
    expect(screen.getByText("6 in the previous 24 hours")).toBeInTheDocument();
    expect(screen.getByText("23 min")).toBeInTheDocument();
    expect(screen.getByText("A required check failed")).toBeInTheDocument();
    expect(screen.getByText("The PR was closed")).toBeInTheDocument();
    const test = screen.getByRole("row", { name: /ci \/ test/ });
    expect(within(test).getByText("flaky")).toBeInTheDocument();
    expect(
      within(screen.getByRole("row", { name: /ci \/ lint/ })).getByText("not required"),
    ).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Health" })).toHaveAttribute("aria-current", "page");
    expect(screen.queryByRole("navigation", { name: "Tabs of the pull requests" })).not.toBeInTheDocument();
  });

  it("offers a longer period when nothing ended in this one", async () => {
    const quiet = healthOf({
      verdict: { kind: "quiet" },
      current: { merged: 0, failed: 0, cancelled: 0, time_to_merge: null },
    });
    renderApp(
      "/orgs/acme/repos/web/queues/main?pane=health&range=7d",
      answersFor({ [`${QUEUE}/health?range=7d`]: [200, quiet] }),
    );

    expect(await screen.findByText("Nothing merged or failed in the last 7 days.")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Show the last 30 days" })).toBeInTheDocument();
  });

  it("shows a period that the person chose, in its own words", async () => {
    vi.useFakeTimers({ now: Date.parse("2026-10-15T09:41:00Z"), toFake: ["Date"] });
    const period = "from=2026-09-12T00:00:00.000Z&to=2026-10-04T00:00:00.000Z";
    const chosen = healthOf({ from: "2026-09-12T00:00:00Z", to: "2026-10-04T00:00:00Z" });
    renderApp(
      `/orgs/acme/repos/web/queues/main?pane=health&${period}`,
      answersFor({ [`${QUEUE}/health?${period}`]: [200, chosen] }),
    );

    expect(await screen.findByText(/passed on a retry, 4 times over Sep 12 – Oct 3/)).toBeInTheDocument();
    expect(screen.getByText("6 in the 22 days before")).toBeInTheDocument();
    expect(screen.getByText("On the test runs of the queue, Sep 12 – Oct 3")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Sep 12 – Oct 3" })).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("link", { name: "24 hours" })).not.toHaveAttribute("aria-current");
  });

  it("applies a period of the calendar, and closes it on Escape", async () => {
    vi.useFakeTimers({ now: Date.parse("2026-10-15T09:41:00Z"), toFake: ["Date"] });
    const lastWeek = "from=2026-10-05T00:00:00.000Z&to=2026-10-12T00:00:00.000Z";
    const router = renderApp(
      "/orgs/acme/repos/web/queues/main?pane=health",
      answersFor({
        [`${QUEUE}/health?range=24h`]: [200, healthOf()],
        [`${QUEUE}/health?${lastWeek}`]: [
          200,
          healthOf({ from: "2026-10-05T00:00:00Z", to: "2026-10-12T00:00:00Z" }),
        ],
      }),
    );

    await userEvent.click(await screen.findByRole("button", { name: "Custom" }));
    const calendar = screen.getByRole("dialog", { name: "Choose a period" });
    await userEvent.click(within(calendar).getByRole("button", { name: "Last week" }));

    expect(within(calendar).getByText(/compared with Sep 28 – Oct 4/)).toHaveTextContent(
      "7 days · charts by day",
    );
    expect(within(calendar).getByRole("button", { name: "Next month" })).toBeDisabled();

    await userEvent.click(within(calendar).getByRole("button", { name: "Apply" }));

    await waitFor(() => {
      expect(router.state.location.search).toMatchObject({
        pane: "health",
        from: "2026-10-05T00:00:00.000Z",
        to: "2026-10-12T00:00:00.000Z",
      });
    });

    await userEvent.click(await screen.findByRole("button", { name: "Oct 5 – Oct 11" }));
    expect(screen.getByRole("dialog", { name: "Choose a period" })).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Choose a period" })).not.toBeInTheDocument();
    });
  });
});

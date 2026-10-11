import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { type Answers, problem, renderApp } from "../test/app";
import { ada, detailOf, itemOf, noGraph, queueOf } from "../test/fixtures";

const QUEUE = "/api/v1/orgs/acme/repos/web/queues/main";
const LIVE = `${QUEUE}/prs?state=not_ready,pending,testing,pending_failure,tests_passed&take=100`;
const entry = queueOf("web", "main");
const testing = itemOf(118, "testing", {
  position: 1000,
  run_id: "run_1",
  title: "Export invoices as CSV",
});

function answersFor(changes: Answers): Answers {
  return {
    "/api/v1/me": [200, ada],
    "/api/v1/orgs/acme/prs": [200, { prs: [] }],
    "/api/v1/orgs/acme/queues": [200, { queues: [entry] }],
    [QUEUE]: [200, detailOf(entry)],
    [`${QUEUE}/graph`]: [200, noGraph],
    [`${QUEUE}/config-check`]: [200, { findings: [], checked_at: null }],
    [LIVE]: [200, { items: [testing] }],
    ...changes,
  };
}

const run = {
  id: "run_1",
  queue_id: "queue_web_main",
  group_id: "group_1",
  attempt: 2,
  status: "in_progress",
  status_reason: "test_run_started",
  test_branch: "cihub-merge/main/run_1",
  test_sha: "abc",
  base_sha: "9c1e2a4f00",
  draft_pr_number: 4521,
  tested_pr_numbers: [118],
  head_shas: ["abc"],
  predecessor_pr_numbers: [117],
  required_statuses: ["ci"],
  required_statuses_source: "config",
  started_at: "2026-10-11T12:04:00Z",
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
      observed_at: "2026-10-11T12:08:00Z",
    },
    {
      name: "lint",
      kind: "check",
      required: false,
      conclusion: null,
      details_url: null,
      observed_at: "2026-10-11T12:05:00Z",
    },
  ],
  impacted_targets: null,
  dependents: [],
};

const timeline = [
  {
    id: "event_1",
    seq: 1,
    type: "item.readiness_changed",
    actor_kind: "system",
    actor: "marie",
    occurred_at: "2026-10-11T12:00:00Z",
    description: "waits: the required check `ci` is in_progress",
    payload: {},
  },
  {
    id: "event_2",
    seq: 2,
    type: "item.transitioned",
    actor_kind: "system",
    actor: null,
    occurred_at: "2026-10-11T12:04:00Z",
    description: "a test run started",
    payload: { to: "testing" },
  },
];

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("the drawer of a PR", () => {
  it("shows where the PR is, its run, its checks and its history", async () => {
    renderApp(
      "/orgs/acme/repos/web/queues/main/prs/118",
      answersFor({
        [`${QUEUE}/prs/118?include=timeline`]: [200, { item: testing, timeline }],
        "/api/v1/orgs/acme/runs/run_1": [200, run],
      }),
    );

    const drawer = await screen.findByRole("dialog", { name: "Export invoices as CSV" });

    expect(drawer).toHaveTextContent("#118 · marie · position 1");
    expect(
      await within(drawer).findByRole("heading", { name: "Run · attempt 2 · Testing" }),
    ).toBeInTheDocument();
    expect(within(drawer).getByText("#4521 · runs your CI")).toBeInTheDocument();
    expect(drawer).toHaveTextContent("On top of#117");
    expect(within(drawer).getByText("not required")).toBeInTheDocument();
    expect(within(drawer).getByText("ci", { selector: "code" })).toBeInTheDocument();
    expect(drawer).toHaveTextContent("· by @marie");
    expect(within(drawer).getAllByText("Testing").length).toBeGreaterThan(0);
  });

  it("says that a PR is not in the queue, over the page of the queue", async () => {
    renderApp(
      "/orgs/acme/repos/web/queues/main/prs/999",
      answersFor({ [`${QUEUE}/prs/999?include=timeline`]: [404, problem(404, "not_found")] }),
    );

    expect(await screen.findByRole("heading", { name: "#999 is not in this queue" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 1, name: "main" })).toBeInTheDocument();
  });

  it("closes with Escape, back to the page of the queue", async () => {
    const router = renderApp(
      "/orgs/acme/repos/web/queues/main/prs/118",
      answersFor({
        [`${QUEUE}/prs/118?include=timeline`]: [200, { item: testing, timeline }],
        "/api/v1/orgs/acme/runs/run_1": [200, run],
      }),
    );

    await screen.findByRole("dialog");
    await userEvent.keyboard("{Escape}");

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(router.state.location.pathname).toBe("/orgs/acme/repos/web/queues/main");
  });
});

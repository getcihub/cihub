import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { type Answers, problem, renderApp } from "@/test/app";
import { answersOfQueuePage, itemOf, pageOf } from "@/test/fixtures";

const PR = "/api/v1/orgs/acme/repos/web/prs";
const testing = itemOf(118, "testing", {
  position: 1000,
  run_id: "run_1",
  title: "Export invoices as CSV",
});

function answersFor(changes: Answers): Answers {
  return { ...answersOfQueuePage([testing]), ...changes };
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
  draft_pr_url: "https://scm.example/acme/web/pull/4521",
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
      status: "completed",
      started_at: "2026-10-11T12:04:00Z",
      completed_at: "2026-10-11T12:26:00Z",
      details_url: null,
      observed_at: "2026-10-11T12:08:00Z",
    },
    {
      name: "lint",
      kind: "check",
      required: false,
      conclusion: null,
      status: "in_progress",
      started_at: "2026-10-11T12:05:00Z",
      completed_at: null,
      details_url: null,
      observed_at: "2026-10-11T12:05:00Z",
    },
    {
      name: "e2e",
      kind: "check",
      required: true,
      conclusion: null,
      status: "queued",
      started_at: null,
      completed_at: null,
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
    description: "the required check `ci` is running",
    payload: { block_reasons: [{ kind: "awaiting_required_check", check: "ci", status: "running" }] },
  },
  {
    id: "event_2",
    seq: 2,
    type: "item.transitioned",
    actor_kind: "system",
    actor: null,
    occurred_at: "2026-10-11T12:04:00Z",
    description: "a test run started",
    payload: { from: "pending", to: "testing" },
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
        [`${PR}/118`]: [200, testing],
        [`${PR}/118/events?limit=100`]: [200, pageOf("events", timeline)],
        "/api/v1/orgs/acme/repos/web/runs/run_1": [200, run],
      }),
    );

    const drawer = await screen.findByRole("dialog", { name: "Export invoices as CSV" });

    expect(drawer).toHaveTextContent("#118 · marie · position 1");
    expect(
      await within(drawer).findByRole("heading", { name: "Run · attempt 2 · Testing" }),
    ).toBeInTheDocument();
    expect(drawer).toHaveTextContent("Draft PR#4521 · runs your CI");
    expect(drawer).toHaveTextContent("On top of#117");
    expect(within(drawer).getByText("not required")).toBeInTheDocument();
    expect(within(drawer).getByText("ci", { selector: "code" })).toBeInTheDocument();
    expect(drawer).toHaveTextContent("by @marie");

    const history = within(drawer).getByRole("region", { name: "History" });
    // A blocked event holds its reasons in a list of its own.
    const events = [...(within(history).getAllByRole("list")[0]?.children ?? [])] as HTMLElement[];

    expect(events.map((event) => within(event).getByText(/^(Testing|Blocked)$/).textContent)).toEqual([
      "Testing",
      "Blocked",
    ]);
  });

  it("links the PR and its draft PR to their pages on the SCM", async () => {
    renderApp(
      "/orgs/acme/repos/web/queues/main/prs/118",
      answersFor({
        [`${PR}/118`]: [200, { ...testing, url: "https://scm.example/acme/web/pull/118" }],
        [`${PR}/118/events?limit=100`]: [200, pageOf("events", timeline)],
        "/api/v1/orgs/acme/repos/web/runs/run_1": [200, run],
      }),
    );

    const drawer = await screen.findByRole("dialog", { name: "Export invoices as CSV" });
    const pr = within(drawer).getByRole("link", { name: "#118" });
    const draft = await within(drawer).findByRole("link", { name: "#4521" });

    expect(pr).toHaveAttribute("href", "https://scm.example/acme/web/pull/118");
    expect(pr).toHaveAttribute("target", "_blank");
    expect(pr).toHaveAttribute("rel", "noreferrer");
    expect(draft).toHaveAttribute("href", "https://scm.example/acme/web/pull/4521");
    expect(draft).toHaveAttribute("rel", "noreferrer");
  });

  it("shows no link without the page of the PR or of its draft PR", async () => {
    renderApp(
      "/orgs/acme/repos/web/queues/main/prs/118",
      answersFor({
        [`${PR}/118`]: [200, { ...testing, url: null }],
        [`${PR}/118/events?limit=100`]: [200, pageOf("events", timeline)],
        "/api/v1/orgs/acme/repos/web/runs/run_1": [200, { ...run, draft_pr_url: null }],
      }),
    );

    const drawer = await screen.findByRole("dialog", { name: "Export invoices as CSV" });

    expect(await within(drawer).findByText("#4521 · runs your CI")).toBeInTheDocument();
    expect(within(drawer).queryByRole("link", { name: /^#\d+$/ })).not.toBeInTheDocument();
  });

  it("tells a queued check, a running check and a finished check with its duration", async () => {
    renderApp(
      "/orgs/acme/repos/web/queues/main/prs/118",
      answersFor({
        [`${PR}/118`]: [200, testing],
        [`${PR}/118/events?limit=100`]: [200, pageOf("events", timeline)],
        "/api/v1/orgs/acme/repos/web/runs/run_1": [200, run],
      }),
    );

    const drawer = await screen.findByRole("dialog", { name: "Export invoices as CSV" });
    const checks = await within(drawer).findByRole("region", { name: "Checks" });
    const line = (name: string) => within(checks).getByText(name).closest("li");

    expect(line("build")).toHaveTextContent("passed in 22 min");
    expect(line("lint")).toHaveTextContent(/running since/);
    expect(line("e2e")).toHaveTextContent("queued");
  });

  it("shows what blocks a PR that is not ready, in the drawer and in its history", async () => {
    const reasons = [
      { kind: "awaiting_required_check", check: "ci", status: "running" },
      {
        kind: "queue_conditions_unmet",
        conditions: ["labels.none: wip", "any: [draft: false; author: bot]"],
      },
    ];
    const blocked = itemOf(133, "not_ready", { title: "Update the install guide", block_reasons: reasons });
    const history = [
      {
        id: "event_1",
        seq: 1,
        type: "item.readiness_changed",
        actor_kind: "system",
        actor: null,
        occurred_at: "2026-10-11T12:00:00Z",
        description: "the required check `ci` is running; the queue conditions … are not met",
        payload: { block_reasons: reasons },
      },
    ];
    renderApp(
      "/orgs/acme/repos/web/queues/main/prs/133",
      answersFor({
        [`${PR}/133`]: [200, blocked],
        [`${PR}/133/events?limit=100`]: [200, pageOf("events", history)],
      }),
    );

    const drawer = await screen.findByRole("dialog", { name: "Update the install guide" });
    const blocks = within(drawer).getByRole("region", { name: "What blocks it" });
    const event = within(drawer).getByRole("region", { name: "History" });

    expect(within(drawer).getByRole("listitem", { current: "step" })).toHaveTextContent("Not ready");
    expect(blocks).toHaveTextContent("The required check ci is running");
    for (const place of [blocks, event]) {
      const conditions = within(place).getByRole("group", { name: "Unmet queue conditions" });

      expect(conditions).toHaveTextContent("whereLabelshas none ofwip");
      expect(conditions).toHaveTextContent(
        "andorAt least one of these holds.Draftis notdraft: falseorAuthorisbot",
      );
    }
  });

  it("says that a PR is not in the queue, over the page of the queue", async () => {
    renderApp(
      "/orgs/acme/repos/web/queues/main/prs/999",
      answersFor({
        [`${PR}/999`]: [404, problem(404, "pr_not_found")],
        [`${PR}/999/events?limit=100`]: [404, problem(404, "pr_not_found")],
      }),
    );

    expect(await screen.findByRole("heading", { name: "#999 is not in this queue" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 1, name: "main" })).toBeInTheDocument();
  });

  it("closes with Escape, back to the page of the queue", async () => {
    const router = renderApp(
      "/orgs/acme/repos/web/queues/main/prs/118",
      answersFor({
        [`${PR}/118`]: [200, testing],
        [`${PR}/118/events?limit=100`]: [200, pageOf("events", timeline)],
        "/api/v1/orgs/acme/repos/web/runs/run_1": [200, run],
      }),
    );

    await screen.findByRole("dialog");
    await userEvent.keyboard("{Escape}");

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(router.state.location.pathname).toBe("/orgs/acme/repos/web/queues/main");
  });
});

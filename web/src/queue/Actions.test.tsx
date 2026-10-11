import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { Me } from "../client/types.gen";
import { type Answers, problem, renderApp } from "../test/app";
import { ada, detailOf, itemOf, noGraph, queueOf } from "../test/fixtures";

const QUEUE = "/api/v1/orgs/acme/repos/web/queues/main";
const LIVE = `${QUEUE}/prs?state=not_ready,pending,testing,pending_failure,tests_passed&take=100`;
const entry = queueOf("web", "main");
const testing = itemOf(118, "testing", { position: 1000, title: "Export invoices as CSV" });

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

/** The requests of the page that change a state: method, path and body. */
async function changes(): Promise<[string, string, unknown, string | null][]> {
  const calls = vi.mocked(fetch).mock.calls.map(([input]) => input as Request);
  const sent = calls.filter((request) => request.method !== "GET");
  return Promise.all(
    sent.map(async (request) => [
      request.method,
      new URL(request.url).pathname,
      JSON.parse((await request.clone().text()) || "null") as unknown,
      request.headers.get("X-CSRF-Token"),
    ]),
  );
}

beforeEach(() => {
  // The server sets this cookie; a script only reads it.
  vi.spyOn(document, "cookie", "get").mockReturnValue("__Host-cihub_csrf=token");
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("the actions on a queue", () => {
  it("let an admin pause the queue, with the CSRF token of the session, and say that it is sent", async () => {
    renderApp("/orgs/acme/repos/web/queues/main", answersFor({}));

    await userEvent.click(await screen.findByRole("button", { name: "Pause" }));
    const dialog = screen.getByRole("dialog", { name: "Pause the queue?" });
    await userEvent.click(within(dialog).getByRole("button", { name: "Pause" }));

    expect(await screen.findByRole("status")).toHaveTextContent("Pause sent");
    expect(await changes()).toEqual([["PATCH", QUEUE, { state: "paused" }, "token"]]);
  });

  it("let a member pause nothing", async () => {
    const member: Me = { ...ada, orgs: [{ slug: "acme", role: "member" }] };
    renderApp("/orgs/acme/repos/web/queues/main", answersFor({ "/api/v1/me": [200, member] }));

    expect(await screen.findByRole("button", { name: "Pause" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Pause" })).toHaveAttribute("title", "Needs the admin role");
    expect(screen.getByRole("button", { name: "Submit a pull request" })).toBeEnabled();
  });

  it("submit a PR with its priority, after the form checks its number", async () => {
    renderApp(
      "/orgs/acme/repos/web/queues/main",
      answersFor({ [`${QUEUE}/prs`]: [202, { accepted: true }] }),
    );

    await userEvent.click(await screen.findByRole("button", { name: "Submit a pull request" }));
    const dialog = screen.getByRole("dialog", { name: "Submit a pull request" });
    await userEvent.click(within(dialog).getByRole("button", { name: "Submit" }));

    expect(within(dialog).getByText("The number of a pull request: 1 or more.")).toBeInTheDocument();

    await userEvent.type(within(dialog).getByRole("textbox", { name: /Pull request number/ }), "124");
    await userEvent.selectOptions(within(dialog).getByRole("combobox", { name: "Priority" }), "high");
    await userEvent.click(within(dialog).getByRole("button", { name: "Submit" }));

    expect(await screen.findByRole("status")).toHaveTextContent("#124 sent");
    expect(await changes()).toEqual([
      ["POST", `${QUEUE}/prs`, { pr_number: 124, priority: "high", no_batch: false }, "token"],
    ]);
  });

  it("say why the queue refuses a submission at once", async () => {
    const refused = { ...problem(409, "invalid_state"), detail: "the PR is already in the queue, `testing`" };
    renderApp("/orgs/acme/repos/web/queues/main", answersFor({ [`${QUEUE}/prs`]: [409, refused] }));

    await userEvent.click(await screen.findByRole("button", { name: "Submit a pull request" }));
    const dialog = screen.getByRole("dialog", { name: "Submit a pull request" });
    await userEvent.type(within(dialog).getByRole("textbox", { name: /Pull request number/ }), "118");
    await userEvent.click(within(dialog).getByRole("button", { name: "Submit" }));

    expect(await within(dialog).findByRole("alert")).toHaveTextContent("the PR is already in the queue");
  });

  it("cancel a PR from its drawer, once confirmed", async () => {
    renderApp(
      "/orgs/acme/repos/web/queues/main/prs/118",
      answersFor({
        [`${QUEUE}/prs/118?include=timeline`]: [200, { item: testing, timeline: [] }],
        "/api/v1/orgs/acme/runs/run_1": [404, problem(404, "not_found")],
        [`${QUEUE}/prs/118:cancel`]: [202, { accepted: true }],
      }),
    );

    const drawer = await screen.findByRole("dialog", { name: "Export invoices as CSV" });
    await userEvent.click(within(drawer).getByRole("button", { name: "Cancel" }));
    await userEvent.click(screen.getByRole("button", { name: "Cancel the pull request" }));

    expect(await within(drawer).findByRole("status")).toHaveTextContent("Cancellation sent");
    expect(await changes()).toEqual([["POST", `${QUEUE}/prs/118:cancel`, null, "token"]]);
  });
});

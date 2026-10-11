import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { Me } from "../session";
import { type Answers, problem, renderApp } from "../test/app";
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
} from "../test/fixtures";

const QUEUE = "/api/v1/orgs/acme/repos/web/queues/main";
const PR = "/api/v1/orgs/acme/repos/web/prs";
const LIVE = liveOf();
const entry = queueOf("web", "main");
const testing = itemOf(118, "testing", { position: 1000, title: "Export invoices as CSV" });

function answersFor(changes: Answers): Answers {
  return {
    ...answersOfMe(ada),
    ...answersOfMine("acme"),
    "/api/v1/orgs/acme/queues": [200, pageOf("queues", [entry])],
    [QUEUE]: [200, detailOf(entry)],
    [`${QUEUE}/graph`]: [200, noGraph],
    [LIVE]: [200, pageOf("prs", [testing])],
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

    expect(await screen.findByText(/^Pause sent/)).toBeInTheDocument();
    expect(await changes()).toEqual([["PATCH", QUEUE, { state: "paused" }, "token"]]);
  });

  it("let a member pause nothing", async () => {
    const member: Me = { ...ada, orgs: [{ slug: "acme", role: "member" }] };
    renderApp("/orgs/acme/repos/web/queues/main", answersFor(answersOfMe(member)));

    expect(await screen.findByRole("button", { name: "Pause" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Pause" })).toHaveAttribute("title", "Needs the admin role");
    expect(screen.getByRole("button", { name: "Submit a pull request" })).toBeEnabled();
  });

  it("submit a PR with its priority, after the form checks its number", async () => {
    renderApp("/orgs/acme/repos/web/queues/main", answersFor({ [`${PR}/124/submit`]: [202, null] }));

    await userEvent.click(await screen.findByRole("button", { name: "Submit a pull request" }));
    const dialog = screen.getByRole("dialog", { name: "Submit a pull request" });
    await userEvent.click(within(dialog).getByRole("button", { name: "Submit" }));

    expect(within(dialog).getByText("The number of a pull request: 1 or more.")).toBeInTheDocument();

    await userEvent.type(within(dialog).getByRole("textbox", { name: /Pull request number/ }), "124");
    await userEvent.selectOptions(within(dialog).getByRole("combobox", { name: "Priority" }), "high");
    await userEvent.click(within(dialog).getByRole("button", { name: "Submit" }));

    expect(await screen.findByText(/^#124 sent/)).toBeInTheDocument();
    expect(await changes()).toEqual([
      ["POST", `${PR}/124/submit`, { queue: "main", priority: "high", no_batch: false }, "token"],
    ]);
  });

  it("say why the queue refuses a submission at once", async () => {
    const refused = {
      ...problem(409, "pr_already_queued"),
      detail: "the PR is already in the queue, `testing`",
    };
    renderApp("/orgs/acme/repos/web/queues/main", answersFor({ [`${PR}/118/submit`]: [409, refused] }));

    await userEvent.click(await screen.findByRole("button", { name: "Submit a pull request" }));
    const dialog = screen.getByRole("dialog", { name: "Submit a pull request" });
    await userEvent.type(within(dialog).getByRole("textbox", { name: /Pull request number/ }), "118");
    await userEvent.click(within(dialog).getByRole("button", { name: "Submit" }));

    expect(await within(dialog).findByRole("alert")).toHaveTextContent("the PR is already in the queue");
  });

  it("cancel a PR from its drawer, once confirmed; Escape closes the confirmation alone", async () => {
    renderApp(
      "/orgs/acme/repos/web/queues/main/prs/118",
      answersFor({
        [`${PR}/118`]: [200, testing],
        [`${PR}/118/events?limit=100`]: [200, pageOf("events", [])],
        "/api/v1/orgs/acme/runs/run_1": [404, problem(404, "run_not_found")],
        [`${PR}/118/cancel`]: [202, null],
      }),
    );

    const drawer = await screen.findByRole("dialog", { name: "Export invoices as CSV" });
    await userEvent.click(within(drawer).getByRole("button", { name: "Cancel" }));
    await userEvent.keyboard("{Escape}");

    expect(screen.queryByRole("dialog", { name: /out of the queue/ })).not.toBeInTheDocument();
    expect(drawer).toBeInTheDocument();
    expect(within(drawer).getByRole("button", { name: "Cancel" })).toHaveFocus();

    await userEvent.click(within(drawer).getByRole("button", { name: "Cancel" }));
    await userEvent.click(screen.getByRole("button", { name: "Cancel the pull request" }));

    expect(await within(drawer).findByRole("status")).toHaveTextContent("Cancellation sent");
    expect(await changes()).toEqual([["POST", `${PR}/118/cancel`, null, "token"]]);
  });
});

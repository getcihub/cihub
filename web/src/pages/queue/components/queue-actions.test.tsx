import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { Me } from "@/domain/session/session";
import { type Answers, mockSessionCookie, problem, renderApp, sentChanges } from "@/test/app";
import { ada, answersOfMe, answersOfQueuePage, itemOf } from "@/test/fixtures";

const QUEUE = "/api/v1/orgs/acme/repos/web/queues/main";
const PR = "/api/v1/orgs/acme/repos/web/prs";
const testing = itemOf(118, "testing", { position: 1000, title: "Export invoices as CSV" });

function answersFor(changes: Answers): Answers {
  return { ...answersOfQueuePage([testing]), ...changes };
}

beforeEach(() => {
  mockSessionCookie();
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
    expect(await sentChanges()).toEqual([["PATCH", QUEUE, { state: "paused" }, "token"]]);
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
    expect(await sentChanges()).toEqual([
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
});

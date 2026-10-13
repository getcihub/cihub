import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { type Answers, mockSessionCookie, problem, renderApp, sentChanges } from "@/test/app";
import { answersOfQueuePage, itemOf, pageOf } from "@/test/fixtures";

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

describe("the actions on a PR", () => {
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
    expect(await sentChanges()).toEqual([["POST", `${PR}/118/cancel`, null, "token"]]);
  });
});

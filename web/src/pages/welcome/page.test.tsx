import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { Me } from "@/domain/session/session";
import { mockSessionCookie, problem, renderApp, sentChanges } from "@/test/app";
import { answersOfMe, answersOfMine, pageOf } from "@/test/fixtures";

const grace: Me = {
  login: "grace",
  orgs: [],
  version: "0.2.0",
  app_install_url: "https://github.com/apps/cihub/installations/new",
};

beforeEach(() => {
  mockSessionCookie();
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("the welcome page", () => {
  it("leads a person without an organization to the installation of the App", async () => {
    renderApp("/welcome", answersOfMe(grace));

    expect(await screen.findByRole("heading", { name: "Welcome to CIHub, @grace" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: /install the app on github/i })).toHaveAttribute(
      "href",
      grace.app_install_url,
    );
    expect(screen.getByRole("link", { name: "sign out of GitHub" })).toHaveAttribute(
      "href",
      "https://github.com/logout",
    );
  });

  it("checks again, then opens the organization that came", { timeout: 10_000 }, async () => {
    const acme = { slug: "acme", role: "admin" as const, is_suspended: false };
    const router = renderApp("/welcome", {
      ...answersOfMe(grace),
      ...answersOfMine("acme"),
      "/api/v1/user/orgs": [
        [200, pageOf("orgs", [])],
        [200, pageOf("orgs", [])],
        [200, pageOf("orgs", [acme])],
      ],
      "/api/v1/user/sync": [202, null],
      "/api/v1/orgs/acme/queues": [200, pageOf("queues", [])],
      "/api/v1/orgs/acme/repos": [200, pageOf("repos", [])],
    });

    await userEvent.click(await screen.findByRole("button", { name: "Check again" }));

    await waitFor(
      () => {
        expect(router.state.location.pathname).toBe("/orgs/acme");
      },
      { timeout: 5000 },
    );
    expect(await sentChanges()).toEqual([["POST", "/api/v1/user/sync", null, "token"]]);
  });

  it("says that the check failed", async () => {
    renderApp("/welcome", {
      ...answersOfMe(grace),
      "/api/v1/user/sync": [503, problem(503, "scm_unavailable")],
    });

    await userEvent.click(await screen.findByRole("button", { name: "Check again" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("The check failed.");
  });

  it("is the page of a person without an organization only", async () => {
    const router = renderApp("/", answersOfMe(grace));

    await waitFor(() => {
      expect(router.state.location.pathname).toBe("/welcome");
    });
  });
});

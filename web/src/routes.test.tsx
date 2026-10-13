import { cleanup, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { type Answers, problem, renderApp } from "@/test/app";
import { ada, answersOfMe, answersOfMine, pageOf, queueOf, signedOut } from "@/test/fixtures";

const signedIn: Answers = {
  ...answersOfMe(ada),
  ...answersOfMine("acme"),
  ...answersOfMine("acme-labs"),
  "/api/v1/orgs/acme/queues": [200, pageOf("queues", [queueOf("web", "main")])],
  "/api/v1/orgs/acme-labs/queues": [200, pageOf("queues", [])],
};

afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
});

describe("the routes of the dashboard", () => {
  it("send a person without a session to the sign-in", async () => {
    const router = renderApp("/", signedOut);

    expect(await screen.findByRole("heading", { name: "Sign in" })).toBeInTheDocument();
    expect(router.state.location.pathname).toBe("/sign-in");
  });

  it("open the organization that the person opened last, whatever its case", async () => {
    localStorage.setItem("org", "ACME-LABS");
    const router = renderApp("/", signedIn);

    await waitFor(() => {
      expect(router.state.location.pathname).toBe("/orgs/acme-labs");
    });
    expect(await screen.findByRole("heading", { name: "No queue yet" })).toBeInTheDocument();
  });

  it("say that an organization without a role is out of reach", async () => {
    renderApp("/orgs/globex", {
      ...signedIn,
      "/api/v1/orgs/globex/queues": [404, problem(404, "org_not_found")],
    });

    expect(
      await screen.findByRole("heading", { level: 1, name: "You have no access to globex" }),
    ).toBeInTheDocument();
  });

  it("lead an address without a page of its own to the page above it", async () => {
    const cases: [string, string][] = [
      ["/orgs", "/orgs/acme"],
      ["/orgs/acme/repos", "/orgs/acme"],
      ["/orgs/acme/repos/web/queues", "/orgs/acme/repos/web"],
    ];

    for (const [address, page] of cases) {
      const router = renderApp(address, signedIn);

      await waitFor(() => {
        expect(router.state.location.pathname).toBe(page);
      });
      cleanup();
    }
  });

  it("say that a repository that has no queue is not found", async () => {
    renderApp("/orgs/acme/repos/nope", signedIn);

    expect(await screen.findByRole("heading", { level: 1, name: "Page not found" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Repository: All repositories" })).toBeInTheDocument();
  });

  it("send a person whose session expired to the sign-in, then back to the page", async () => {
    const router = renderApp("/orgs/acme", {
      ...answersOfMe(ada),
      "/api/v1/user": [
        [200, { login: ada.login }],
        [401, problem(401, "unauthorized")],
      ],
      "/api/v1/orgs/acme/queues": [401, problem(401, "unauthorized")],
    });

    await waitFor(() => {
      expect(router.state.location.pathname).toBe("/sign-in");
    });
    expect(router.state.location.search).toEqual({ next: "/orgs/acme" });
  });
});

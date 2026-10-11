import { screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { type Answers, problem, renderApp } from "./test/app";
import { ada, queueOf } from "./test/fixtures";

const signedIn: Answers = {
  "/api/v1/me": [200, ada],
  "/api/v1/orgs/acme/queues": [200, { queues: [queueOf("web", "main")] }],
  "/api/v1/orgs/acme-labs/queues": [200, { queues: [] }],
};

afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
});

describe("the routes of the dashboard", () => {
  it("send a person without a session to the sign-in", async () => {
    const router = renderApp("/", { "/api/v1/me": [401, problem(401, "unauthorized")] });

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
    renderApp("/orgs/globex", signedIn);

    expect(
      await screen.findByRole("heading", { level: 1, name: "You have no access to globex" }),
    ).toBeInTheDocument();
  });

  it("say that a repository that has no queue is not found", async () => {
    renderApp("/orgs/acme/repos/nope", signedIn);

    expect(await screen.findByRole("heading", { level: 1, name: "Page not found" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Repository: All repositories" })).toBeInTheDocument();
  });

  it("send a person whose session expired to the sign-in, then back to the page", async () => {
    const router = renderApp("/orgs/acme", {
      "/api/v1/me": [
        [200, ada],
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

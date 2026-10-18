import { screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { type Answers, renderApp } from "@/test/app";
import { ada, answersOfMe, answersOfMine, pageOf, queueOf, repoRowOf } from "@/test/fixtures";

const REPOS = "/api/v1/orgs/acme/repos";

function answersFor(changes: Answers): Answers {
  return {
    ...answersOfMe(ada),
    ...answersOfMine("acme"),
    "/api/v1/orgs/acme/queues": [
      200,
      pageOf("queues", [queueOf("app", "main"), queueOf("app", "release"), queueOf("web", "main")]),
    ],
    ...changes,
  };
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("the page of an organization", () => {
  it("lists the repositories with their queues below them, and the way to a first queue", async () => {
    const repos = [
      repoRowOf("app", "started"),
      repoRowOf("docs", "cannot_merge"),
      repoRowOf("site", "ready"),
    ];
    renderApp("/orgs/acme", answersFor({ [REPOS]: [200, pageOf("repos", repos)] }));

    expect(await screen.findByRole("heading", { level: 1, name: "Repositories" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: /^main\s*Running/ })).toHaveAttribute(
      "href",
      "/orgs/acme/repos/app/queues/main",
    );
    expect(screen.getByRole("link", { name: /^release\s*Running/ })).toHaveAttribute(
      "href",
      "/orgs/acme/repos/app/queues/release",
    );
    expect(screen.getByText("Cannot merge")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Fix" })).toHaveAttribute("href", "/orgs/acme/repos/docs");
    expect(screen.getByRole("link", { name: "Start" })).toHaveAttribute("href", "/orgs/acme/repos/site");
    const queues = screen.getAllByRole("link", { name: /Running/ });
    expect(queues).toHaveLength(2);
    expect(within(queues[0] ?? document.body).getByText(/in the queue/)).toBeInTheDocument();
    expect(screen.queryByRole("searchbox")).not.toBeInTheDocument();
  });

  it("says that CIHub connects the organization after an install, until the first reads end", async () => {
    const router = renderApp(
      "/orgs/acme?connecting=install",
      answersFor({
        [REPOS]: [
          [200, pageOf("repos", [repoRowOf("site", "checking")])],
          [200, pageOf("repos", [repoRowOf("site", "ready")])],
        ],
      }),
    );

    expect(await screen.findByRole("heading", { name: "Connecting acme" })).toBeInTheDocument();

    await waitFor(
      () => {
        expect(router.state.location.search).toEqual({});
      },
      { timeout: 5000 },
    );
    expect(screen.queryByRole("heading", { name: "Connecting acme" })).not.toBeInTheDocument();
    expect(screen.getByText("Ready")).toBeInTheDocument();
  });

  it("opens with a banner when every installation of the App on the organization is suspended", async () => {
    const suspended = { ...ada, orgs: [{ slug: "acme", role: "admin" as const, is_suspended: true }] };
    renderApp(
      "/orgs/acme",
      answersFor({
        ...answersOfMe(suspended),
        [REPOS]: [200, pageOf("repos", [repoRowOf("app", "started")])],
      }),
    );

    expect(await screen.findByText("CIHub lost its access to acme.")).toBeInTheDocument();
    expect(screen.getAllByRole("link", { name: /Running/ })).toHaveLength(2);
  });

  it("leads the old address of the repositories to the page of the organization", async () => {
    const router = renderApp(
      "/orgs/acme/repos?connecting=update",
      answersFor({ [REPOS]: [200, pageOf("repos", [repoRowOf("app", "started")])] }),
    );

    await waitFor(() => {
      expect(router.state.location.pathname).toBe("/orgs/acme");
    });
  });
});

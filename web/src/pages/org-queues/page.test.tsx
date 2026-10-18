import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { type Answers, renderApp } from "@/test/app";
import { ada, answersOfMe, answersOfMine, pageOf, queueOf, repoOf, repoRowOf } from "@/test/fixtures";

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
  it("lists the repositories with a queue in a table, one row per queue, then the others", async () => {
    const repos = [
      repoRowOf("app", "started"),
      repoRowOf("docs", "cannot_merge"),
      repoRowOf("site", "ready"),
    ];
    renderApp("/orgs/acme", answersFor({ [REPOS]: [200, pageOf("repos", repos)] }));

    expect(await screen.findByRole("heading", { level: 1, name: "Repositories" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /^With a queue/ })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "main" })).toHaveAttribute(
      "href",
      "/orgs/acme/repos/app/queues/main",
    );
    expect(screen.getByRole("link", { name: "release" })).toHaveAttribute(
      "href",
      "/orgs/acme/repos/app/queues/release",
    );
    expect(screen.getAllByRole("link", { name: "acme/app" })).toHaveLength(1);
    expect(screen.getByRole("heading", { name: /^Not started/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /docs.*Cannot merge.*Fix/ })).toHaveAttribute(
      "aria-expanded",
      "false",
    );
    expect(screen.getByRole("button", { name: /site.*Ready.*Start/ })).toBeInTheDocument();
    expect(screen.queryByText(/No queue yet in acme/)).not.toBeInTheDocument();
    expect(screen.queryByRole("searchbox")).not.toBeInTheDocument();
  });

  it("opens a repository in place on what blocks it, and keeps it in the address", async () => {
    const blocked = repoOf("docs");
    const router = renderApp(
      "/orgs/acme",
      answersFor({
        [REPOS]: [200, pageOf("repos", [repoRowOf("docs", "cannot_merge")])],
        [`${REPOS}/docs`]: [
          200,
          {
            ...blocked,
            setup: {
              ...blocked.setup,
              state: "cannot_merge",
              queue: null,
              checked_at: new Date().toISOString(),
              rules: [
                {
                  code: "app_not_exempt",
                  passed: false,
                  blocks_merge: true,
                  title: "A rule restricts the updates of `main` without an exemption for the App",
                  message: "add the App of CIHub to the bypass list of the ruleset, in exempt mode",
                },
              ],
              config: null,
            },
          },
        ],
      }),
    );

    await userEvent.click(await screen.findByRole("button", { name: /docs.*Fix/ }));

    expect(await screen.findByText("1 point blocks every merge", { exact: false })).toBeInTheDocument();
    expect(screen.getByText(/without an exemption for the App/)).toBeInTheDocument();
    expect(screen.getByText(/Fix the points above/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Check again" })).toBeInTheDocument();
    expect(router.state.location.search).toEqual({ open: "docs" });
  });

  it("gives one check to a repository whose rules and file both fail", async () => {
    const blocked = repoOf("docs");
    renderApp(
      "/orgs/acme?open=docs",
      answersFor({
        [REPOS]: [200, pageOf("repos", [repoRowOf("docs", "invalid_config")])],
        [`${REPOS}/docs`]: [
          200,
          {
            ...blocked,
            setup: {
              ...blocked.setup,
              state: "invalid_config",
              queue: null,
              checked_at: new Date().toISOString(),
              rules: [
                {
                  code: "app_not_exempt",
                  passed: false,
                  blocks_merge: true,
                  title: "A rule restricts the updates of `main` without an exemption for the App",
                },
              ],
              config: {
                state: "invalid",
                path: ".cihub.yaml",
                sha: "abc",
                error: "unknown key `stratgy`",
                line: 3,
                column: 5,
                text: "version: 1\nqueues:\n  - stratgy: batched\n",
                entries: [],
                strategy: "pipelined",
                settings: {
                  batching_enabled: false,
                  batching_size: 1,
                  batching_max_wait_minutes: 0,
                  failure_budget_retries: 1,
                  merge_method: "squash",
                },
              },
            },
          },
        ],
      }),
    );

    expect(await screen.findByText(/is not valid:/)).toBeInTheDocument();
    expect(screen.getByText(/without an exemption for the App/)).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "Check again" })).toHaveLength(1);
  });

  it("shows how the first queue starts on a repository that is ready", async () => {
    const ready = repoOf("site");
    renderApp(
      "/orgs/acme?open=site",
      answersFor({
        "/api/v1/orgs/acme/queues": [200, pageOf("queues", [])],
        [REPOS]: [200, pageOf("repos", [repoRowOf("site", "ready")])],
        [`${REPOS}/site`]: [
          200,
          {
            ...ready,
            setup: {
              ...ready.setup,
              state: "ready",
              queue: null,
              checked_at: new Date().toISOString(),
              rules: [
                { code: "app_not_exempt", passed: true, blocks_merge: true, title: "The App is exempt" },
              ],
              config: {
                state: "none",
                entries: [],
                strategy: "pipelined",
                settings: {
                  batching_enabled: false,
                  batching_size: 1,
                  batching_max_wait_minutes: 0,
                  failure_budget_retries: 1,
                  merge_method: "squash",
                },
              },
            },
          },
        ],
      }),
    );

    expect(await screen.findByText("No queue yet in acme.")).toBeInTheDocument();
    expect(await screen.findByText("Waiting for the first command")).toBeInTheDocument();
    expect(screen.getByText(/CIHub can merge on/)).toBeInTheDocument();
    expect(screen.getByText(/the defaults: pipelined, squash merge/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Copy" })).toBeInTheDocument();
  });

  it("cuts a long list of the repositories without a queue, until the person asks for all", async () => {
    const repos = Array.from({ length: 12 }, (_, index) => repoRowOf(`repo-${String(index)}`, "ready"));
    renderApp("/orgs/acme", answersFor({ [REPOS]: [200, pageOf("repos", repos)] }));

    expect(await screen.findAllByRole("button", { name: /Start$/ })).toHaveLength(8);

    await userEvent.click(screen.getByRole("button", { name: /Show the 4 other repositories/ }));

    expect(screen.getAllByRole("button", { name: /Start$/ })).toHaveLength(12);
    expect(screen.getByRole("searchbox", { name: "Search the repositories" })).toHaveAttribute(
      "placeholder",
      "Search 12 repositories",
    );
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
    expect(screen.getByRole("link", { name: "main" })).toBeInTheDocument();
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

import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { type Answers, mockSessionCookie, renderApp, sentChanges } from "@/test/app";
import type { RepoDetail } from "@/client/types.gen";
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

/** The organization before its first queue: no queue, and `repos`. */
function firstQueueAnswers(repos: Answers[string], changes: Answers = {}): Answers {
  return answersFor({ "/api/v1/orgs/acme/queues": [200, pageOf("queues", [])], [REPOS]: repos, ...changes });
}

/** `docs`, read: the App is not exempt from the rule that restricts the updates of `main`. */
function blockedDocs(state: "cannot_merge" | "checking" = "cannot_merge"): RepoDetail {
  const repo = repoOf("docs");
  return {
    ...repo,
    setup: {
      ...repo.setup,
      state,
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
      config: null,
    },
  };
}

const MIXED = pageOf("repos", [
  repoRowOf("docs", "cannot_merge"),
  repoRowOf("site", "ready"),
  repoRowOf("web", "ready_with_warnings"),
]);

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
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
    expect(screen.queryByRole("heading", { name: "Start your first merge queue" })).not.toBeInTheDocument();
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

    expect(await screen.findByRole("heading", { name: "Start your first merge queue" })).toBeInTheDocument();
    expect(await screen.findByText("Waiting for the first command")).toBeInTheDocument();
    expect(screen.getByText(/CIHub can merge on/)).toBeInTheDocument();
    expect(screen.getByText(/the defaults: pipelined, squash merge/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Copy" })).toBeInTheDocument();
  });

  it("splits the repositories into the ready ones and the ones that need a fix, before the first queue", async () => {
    renderApp("/orgs/acme", firstQueueAnswers([200, MIXED]));

    expect(await screen.findByRole("heading", { name: /^Ready to start\s*2$/ })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /^Needs a fix\s*1$/ })).toBeInTheDocument();
    expect(screen.getByText("A point blocks every merge until it is fixed")).toBeInTheDocument();
    expect(screen.getByText(/of acme: 2 are ready to start\./)).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: /^Repositories\s*3$/ })).not.toBeInTheDocument();
  });

  it("keeps the list in one part for a search", async () => {
    renderApp("/orgs/acme?q=s", firstQueueAnswers([200, MIXED]));

    expect(await screen.findByRole("heading", { name: /^Not started\s*3$/ })).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: /^Needs a fix/ })).not.toBeInTheDocument();
  });

  it.each([
    ["/orgs/acme", "Pick a repository"],
    ["/orgs/acme?open=docs", "Check its setup"],
    ["/orgs/acme?open=site", "Comment /cihub merge"],
  ])("follows the open row in the steps to the first queue, at %s", async (path, step) => {
    renderApp(path, firstQueueAnswers([200, MIXED], { [`${REPOS}/docs`]: [200, blockedDocs()] }));

    const current = await screen.findByRole("listitem", { current: "step" });

    expect(current).toHaveTextContent(step);
  });

  it("opens the first ready repository from the start of the page, and scrolls to it", async () => {
    const scrollIntoView = vi.fn();
    Element.prototype.scrollIntoView = scrollIntoView;
    const router = renderApp("/orgs/acme", firstQueueAnswers([200, MIXED]));

    await userEvent.click(await screen.findByRole("button", { name: "Start with acme/site" }));

    await waitFor(() => {
      expect(router.state.location.search).toEqual({ open: "site" });
    });
    expect(scrollIntoView).toHaveBeenCalled();
    expect(screen.queryByRole("button", { name: /^Start with/ })).not.toBeInTheDocument();
  });

  it("keeps a repository in its part while CIHub checks it again", async () => {
    mockSessionCookie();
    renderApp(
      "/orgs/acme?open=docs",
      firstQueueAnswers(
        [
          [200, MIXED],
          [
            200,
            pageOf("repos", [
              {
                ...repoRowOf("docs", "checking"),
                setup: { state: "checking", requested_at: new Date().toISOString() },
              },
              repoRowOf("site", "ready"),
              repoRowOf("web", "ready_with_warnings"),
            ]),
          ],
        ],
        {
          [`${REPOS}/docs`]: [
            [200, blockedDocs()],
            [200, blockedDocs("checking")],
          ],
          [`${REPOS}/docs/sync`]: [202, null],
        },
      ),
    );

    await userEvent.click(await screen.findByRole("button", { name: "Check again" }));

    expect(await screen.findByRole("button", { name: /docs.*Checking/ })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /^Needs a fix\s*1$/ })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /^Ready to start\s*2$/ })).toBeInTheDocument();
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

  it("waits for the first read of the repositories of the person, then lists them", async () => {
    renderApp(
      "/orgs/acme",
      answersFor({
        "/api/v1/user": [
          [200, { login: ada.login, synced_at: null }],
          [200, { login: ada.login, synced_at: ada.synced_at }],
        ],
        [REPOS]: [
          [200, pageOf("repos", [])],
          [200, pageOf("repos", [repoRowOf("site", "ready")])],
        ],
      }),
    );

    expect(await screen.findByRole("heading", { name: "Reading your repositories" })).toBeInTheDocument();
    expect(screen.queryByText(/reaches no repository/)).not.toBeInTheDocument();

    expect(await screen.findByText("Ready", {}, { timeout: 5000 })).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Reading your repositories" })).not.toBeInTheDocument();
  });

  it("asks GitHub again for the repositories of the person when the list is empty", async () => {
    mockSessionCookie();
    renderApp(
      "/orgs/acme",
      answersFor({
        "/api/v1/orgs/acme/queues": [200, pageOf("queues", [])],
        [REPOS]: [
          [200, pageOf("repos", [])],
          [200, pageOf("repos", [])],
          [200, pageOf("repos", [repoRowOf("site", "ready")])],
        ],
        "/api/v1/user/sync": [202, null],
      }),
    );

    await userEvent.click(await screen.findByRole("button", { name: "Check again" }));

    expect(await screen.findByText("Ready", {}, { timeout: 5000 })).toBeInTheDocument();
    expect(await sentChanges()).toEqual([["POST", "/api/v1/user/sync", null, "token"]]);
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

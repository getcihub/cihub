import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { RepoDetail, RuleCheck } from "@/client/types.gen";
import { type Answers, mockSessionCookie, renderApp, sentChanges } from "@/test/app";
import { ada, answersOfMe, answersOfMine, pageOf, repoOf } from "@/test/fixtures";

const REPO = "/api/v1/orgs/acme/repos/web";

/** `web`, without a queue: its App is not exempt, and its file names an unknown key at line 3. */
function blocked(admin = true): RepoDetail {
  const repo = repoOf("web", { read: true, write: true, admin });
  return {
    ...repo,
    setup: {
      ...repo.setup,
      state: "invalid_config",
      queue: null,
      checked_at: new Date().toISOString(),
      rules_url: "https://github.com/acme/web/settings/rules",
      rules: [
        {
          code: "app_not_exempt",
          passed: false,
          blocks_merge: true,
          title: "A rule restricts the updates of `main` without an exemption for the App",
          message: "add the App of CIHub to the bypass list of the ruleset, in exempt mode",
        },
        {
          code: "up_to_date_required",
          passed: true,
          blocks_merge: false,
          title: "`main` asks for no branch up to date",
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
  };
}

/** `web`, without a queue and ready: its rules pass but `failed`, and it has no file. */
function ready(failed: RuleCheck[] = []): RepoDetail {
  const repo = repoOf("web");
  return {
    ...repo,
    setup: {
      ...repo.setup,
      state: failed.length > 0 ? "ready_with_warnings" : "ready",
      queue: null,
      checked_at: new Date().toISOString(),
      rules: [
        {
          code: "app_not_exempt",
          passed: true,
          blocks_merge: true,
          title: "The App of CIHub may update `main`",
        },
        ...failed,
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
  };
}

const UP_TO_DATE: RuleCheck = {
  code: "up_to_date_required",
  passed: false,
  blocks_merge: false,
  title: "`main` asks for branches up to date",
  message:
    'turn off "Require branches to be up to date before merging" on `main`, unless the queue merges directly',
};

function answersFor(repo: RepoDetail, changes: Answers = {}): Answers {
  return {
    ...answersOfMe(ada),
    ...answersOfMine("acme"),
    "/api/v1/orgs/acme/queues": [200, pageOf("queues", [])],
    [REPO]: [200, repo],
    ...changes,
  };
}

beforeEach(() => {
  mockSessionCookie();
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("the start of a repository without a queue", () => {
  it("gives the first command of a ready repository, and folds its setup", async () => {
    renderApp("/orgs/acme/repos/web", answersFor(ready()));

    expect(await screen.findByRole("heading", { name: "No queue on main yet" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Copy" })).toBeInTheDocument();
    expect(screen.getByText("Waiting for the first command")).toBeInTheDocument();
    expect(screen.getByText(/Settings: the defaults: pipelined, squash merge/)).toBeInTheDocument();
    expect(screen.queryByText(/with a warning/)).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Show" })).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByText(/Start the queue/)).not.toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: /^Queues/ })).not.toBeInTheDocument();
  });

  it("says the first warning of a repository that can start, in the words of the server", async () => {
    renderApp("/orgs/acme/repos/web", answersFor(ready([UP_TO_DATE])));

    expect(await screen.findByText(/The queue can start, with a warning\./)).toHaveTextContent(
      "The queue can start, with a warning. main asks for branches up to date. See the setup below.",
    );
    expect(screen.getByRole("button", { name: "Copy" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Hide" })).toHaveAttribute("aria-expanded", "true");
  });

  it("says what blocks a repository, and how to fix it, instead of the command", async () => {
    const repo = blocked();
    renderApp(
      "/orgs/acme/repos/web",
      answersFor({ ...repo, setup: { ...repo.setup, state: "cannot_merge", config: null } }),
    );

    expect(await screen.findByRole("heading", { name: "main cannot have a queue yet" })).toBeInTheDocument();
    expect(screen.getByText(/^1 branch rule to fix: the queue could not merge/)).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Open the rules on GitHub" })).toHaveAttribute(
      "href",
      "https://github.com/acme/web/settings/rules",
    );
    expect(screen.getAllByRole("button", { name: "Check again" })).toHaveLength(2);
    expect(screen.queryByRole("button", { name: "Copy" })).not.toBeInTheDocument();
  });

  it("says that a file that is not valid blocks the repository", async () => {
    renderApp("/orgs/acme/repos/web", answersFor(blocked(false)));

    expect(await screen.findByText(/is not valid: the queue could not merge/)).toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "Open the rules on GitHub" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Check again" })).not.toBeInTheDocument();
  });
});

describe("the setup of a repository", () => {
  it("shows what the rules miss and where the file fails, as the server says them", async () => {
    renderApp("/orgs/acme/repos/web", answersFor(blocked()));

    expect(await screen.findByText(/without an exemption for the App/)).toBeInTheDocument();
    expect(screen.getByText("blocks every merge")).toBeInTheDocument();
    expect(
      screen.getByText("add the App of CIHub to the bypass list of the ruleset, in exempt mode"),
    ).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Open the rules of web on GitHub →" })).toHaveAttribute(
      "href",
      "https://github.com/acme/web/settings/rules",
    );
    expect(screen.getByText(/is not valid: unknown key/)).toBeInTheDocument();
    expect(screen.getByText("- stratgy: batched")).toHaveClass("bg-destructive-muted");
    expect(screen.queryByText(/Start the queue/)).not.toBeInTheDocument();
  });

  it("lets an admin of the repository check it again", async () => {
    renderApp("/orgs/acme/repos/web", answersFor(blocked(), { [`${REPO}/sync`]: [202, null] }));

    const [checkAgain] = await screen.findAllByRole("button", { name: "Check again" });
    if (checkAgain) {
      await userEvent.click(checkAgain);
    }

    expect(await sentChanges()).toEqual([["POST", `${REPO}/sync`, null, "token"]]);
  });

  it("gives no check to a person who does not administer the repository", async () => {
    renderApp("/orgs/acme/repos/web", answersFor(blocked(false)));

    expect(await screen.findByText("blocks every merge")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Check again" })).not.toBeInTheDocument();
  });
});

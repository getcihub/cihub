import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { RepoDetail } from "@/client/types.gen";
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
    expect(screen.getByRole("button", { name: "Copy" })).toBeInTheDocument();
  });

  it("lets an admin of the repository check it again", async () => {
    renderApp("/orgs/acme/repos/web", answersFor(blocked(), { [`${REPO}/sync`]: [202, null] }));

    await userEvent.click(await screen.findByRole("button", { name: "Check again" }));

    expect(await sentChanges()).toEqual([["POST", `${REPO}/sync`, null, "token"]]);
  });

  it("gives no check to a person who does not administer the repository", async () => {
    renderApp("/orgs/acme/repos/web", answersFor(blocked(false)));

    expect(await screen.findByText("blocks every merge")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Check again" })).not.toBeInTheDocument();
  });
});

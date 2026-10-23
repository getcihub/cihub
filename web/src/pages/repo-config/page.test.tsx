import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { BranchConfig, ConfigPreview, ConfigProposal, Permission } from "@/client/types.gen";
import { type Answers, mockSessionCookie, problem, renderApp, sentChanges } from "@/test/app";
import { branchConfigOf } from "@/test/config";
import { ADMIN, ada, answersOfMe, pageOf, repoOf } from "@/test/fixtures";

const REPO = "/api/v1/orgs/acme/repos/web";
const CONFIG = `${REPO}/config`;
const READER: Permission = { read: true, write: true, admin: false };

function answersFor(config: Answers[string], permission: Permission = ADMIN, changes: Answers = {}): Answers {
  return {
    ...answersOfMe(ada),
    "/api/v1/orgs/acme/queues": [200, pageOf("queues", [])],
    [REPO]: [200, repoOf("web", permission)],
    [CONFIG]: config,
    ...changes,
  };
}

/** The file of `main` once the person changed the size of the batches of `main` to 7. */
function previewOf(config: BranchConfig): ConfigPreview {
  return {
    text: "version: 1\nqueues:\n  - branch: main\n    batching:\n      size: 7\n",
    state: "valid",
    settings: config.settings,
    diff: [
      { change: "same", text: "version: 1" },
      { change: "removed", text: "      size: 6" },
      { change: "added", text: "      size: 7" },
    ],
  };
}

/** The row of the setting `title`: its card, the first one of the page. */
function rowOf(title: string): HTMLElement {
  const [label] = screen.getAllByText(title, { selector: "span" });
  const card = label?.closest<HTMLElement>("[data-slot=card]");
  if (!card) {
    throw new Error(`no row ${title}`);
  }
  return card;
}

beforeEach(() => {
  mockSessionCookie();
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("the builder of the configuration", () => {
  it("shows each setting of the queue of the branch with its source, read only without admin", async () => {
    renderApp("/orgs/acme/repos/web/config", answersFor([200, branchConfigOf()], READER));

    expect(await screen.findByRole("heading", { name: "Queue of main" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Configuration" })).toHaveAttribute("aria-current", "page");
    expect(screen.getByText("Read only.")).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: /^Batched/ })).toHaveAttribute("aria-checked", "true");
    expect(within(rowOf("Batch size")).getByRole("textbox")).toHaveValue("6");
    expect(within(rowOf("Batch size")).getByRole("textbox")).toBeDisabled();
    expect(within(rowOf("Batch size")).getByText("in this queue")).toBeInTheDocument();
    expect(within(rowOf("Time limit of a test")).getByText("from defaults")).toBeInTheDocument();
    expect(within(rowOf("Optimistic merging")).getByText("from the strategy")).toBeInTheDocument();
    expect(within(rowOf("Optimistic merging")).getByRole("switch")).toHaveAttribute("aria-checked", "true");
    expect(screen.getByText("From defaults, checked first")).toBeInTheDocument();
    expect(screen.getByText("/cihub merge --profile=dependencies")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Add a queue/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "use the default" })).not.toBeInTheDocument();
  });

  it("lets an admin change a setting, see the diff and propose it as a pull request", async () => {
    const config = branchConfigOf();
    const proposal: ConfigProposal = {
      id: "proposal_1",
      branch: "main",
      state: "requested",
      requested_at: "2026-10-01T09:00:00Z",
    };
    renderApp(
      "/orgs/acme/repos/web/config",
      answersFor([200, config], ADMIN, {
        [`${CONFIG}/preview`]: [200, previewOf(config)],
        [`${CONFIG}/proposals`]: [202, proposal],
      }),
    );

    const size = within(await screenRow("Batch size")).getByRole("textbox");
    await userEvent.clear(size);
    await userEvent.type(size, "7{Enter}");

    expect(await screen.findByText("changed")).toBeInTheDocument();
    const bar = await screen.findByRole("region", { name: "Pending changes" });
    await waitFor(() => {
      expect(bar).toHaveTextContent("+1 −1 pending changes to .cihub.yaml");
    });

    await userEvent.click(within(bar).getByRole("button", { name: "Propose as a pull request" }));
    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByText("Comments in the file are lost.")).toBeInTheDocument();
    expect(within(dialog).getByText("size: 7", { exact: false })).toBeInTheDocument();
    await userEvent.click(within(dialog).getByRole("button", { name: "Open the pull request" }));

    const [preview, propose] = (await sentChanges()).slice(-2);
    expect(preview?.[1]).toBe(`${CONFIG}/preview`);
    expect(propose?.[1]).toBe(`${CONFIG}/proposals`);
    expect(propose?.[2]).toMatchObject({
      branch: "main",
      base_sha: "3f2a1c9e0b1d",
      message: "Update the merge queue configuration",
      document: { queues: [{ branch: "main", settings: { batching_size: 7 } }, { branch: "release/*" }] },
    });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });
    expect(screen.queryByRole("region", { name: "Pending changes" })).not.toBeInTheDocument();
  });

  it("adds a queue to the draft, and undoes every change", async () => {
    const config = branchConfigOf();
    renderApp(
      "/orgs/acme/repos/web/config",
      answersFor([200, config], ADMIN, { [`${CONFIG}/preview`]: [200, previewOf(config)] }),
    );

    // The list of the file and the bar of a narrow screen both offer it.
    const [add] = await screen.findAllByRole("button", { name: "Add a queue" });
    if (add) {
      await userEvent.click(add);
    }
    await userEvent.type(within(screen.getByRole("dialog")).getByRole("textbox"), "dev");
    await userEvent.click(screen.getByRole("button", { name: "Add the queue" }));

    expect(await screen.findByRole("heading", { name: "Queue of dev" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Remove the queue" })).toBeInTheDocument();

    await userEvent.click(await screen.findByRole("button", { name: "Undo all" }));

    expect(screen.queryByRole("region", { name: "Pending changes" })).not.toBeInTheDocument();
    expect(screen.queryByRole("link", { name: /^dev/ })).not.toBeInTheDocument();
  });

  it("shows `defaults`, without the rules and the profiles of a queue", async () => {
    renderApp("/orgs/acme/repos/web/config", answersFor([200, branchConfigOf()], READER));

    await userEvent.click(await screen.findByRole("link", { name: /^Defaults/ }));

    expect(await screen.findByRole("heading", { name: "Defaults" })).toBeInTheDocument();
    expect(within(rowOf("Time limit of a test")).getByText("in defaults")).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Priority" })).not.toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Profiles" })).not.toBeInTheDocument();
  });

  it("shows the file, and the error of a file that is not valid at its line", async () => {
    const invalid: BranchConfig = {
      ...branchConfigOf(),
      state: "invalid",
      error: "strategy `parallel` is not available yet",
      line: 2,
      column: 15,
      text: "version: 1\nstrategy: parallel\n",
      document: null,
      settings: null,
    };
    renderApp("/orgs/acme/repos/web/config", answersFor([200, invalid], READER));

    expect(await screen.findByText("The file is not valid, line 2:")).toBeInTheDocument();
    expect(screen.getByText(/The builder shows a valid file only/)).toBeInTheDocument();

    const views = screen.getByRole("navigation", { name: "View" });
    await userEvent.click(within(views).getByRole("link", { name: "YAML" }));

    expect(await screen.findByText("strategy: parallel")).toHaveClass("bg-destructive-muted");
  });

  it("says where the proposal of a change stands", async () => {
    const opened: BranchConfig = {
      ...branchConfigOf(),
      proposal: {
        id: "proposal_1",
        branch: "main",
        state: "opened",
        pr_number: 141,
        pr_url: "https://github.com/acme/web/pull/141",
        requested_at: "2026-10-01T09:00:00Z",
      },
    };
    renderApp("/orgs/acme/repos/web/config", answersFor([200, opened]));

    expect(await screen.findByText("Pull request #141 changes this file.")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Open #141 on GitHub" })).toHaveAttribute(
      "href",
      "https://github.com/acme/web/pull/141",
    );
  });

  it("says that CIHub has not read the file yet", async () => {
    renderApp("/orgs/acme/repos/web/config", answersFor([404, problem(404, "config_not_found")]));

    expect(await screen.findByRole("heading", { name: "Not read yet" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Overview" })).toBeInTheDocument();
  });
});

async function screenRow(title: string): Promise<HTMLElement> {
  await screen.findAllByText(title, { selector: "span" });
  return rowOf(title);
}

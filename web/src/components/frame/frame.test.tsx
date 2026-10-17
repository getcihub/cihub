import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ada, queueOf } from "@/test/fixtures";
import { renderInRouter } from "@/test/router";
import { Frame } from "./frame";

const queues = [queueOf("api", "main", 3), queueOf("web", "main", 6), queueOf("web", "release", 0, "line 7")];

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("Frame", () => {
  it("names the organization of the page and lists the others with the role of the person", async () => {
    renderInRouter(
      <Frame me={ada} org="acme" queues={queues}>
        page
      </Frame>,
    );

    await userEvent.click(await screen.findByRole("button", { name: "Organization: acme" }));
    const menu = screen.getByRole("group");

    expect(within(menu).getByRole("link", { name: /acme-labs/ })).toHaveAttribute("href", "/orgs/acme-labs");
    expect(within(menu).getByRole("link", { name: /^acme\s*Admin$/ })).toBeInTheDocument();
    expect(within(menu).getByRole("link", { name: /add an organization/i })).toHaveAttribute(
      "href",
      ada.app_install_url,
    );
  });

  it("lists the repositories with their number of queues, opens the only queue of a repository, and closes on Escape", async () => {
    renderInRouter(
      <Frame me={ada} org="acme" queues={queues}>
        page
      </Frame>,
    );

    await userEvent.click(await screen.findByRole("button", { name: "Repository: All repositories" }));
    const menu = screen.getByRole("group");

    expect(within(menu).getByRole("link", { name: /^all repositories\s*3$/i })).toHaveAttribute(
      "href",
      "/orgs/acme",
    );
    expect(within(menu).getByRole("link", { name: /^acme\/web\s*2$/ })).toHaveAttribute(
      "href",
      "/orgs/acme/repos/web",
    );
    expect(within(menu).getByRole("link", { name: /^acme\/api\s*1$/ })).toHaveAttribute(
      "href",
      "/orgs/acme/repos/api/queues/main",
    );

    await userEvent.keyboard("{Escape}");

    expect(screen.queryByRole("group")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Repository: All repositories" })).toHaveFocus();
  });

  it("lists the queues of the repository on the page of a queue", async () => {
    renderInRouter(
      <Frame me={ada} org="acme" queues={queues} repo="web" queue="main">
        page
      </Frame>,
    );

    await userEvent.click(await screen.findByRole("button", { name: "Queue: main" }));
    const menu = screen.getByRole("group");

    expect(within(menu).getByRole("link", { name: /^main\s*6 in the queue$/ })).toHaveAttribute(
      "href",
      "/orgs/acme/repos/web/queues/main",
    );
    expect(within(menu).getByRole("link", { name: /^release\s*invalid$/ })).toBeInTheDocument();
  });

  it("has no repository to pick in an organization without a queue", async () => {
    renderInRouter(
      <Frame me={ada} org="acme" queues={[]}>
        page
      </Frame>,
    );

    expect(await screen.findByRole("button", { name: "Organization: acme" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Repository/ })).not.toBeInTheDocument();
  });

  it("keeps the theme that the person chooses, or follows the system", async () => {
    vi.stubGlobal(
      "matchMedia",
      vi.fn(() => ({ matches: true })),
    );
    renderInRouter(
      <Frame me={ada} org="acme" queues={queues}>
        page
      </Frame>,
    );

    await userEvent.click(await screen.findByRole("button", { name: "Your account: ada" }));
    await userEvent.click(screen.getByRole("button", { name: "Appearance" }));
    await userEvent.click(screen.getByRole("button", { name: "Light" }));

    expect(document.documentElement.dataset.theme).toBe("light");
    expect(localStorage.getItem("theme")).toBe("light");

    await userEvent.click(screen.getByRole("button", { name: "Your account: ada" }));
    await userEvent.click(screen.getByRole("button", { name: "Appearance" }));

    expect(screen.getByRole("button", { name: "Light" })).toHaveAttribute("aria-pressed", "true");

    await userEvent.click(screen.getByRole("button", { name: "System" }));

    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(localStorage.getItem("theme")).toBeNull();
  });

  it("links to the notes of the release that the instance runs", async () => {
    renderInRouter(
      <Frame me={ada} org="acme" queues={queues}>
        page
      </Frame>,
    );

    await userEvent.click(await screen.findByRole("button", { name: "Your account: ada" }));

    expect(screen.getByRole("link", { name: /About/ })).toHaveAttribute(
      "href",
      `https://github.com/getcihub/cihub/releases/tag/v${ada.version}`,
    );
  });

  it("signs out, or says that the sign-out failed", async () => {
    const assign = vi.fn();
    vi.stubGlobal("location", { assign });
    vi.stubGlobal(
      "fetch",
      vi.fn(() => Promise.resolve(new Response(null, { status: 403 }))),
    );
    renderInRouter(
      <Frame me={ada} org="acme" queues={queues}>
        page
      </Frame>,
    );

    await userEvent.click(await screen.findByRole("button", { name: "Your account: ada" }));
    await userEvent.click(screen.getByRole("button", { name: "Sign out" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("The sign-out failed.");
    expect(assign).not.toHaveBeenCalled();

    vi.stubGlobal(
      "fetch",
      vi.fn(() => Promise.resolve(new Response(null, { status: 204 }))),
    );
    await userEvent.click(screen.getByRole("button", { name: "Your account: ada" }));
    await userEvent.click(screen.getByRole("button", { name: "Sign out" }));

    expect(assign).toHaveBeenCalledWith("/sign-in");
  });
});

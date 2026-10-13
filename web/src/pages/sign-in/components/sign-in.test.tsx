import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Me } from "@/domain/session/session";
import { NoOrganization } from "./no-organization";
import { SignInForm } from "./sign-in-form";

const lonely: Me = {
  login: "grace",
  orgs: [],
  version: "0.2.0",
  app_install_url: "https://github.com/apps/cihub/installations/new",
};

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("the sign-in", () => {
  it("starts the sign-in, then opens the page that the link asked for", () => {
    render(<SignInForm hasFailed={false} next="/orgs/acme" />);

    const start = screen.getByRole("link", { name: /continue with github/i });

    expect(screen.getByRole("heading", { name: "Sign in" })).toBeInTheDocument();
    expect(start).toHaveAttribute("href", "/auth/github?next=%2Forgs%2Facme");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("says that the sign-in failed", () => {
    render(<SignInForm hasFailed={true} next="/" />);

    expect(screen.getByRole("alert")).toHaveTextContent("The sign-in failed. Try again.");
  });

  it("leads a user without an organization to the installation of the App", () => {
    render(<NoOrganization installUrl={lonely.app_install_url ?? null} />);

    expect(screen.getByRole("heading", { name: "No organization yet" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: /install the app on github/i })).toHaveAttribute(
      "href",
      lonely.app_install_url,
    );
  });

  it("signs out, and tells how to use another GitHub account", async () => {
    const fetch = vi.fn(() => Promise.resolve(new Response(null, { status: 204 })));
    const assign = vi.fn();
    vi.stubGlobal("fetch", fetch);
    vi.stubGlobal("location", { assign });
    // The server sets this cookie; a script only reads it.
    vi.spyOn(document, "cookie", "get").mockReturnValue("theme=dark; __Host-cihub_csrf=token");
    render(<NoOrganization installUrl={lonely.app_install_url ?? null} />);

    expect(screen.getByRole("link", { name: "sign out of GitHub" })).toHaveAttribute(
      "href",
      "https://github.com/logout",
    );

    await userEvent.click(screen.getByRole("button", { name: "Sign out" }));

    expect(fetch).toHaveBeenCalledWith("/auth/sign-out", {
      method: "POST",
      headers: { "X-CSRF-Token": "token" },
    });
    expect(assign).toHaveBeenCalledWith("/sign-in");
  });

  it("stays on the page and says so when the sign-out fails", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() => Promise.resolve(new Response(null, { status: 403 }))),
    );
    const assign = vi.fn();
    vi.stubGlobal("location", { assign });
    render(<NoOrganization installUrl={lonely.app_install_url ?? null} />);

    await userEvent.click(screen.getByRole("button", { name: "Sign out" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("The sign-out failed.");
    expect(assign).not.toHaveBeenCalled();
  });
});

import { screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ada } from "../test/fixtures";
import { renderInRouter } from "../test/router";
import { OrgLostPage } from "./OrgLostPage";
import { ServerErrorPage } from "./ServerErrorPage";

describe("OrgLostPage", () => {
  it("says that the access is lost and opens another organization of the person", async () => {
    renderInRouter(<OrgLostPage me={ada} org="acme" />);

    expect(
      await screen.findByRole("heading", { level: 1, name: "You have no access to acme" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Open acme-labs" })).toHaveAttribute("href", "/orgs/acme-labs");
  });
});

describe("ServerErrorPage", () => {
  it("says what the server answered", async () => {
    const problem = {
      status: 503,
      title: "Service Unavailable",
      code: "internal",
      detail: "",
      type: "about:blank",
    };
    renderInRouter(<ServerErrorPage error={problem} reset={vi.fn()} info={{ componentStack: "" }} />);

    expect(
      await screen.findByRole("heading", { level: 1, name: "CIHub could not load this page" }),
    ).toBeInTheDocument();
    expect(screen.getByText("503 Service Unavailable")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Try again" })).toBeInTheDocument();
  });

  it("says that the server did not answer when the failure is not a problem of the API", async () => {
    renderInRouter(
      <ServerErrorPage
        error={new TypeError("Failed to fetch")}
        reset={vi.fn()}
        info={{ componentStack: "" }}
      />,
    );

    expect(await screen.findByText(/The server did not answer, or the page failed./)).toBeInTheDocument();
  });
});

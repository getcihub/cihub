import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ada } from "@/test/fixtures";
import { renderInRouter } from "@/test/router";
import { OrgLostPage } from "./org-lost-page";

describe("OrgLostPage", () => {
  it("says that the access is lost and opens another organization of the person", async () => {
    renderInRouter(<OrgLostPage me={ada} org="acme" />);

    expect(
      await screen.findByRole("heading", { level: 1, name: "You have no access to acme" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Open acme-labs" })).toHaveAttribute("href", "/orgs/acme-labs");
  });
});

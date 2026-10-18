import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Alert, AlertContent, AlertDescription, AlertTitle } from "./alert";

describe("Alert", () => {
  it("is an alert in the tone of its variant, with its parts named by slot", () => {
    render(
      <Alert variant="destructive">
        <AlertContent>
          <AlertTitle>The sign-in failed.</AlertTitle>
          <AlertDescription>Try again.</AlertDescription>
        </AlertContent>
      </Alert>,
    );

    const alert = screen.getByRole("alert");

    expect(alert).toHaveAttribute("data-variant", "destructive");
    expect(alert.querySelector("[data-slot=alert-title]")).toHaveTextContent("The sign-in failed.");
    expect(alert.querySelector("[data-slot=alert-description]")).toHaveTextContent("Try again.");
  });

  it("keeps a neutral surface as a banner, with its tone in a rule on its left", () => {
    render(<Alert variant="warning" isBanner />);

    const alert = screen.getByRole("alert");

    expect(alert).toHaveClass("bg-card", "border-border", "shadow-[inset_2px_0_0_var(--color-warning)]");
    expect(alert).not.toHaveClass("bg-warning-muted");
  });

  it("takes the role and the classes of the caller over its own", () => {
    render(<Alert role="status" className="px-4" />);

    const alert = screen.getByRole("status");

    expect(alert).toHaveAttribute("data-variant", "info");
    expect(alert).toHaveClass("px-4");
    expect(alert).not.toHaveClass("px-3");
  });
});

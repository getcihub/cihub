import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Button } from "./button";

describe("Button", () => {
  it("says what runs, and waits, while its action runs", () => {
    render(
      <Button isPending pendingText="Submitting">
        Submit
      </Button>,
    );

    const button = screen.getByRole("button", { name: "Submitting" });

    expect(button).toBeDisabled();
    expect(button).toHaveAttribute("aria-busy", "true");
    expect(button.querySelector("[data-slot=spinner]")).toBeInTheDocument();
  });

  it("is a plain button otherwise", () => {
    render(<Button>Submit</Button>);

    const button = screen.getByRole("button", { name: "Submit" });

    expect(button).toBeEnabled();
    expect(button).not.toHaveAttribute("aria-busy");
  });
});

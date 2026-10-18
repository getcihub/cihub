import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { SignInForm } from "./sign-in-form";

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
});

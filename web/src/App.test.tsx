import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { App } from "./App.tsx";

afterEach(cleanup);

test("the root path shows the home page", () => {
  render(<App path="/" />);
  expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("Merge queue");
});

test("an unknown path shows the not found page", () => {
  render(<App path="/unknown" />);
  expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("Page not found");
});

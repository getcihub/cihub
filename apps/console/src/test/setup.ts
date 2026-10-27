import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

afterEach(() => {
  cleanup();
});

// jsdom has no media queries: the tests ask for less motion, as a person may.
Object.defineProperty(window, "matchMedia", {
  value: (query: string) => ({
    matches: query.includes("prefers-reduced-motion"),
    media: query,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
  }),
});

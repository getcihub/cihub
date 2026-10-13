import { defineConfig } from "@playwright/test";

// The views of the dashboard against a build and the API of `e2e/visual/api.ts` (`mise run web:shots`): no
// server, so the views run in parallel. The browser tests of `playwright.config.ts` run against the simulator.
export default defineConfig({
  testDir: "./e2e/visual",
  testMatch: "*.visual.ts",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  reporter: process.env.CI ? "github" : "line",
  outputDir: "test-results/visual",
  projects: [{ name: "chromium", use: { browserName: "chromium" } }],
});

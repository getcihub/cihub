import { defineConfig, devices } from "@playwright/test";

const port = 4173;
// `scripts/shots-compare.ts` points it to the build of another git ref.
const dist = process.env.SHOTS_DIST ?? "dist";

export default defineConfig({
  testDir: "e2e",
  forbidOnly: process.env.CI !== undefined,
  reporter: process.env.CI === undefined ? "list" : [["github"], ["html", { open: "never" }]],
  use: {
    ...devices["Desktop Chrome"],
    baseURL: `http://127.0.0.1:${port}`,
  },
  projects: [
    { name: "e2e", testIgnore: "shots.spec.ts" },
    { name: "shots", testMatch: "shots.spec.ts" },
  ],
  // The built dashboard, without the cihub server: `vite preview` also answers
  // index.html for the paths of the app, like cihub-web.
  webServer: {
    command: `pnpm exec vite preview --outDir "${dist}" --host 127.0.0.1 --port ${port} --strictPort`,
    url: `http://127.0.0.1:${port}`,
    reuseExistingServer: false,
  },
});

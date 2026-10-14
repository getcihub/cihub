import { defineConfig, devices } from "@playwright/test";

// The browser tests of the dashboard (`mise run web:e2e`): one server on the
// simulator, `cargo run --example e2e`, with a fresh database at each start.
// The tests run one by one, as they share this server.
export default defineConfig({
  testDir: "./e2e",
  workers: 1,
  forbidOnly: !!process.env.CI,
  reporter: process.env.CI ? "github" : "list",
  use: {
    baseURL: "http://127.0.0.1:18090",
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: "cargo run --quiet -p cihub-server --example e2e",
    cwd: "..",
    url: "http://127.0.0.1:18090/sign-in",
    reuseExistingServer: false,
    timeout: 300_000,
  },
});

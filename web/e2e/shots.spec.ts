import { test } from "./fixtures.ts";

// One screenshot per scenario, compared by name between two builds
// (`mise run web-shots-compare`). A scenario mocks the API routes it reads.
const scenarios = [
  { name: "home", path: "/" },
  { name: "not-found", path: "/unknown" },
];

const dir = process.env.SHOTS_DIR ?? ".screenshots/current";

for (const scenario of scenarios) {
  test(scenario.name, async ({ page }) => {
    await page.clock.setFixedTime(new Date("2026-10-01T12:00:00Z"));
    await page.goto(scenario.path);
    await page.screenshot({
      path: `${dir}/${scenario.name}.png`,
      fullPage: true,
      animations: "disabled",
    });
  });
}

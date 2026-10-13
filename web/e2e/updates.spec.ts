// The exit criterion of lot 10d (specs/LOTS.md): a change on the simulator
// shows on an open page in less than 2 seconds, without a reload. The isolation
// of the stream of an organization and its end with the server are tested
// against Postgres (`crates/web/tests/postgres.rs`).
import { expect, test } from "@playwright/test";

test("a change of a queue shows on its open page in less than 2 seconds", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("link", { name: /continue with github/i }).click();
  await page.getByRole("link", { name: /sign in as ada/i }).click();
  await expect(page).toHaveURL(/\/orgs\/acme$/);

  await page.goto("/orgs/acme/repos/web/queues/main");
  await expect(page.getByRole("table")).toBeVisible();
  await expect(page.getByText("Update the changelog")).toHaveCount(0);

  const commented = await page.request.get("/sim/merge?repo=web&number=4&login=ada");
  expect(await commented.text()).toBe("commented");

  await expect(page.getByRole("table").getByText("Update the changelog")).toBeVisible({ timeout: 2_000 });
});

import { expect, test } from "./fixtures.ts";

test("the root path shows the home page", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("Merge queue");
});

test("a path of the app without a page shows the not found page", async ({ page }) => {
  await page.goto("/queues/unknown");
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("Page not found");
  await page.getByRole("link", { name: "Back to the home page" }).click();
  await expect(page).toHaveURL("/");
});
